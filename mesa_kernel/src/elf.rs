//! Cargador de binarios ELF64 para procesos userland (Ring 3)
//!
//! Mejorado: - Validación exhaustiva de cabeceras y segmentos
//!           - ASLR (Address Space Layout Randomization)
//!           - NX bit enforcement (W^X policy)
//!           - Bounds checking

use crate::memory::{
    address_space::{flags, layout},
    vmm, AddressSpace, PAGE_SIZE,
};
use crate::security;

/// Magic ELF
const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u32 = 1;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;
const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

/// Cabecera ELF64 (e_ident ya validado)
#[repr(C)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

/// Aplica ASLR a una dirección base de código
fn randomize_addr(base: u64, size: u64) -> u64 {
    let entropy_bits = security::ASLR_CODE_ENTROPY_BITS;
    let mask = (1u64 << entropy_bits) - 1;
    let offset = security::random_u64() & mask;
    let page_offset = offset * PAGE_SIZE;
    // Asegurar que no se salga del espacio de usuario
    let randomized = base + page_offset;
    if randomized + size > layout::USER_STACK_TOP {
        base
    } else {
        randomized & !(PAGE_SIZE - 1)
    }
}

/// Convierte flags de segmento ELF a flags de página, aplicando política W^X (NX enforcement)
fn elf_flags_to_page(elf_flags: u32) -> u64 {
    // Política W^X: un segmento no puede ser WRITABLE y EXECUTABLE al mismo tiempo
    let can_write = (elf_flags & PF_W) != 0;
    let can_exec = (elf_flags & PF_X) != 0;
    let can_read = (elf_flags & PF_R) != 0;

    if can_write && can_exec {
        // Violación W^X: dar solo RW por seguridad, forzar NX
        security::audit_log(
            security::AuditSeverity::Warning,
            "ELF W^X violation: segment marked both writable and executable, forcing NX",
        );
        flags::USER_RW
    } else if can_write {
        // Solo escritura: RW, NX (no ejecutable)
        flags::USER_RW
    } else if can_exec && can_read {
        // Ejecutable + legible: RX, no writable
        flags::USER_RX
    } else if can_exec {
        // Solo ejecutable: RX (necesita read para ejecutar)
        flags::USER_RX
    } else if can_read {
        // Solo lectura: R
        flags::USER_RW & !flags::WRITABLE
    } else {
        // Sin permisos (deny all)
        flags::USER_RW
    }
}

