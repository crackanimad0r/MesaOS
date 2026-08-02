// mesa_kernel/src/acpi/mod.rs
#![cfg(target_arch = "x86_64")]

extern crate alloc;

pub mod aml_handler;

use alloc::{boxed::Box, string::String, vec::Vec};
use aml::AmlContext;
use aml_handler::MesaAmlHandler;
use spin::Mutex;

// ── Tipos públicos ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct InterruptOverride {
    pub source: u8,
    pub global_system_interrupt: u32,
    pub flags: u16,
}

#[derive(Debug, Clone)]
pub struct AcpiInfo {
    pub rsdp_address: u64,
    pub revision: u8,
    pub oem_id: String,
    pub local_apic_address: u64,
    pub ioapic_address: u64,
    pub cpu_count: usize,
    pub cpu_ids: Vec<u8>,
    pub overrides: Vec<InterruptOverride>,
    pub has_8042: bool,
}

// ── Estado global ──────────────────────────────────────────────────────────────

static mut ACPI_INFO: Option<AcpiInfo> = None;

/// Contexto AML global. Disponible tras `init()`.
static AML_CTX: Mutex<Option<AmlContext>> = Mutex::new(None);

// ── Helpers de dirección ───────────────────────────────────────────────────────

fn to_virt(addr: u64) -> u64 {
    let hhdm = crate::limine_req::hhdm_offset().unwrap_or(0);
    if addr < hhdm {
        addr + hhdm
    } else {
        addr
    }
}

/// Lee la longitud (bytes 4-7) de cualquier tabla SDT.
unsafe fn sdt_len(virt: *const u8) -> u32 {
    core::ptr::read_unaligned(virt.add(4) as *const u32)
}

unsafe fn sdt_sig(virt: *const u8) -> [u8; 4] {
    let mut sig = [0u8; 4];
    core::ptr::copy_nonoverlapping(virt, sig.as_mut_ptr(), 4);
    sig
}

// ── Inicialización ─────────────────────────────────────────────────────────────

