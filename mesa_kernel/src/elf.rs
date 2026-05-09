//! Cargador de binarios ELF64 para procesos userland (Ring 3)

use crate::memory::{AddressSpace, vmm, PAGE_SIZE, address_space::{flags, layout}};

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

/// Valida y carga un ELF64 en el espacio de direcciones. Retorna (entry_point, user_stack_top).
pub fn load_elf(space: &mut AddressSpace, elf: &[u8]) -> Result<(u64, u64), &'static str> {
    if elf.len() < 64 {
        return Err("ELF too small");
    }
    let hdr = unsafe { &*(elf.as_ptr() as *const Elf64Ehdr) };
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
    if hdr.e_phnum == 0 || hdr.e_phentsize != core::mem::size_of::<Elf64Phdr>() as u16 {
        return Err("Invalid program headers");
    }

    let entry = hdr.e_entry;
    let phoff = hdr.e_phoff as usize;
    let phnum = hdr.e_phnum as usize;
    let phentsize = hdr.e_phentsize as usize;

    // Mapear stack de usuario (todos los ELF lo necesitan)
    let stack_bottom = layout::USER_STACK_TOP - layout::USER_STACK_SIZE;
    space.map_range(stack_bottom, layout::USER_STACK_SIZE, flags::USER_RW)?;

    for i in 0..phnum {
        let off = phoff + i * phentsize;
        if off + core::mem::size_of::<Elf64Phdr>() > elf.len() {
            return Err("Program header out of bounds");
        }
        let ph = unsafe { &*(elf.as_ptr().add(off) as *const Elf64Phdr) };
        if ph.p_type != PT_LOAD {
            continue;
        }
        let vaddr = ph.p_vaddr;
        let filesz = ph.p_filesz as usize;
        let memsz = ph.p_memsz as usize;
        let offset = ph.p_offset as usize;
        if memsz == 0 {
            continue;
        }
        if offset + filesz > elf.len() {
            return Err("Segment out of bounds");
        }
        let page_align = ph.p_align.max(PAGE_SIZE);
        let vaddr_align = vaddr & !(page_align - 1);
        let size_pages = (memsz + (vaddr - vaddr_align) as usize + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;
        let size_bytes = (size_pages * PAGE_SIZE as usize) as u64;

        let flags_page = if (ph.p_flags & (PF_R | PF_W | PF_X)) == (PF_R | PF_W | PF_X) {
            flags::USER_RWX
        } else if (ph.p_flags & (PF_R | PF_X)) == (PF_R | PF_X) {
            flags::USER_RX
        } else if (ph.p_flags & (PF_R | PF_W)) == (PF_R | PF_W) {
            flags::USER_RW
        } else if ph.p_flags & PF_R != 0 {
            flags::USER_RX
        } else {
            flags::USER_RW
        };

        space.map_range(vaddr_align, size_bytes, flags_page)?;

        let src = &elf[offset..offset + filesz];
        space.write_to(vaddr, src)?;

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

    crate::serial_println!("[ELF] Loaded: entry={:#x}, stack_top={:#x}", entry, layout::USER_STACK_TOP - 8);
    Ok((entry, layout::USER_STACK_TOP - 8))
}