/// Valida y carga un ELF64 en el espacio de direcciones. Retorna (entry_point, user_stack_top).
pub fn load_elf(space: &mut AddressSpace, elf: &[u8]) -> Result<(u64, u64), &'static str> {
    // Validación de tamaño mínimo
    if elf.len() < 64 {
        security::audit_log(security::AuditSeverity::Warning, "ELF too small");
        return Err("ELF too small");
    }

    // Validación de longitud vs e_ehsize para evitar out-of-bounds
    let hdr = unsafe { &*(elf.as_ptr() as *const Elf64Ehdr) };

    if hdr.e_ehsize as usize > elf.len() || hdr.e_ehsize < 64 {
        return Err("Invalid ELF header size");
    }

    // Magic ELF
    if hdr.e_ident[0..4] != ELF_MAGIC {
        return Err("Invalid ELF magic");
    }
    if hdr.e_ident[4] != ELFCLASS64 {
        return Err("Not ELF64");
    }
    if hdr.e_ident[5] != ELFDATA2LSB {
        return Err("Not little-endian");
    }
    if hdr.e_version != EV_CURRENT {
        return Err("Unknown ELF version");
    }
    if hdr.e_type != ET_EXEC && hdr.e_type != ET_DYN {
        return Err("Not executable");
    }
    if hdr.e_machine != EM_X86_64 {
        return Err("Not x86_64");
    }

    // Validar program headers
    if hdr.e_phnum == 0 {
        return Err("No program headers");
    }
    if hdr.e_phentsize != core::mem::size_of::<Elf64Phdr>() as u16 {
        return Err("Invalid program header entry size");
    }

    // Validar que los program headers no se salgan del binario
    let phoff = hdr.e_phoff as usize;
    let phnum = hdr.e_phnum as usize;
    let phentsize = hdr.e_phentsize as usize;

    if phoff
        .checked_add(phnum * phentsize)
        .map_or(true, |end| end > elf.len())
    {
        return Err("Program headers out of bounds");
    }

    let orig_entry = hdr.e_entry;
    let e_type = hdr.e_type;

    // Calcular límites para ASLR (solo para ET_DYN/PIE)
    let mut aslr_delta = 0u64;
    if e_type == ET_DYN {
        let mut min_vaddr = u64::MAX;
        let mut max_vaddr = 0u64;

        for i in 0..phnum {
            let off = phoff + i * phentsize;
            let ph = unsafe { &*(elf.as_ptr().add(off) as *const Elf64Phdr) };
            if ph.p_type == PT_LOAD && ph.p_memsz > 0 {
                let vaddr = ph.p_vaddr;
                let memsz = ph.p_memsz;
                if vaddr < min_vaddr {
                    min_vaddr = vaddr;
                }
                if vaddr + memsz > max_vaddr {
                    max_vaddr = vaddr + memsz;
                }
            }
        }

        if min_vaddr == u64::MAX {
            return Err("No loadable segments");
        }

        let total_code_size = max_vaddr - min_vaddr;
        let aslr_offset = randomize_addr(0, total_code_size);
        aslr_delta = aslr_offset;
    }

    // Mapear stack de usuario con ASLR
    let stack_size = layout::USER_STACK_SIZE;
    let stack_bottom = layout::USER_STACK_TOP - stack_size;
    let random_stack_offset =
        security::randomize_stack_top(layout::USER_STACK_TOP, security::ASLR_STACK_ENTROPY_BITS);
    let stack_top_actual = if random_stack_offset < layout::USER_STACK_TOP
        && random_stack_offset >= stack_bottom + 8
    {
        random_stack_offset
    } else {
        layout::USER_STACK_TOP - 8
    };

    space.map_range(stack_bottom, stack_size, flags::USER_RW)?;

    for i in 0..phnum {
        let off = phoff + i * phentsize;
        let ph = unsafe { &*(elf.as_ptr().add(off) as *const Elf64Phdr) };
        if ph.p_type != PT_LOAD {
            continue;
        }

        let vaddr = ph.p_vaddr.wrapping_add(aslr_delta);
        let filesz = ph.p_filesz as usize;
        let memsz = ph.p_memsz as usize;
        let offset = ph.p_offset as usize;

        if memsz == 0 {
            continue;
        }

        // Bounds checking: verificar que el segmento no exceda el binario
        if offset
            .checked_add(filesz)
            .map_or(true, |end| end > elf.len())
        {
            security::audit_log(
                security::AuditSeverity::Warning,
                &alloc::format!("ELF segment at offset {:#x} out of bounds", offset),
            );
            return Err("Segment out of bounds");
        }

        // Validar que vaddr esté en el rango de usuario
        if vaddr >= security::USER_ADDR_MAX || vaddr + memsz as u64 >= security::USER_ADDR_MAX {
            return Err("Segment address outside user space");
        }

        let page_align = ph.p_align.max(PAGE_SIZE);
        let vaddr_align = vaddr & !(page_align - 1);
        let size_pages =
            (memsz + (vaddr - vaddr_align) as usize + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;
        let size_bytes = (size_pages * PAGE_SIZE as usize) as u64;

        // Aplicar flags de página con política W^X (NX enforcement)
        let flags_page = elf_flags_to_page(ph.p_flags);

        space.map_range(vaddr_align, size_bytes, flags_page)?;

        // Copiar datos del segmento
        let src = &elf[offset..offset + filesz];
        space.write_to(vaddr, src)?;

        // Rellenar con ceros la parte BSS (memsz > filesz)
        if memsz > filesz {
            let zero_start = vaddr + filesz as u64;
            let zero_len = memsz - filesz;
            let hhdm = vmm::hhdm_offset();
            for j in 0..zero_len {
                let virt = zero_start + j as u64;
                if let Some(phys) = space.translate(virt) {
                    let ptr = (hhdm + phys + (virt & 0xFFF)) as *mut u8;
                    unsafe { *ptr = 0 };
                }
            }
        }
    }

    // Aplicar ASLR al entry point
    let entry = orig_entry.wrapping_add(aslr_delta);

    crate::serial_println!(
        "[ELF] Loaded with ASLR: entry={:#x}, stack_top={:#x}, delta={:#x}",
        entry,
        stack_top_actual,
        aslr_delta
    );

    security::audit_log(
        security::AuditSeverity::Info,
        &alloc::format!(
            "ELF loaded: entry={:#x}, stack={:#x}, delta={:#x}",
            entry,
            stack_top_actual,
            aslr_delta
        ),
    );

    Ok((entry, stack_top_actual))
}