pub fn init() -> Result<(), &'static str> {
    crate::serial_println!("[ACPI] Iniciando parseo de tablas...");

    let rsdp_addr = crate::limine_req::rsdp_address().ok_or("No RSDP encontrado")?;
    let hhdm = crate::limine_req::hhdm_offset().unwrap_or(0);

    let rsdp_virt = to_virt(rsdp_addr) as *const u8;

    // ── Verificar firma "RSD PTR " ────────────────────────────────────────────
    let mut sig = [0u8; 8];
    unsafe {
        core::ptr::copy_nonoverlapping(rsdp_virt, sig.as_mut_ptr(), 8);
    }
    if &sig != b"RSD PTR " {
        return Err("RSDP signature mismatch");
    }

    let revision = unsafe { core::ptr::read_volatile(rsdp_virt.add(15)) };
    crate::serial_println!("[ACPI] RSDP rev={} en {:#x}", revision, rsdp_addr);

    // ── Localizar tabla raíz (RSDT o XSDT) ───────────────────────────────────
    let (is_xsdt, root_phys) = if revision >= 2 {
        let x = unsafe { core::ptr::read_unaligned(rsdp_virt.add(24) as *const u64) };
        (true, x)
    } else {
        let r = unsafe { core::ptr::read_unaligned(rsdp_virt.add(16) as *const u32) } as u64;
        (false, r)
    };
    if root_phys == 0 {
        return Err("Root table address is null");
    }

    let root_virt = (root_phys + hhdm) as *const u8;
    let root_len = unsafe { sdt_len(root_virt) };
    if root_len < 36 {
        return Err("Root table length invalid");
    }

    let entry_sz = if is_xsdt { 8usize } else { 4usize };
    let n_entries = (root_len as usize - 36) / entry_sz;
    crate::serial_println!(
        "[ACPI] {} entradas en {}",
        if is_xsdt { "XSDT" } else { "RSDT" },
        n_entries
    );

    // ── Variables de resultado ────────────────────────────────────────────────
    let mut lapic: u64 = 0;
    let mut ioapic: u64 = 0;
    let mut overrides: Vec<InterruptOverride> = Vec::new();
    let mut has_8042: bool = true;
    let mut dsdt_phys: u64 = 0;
    let mut oem_id = [0u8; 6];
    unsafe {
        core::ptr::copy_nonoverlapping(root_virt.add(10), oem_id.as_mut_ptr(), 6);
    }

    // Recopilamos direcciones físicas de todas las SSDTs
    let mut ssdt_phys_list: Vec<u64> = Vec::new();

    // ── Iterar entradas ───────────────────────────────────────────────────────
    for i in 0..n_entries {
        let off = 36 + i * entry_sz;
        let tbl_phys = if is_xsdt {
            unsafe { core::ptr::read_unaligned(root_virt.add(off) as *const u64) }
        } else {
            unsafe { core::ptr::read_unaligned(root_virt.add(off) as *const u32) as u64 }
        };
        if tbl_phys == 0 {
            continue;
        }

        let tv = (tbl_phys + hhdm) as *const u8;
        let s = unsafe { sdt_sig(tv) };
        let table_len = unsafe { sdt_len(tv) };
        if table_len < 36 {
            crate::serial_println!(
                "[ACPI] Tabla {:?} ignorada: longitud inválida {}",
                s,
                table_len
            );
            continue;
        }

        match &s {
            b"FACP" => {
                crate::serial_println!("[ACPI] FADT en {:#x}", tbl_phys);
                if table_len >= 111 {
                    // IA-PC Boot Architecture Flags está en offset 109 (bytes).
                    let boot_flags =
                        unsafe { core::ptr::read_unaligned(tv.add(109) as *const u16) };
                    has_8042 = (boot_flags & (1 << 1)) != 0;
                }

                let dsdt = if table_len >= 44 {
                    (unsafe { core::ptr::read_unaligned(tv.add(40) as *const u32) }) as u64
                } else {
                    0
                };

                // X_DSDT solo existe en FADT v2+ y requiere que la tabla sea lo bastante larga.
                let x_dsdt = if revision >= 2 && table_len >= 148 {
                    unsafe { core::ptr::read_unaligned(tv.add(140) as *const u64) }
                } else {
                    0
                };

                dsdt_phys = if x_dsdt != 0 { x_dsdt } else { dsdt };
                crate::serial_println!("[ACPI] DSDT en {:#x}, 8042={}", dsdt_phys, has_8042);
            }
            b"APIC" => {
                crate::serial_println!("[ACPI] MADT en {:#x}", tbl_phys);
                if table_len < 44 {
                    crate::serial_println!("[ACPI] MADT ignorada: longitud inválida {}", table_len);
                    continue;
                }
                lapic = unsafe { core::ptr::read_unaligned(tv.add(36) as *const u32) } as u64;
                let madt_len = table_len;
                let mut off2 = 44u32;
                while off2 < madt_len {
                    let typ = unsafe { *tv.add(off2 as usize) };
                    let len = unsafe { *tv.add(off2 as usize + 1) };
                    if len < 2 || off2 + len as u32 > madt_len {
                        break;
                    }
                    match typ {
                        1 => {
                            // IOAPIC
                            if len < 12 {
                                off2 += len as u32;
                                continue;
                            }
                            let addr = unsafe {
                                core::ptr::read_unaligned(tv.add(off2 as usize + 4) as *const u32)
                            } as u64;
                            let gsi = unsafe {
                                core::ptr::read_unaligned(tv.add(off2 as usize + 8) as *const u32)
                            };
                            crate::serial_println!("[ACPI] IOAPIC en {:#x} GSI={}", addr, gsi);
                            if gsi == 0 {
                                ioapic = addr;
                            }
                        }
                        2 => {
                            // Interrupt Source Override
                            if len < 10 {
                                off2 += len as u32;
                                continue;
                            }
                            let src = unsafe { *tv.add(off2 as usize + 3) };
                            let gsi = unsafe {
                                core::ptr::read_unaligned(tv.add(off2 as usize + 4) as *const u32)
                            };
                            let flags = unsafe {
                                core::ptr::read_unaligned(tv.add(off2 as usize + 8) as *const u16)
                            };
                            overrides.push(InterruptOverride {
                                source: src,
                                global_system_interrupt: gsi,
                                flags,
                            });
                        }
                        _ => {}
                    }
                    off2 += len as u32;
                }
            }
            b"SSDT" => {
                ssdt_phys_list.push(tbl_phys);
            }
            _ => {}
        }
    }

    // ── Guardar AcpiInfo ──────────────────────────────────────────────────────
    let info = AcpiInfo {
        rsdp_address: rsdp_addr,
        revision,
        oem_id: String::from_utf8_lossy(&oem_id).into_owned(),
        local_apic_address: lapic,
        ioapic_address: ioapic,
        cpu_count: crate::limine_req::cpu_count(),
        cpu_ids: Vec::new(),
        overrides,
        has_8042,
    };
    unsafe {
        ACPI_INFO = Some(info);
    }
    crate::serial_println!("[ACPI] LAPIC={:#x} IOAPIC={:#x}", lapic, ioapic);

    // ── Crear contexto AML ────────────────────────────────────────────────────
    let mut aml = AmlContext::new(Box::new(MesaAmlHandler), aml::DebugVerbosity::None);

    // Cargar DSDT
    if dsdt_phys != 0 {
        let dv = (dsdt_phys + hhdm) as *const u8;
        let dsig = unsafe { sdt_sig(dv) };
        let dlen = unsafe { sdt_len(dv) } as usize;
        if dsig == *b"DSDT" && dlen > 36 {
            let bytes = unsafe { core::slice::from_raw_parts(dv.add(36), dlen - 36) };
            match aml.parse_table(bytes) {
                Ok(()) => crate::serial_println!("[AML] DSDT parseado ({} bytes)", bytes.len()),
                Err(e) => crate::serial_println!("[AML] Error DSDT: {:?}", e),
            }
        } else {
            crate::serial_println!("[AML] DSDT inválido/ignorado sig={:?} len={}", dsig, dlen);
        }
    } else {
        crate::serial_println!("[AML] DSDT no encontrado");
    }

    // Cargar SSDTs
    for (i, ssdt_phys) in ssdt_phys_list.iter().enumerate() {
        let sv = (ssdt_phys + hhdm) as *const u8;
        let ssig = unsafe { sdt_sig(sv) };
        let slen = unsafe { sdt_len(sv) } as usize;
        if ssig == *b"SSDT" && slen > 36 {
            let bytes = unsafe { core::slice::from_raw_parts(sv.add(36), slen - 36) };
            match aml.parse_table(bytes) {
                Ok(()) => {
                    crate::serial_println!("[AML] SSDT[{}] parseado ({} bytes)", i, bytes.len())
                }
                Err(e) => crate::serial_println!("[AML] Error SSDT[{}]: {:?}", i, e),
            }
        } else {
            crate::serial_println!(
                "[AML] SSDT[{}] inválido/ignorado sig={:?} len={}",
                i,
                ssig,
                slen
            );
        }
    }

    // Inicializar objetos del namespace
    if let Err(e) = aml.initialize_objects() {
        crate::serial_println!("[AML] Advertencia initialize_objects: {:?}", e);
    }

    *AML_CTX.lock() = Some(aml);
    crate::serial_println!("[ACPI] Listo. Contexto AML disponible.");
    Ok(())
}

// ── API pública ────────────────────────────────────────────────────────────────

pub fn get_info() -> Option<&'static AcpiInfo> {
    unsafe { ACPI_INFO.as_ref() }
}

/// Ejecuta `f` con acceso mutable al contexto AML global.
/// Devuelve `None` si el contexto no está inicializado.
pub fn with_aml<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AmlContext) -> R,
{
    AML_CTX.lock().as_mut().map(f)
}
