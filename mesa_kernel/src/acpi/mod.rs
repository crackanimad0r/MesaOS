// mesa_kernel/src/acpi/mod.rs
#![cfg(target_arch = "x86_64")]

extern crate alloc;

use alloc::{string::String, vec::Vec};

#[derive(Debug, Clone, Copy)]
pub struct InterruptOverride {
    pub source: u8,
    pub global_system_interrupt: u32,
    pub flags: u16,
}

/// Información básica de ACPI
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

static mut ACPI_INFO: Option<AcpiInfo> = None;

/// Convierte una dirección (física o virtual) a virtual usando HHDM si es necesario
fn to_virt(addr: u64) -> u64 {
    let hhdm = crate::limine_req::hhdm_offset().unwrap_or(0);
    if addr < hhdm {
        addr + hhdm
    } else {
        addr
    }
}

/// Inicializa ACPI usando el RSDP de Limine
pub fn init() -> Result<(), &'static str> {
    crate::serial_println!("[ACPI] Iniciando busqueda de tablas...");
    let rsdp_addr = crate::limine_req::rsdp_address().ok_or("No RSDP encontrado")?;
    let hhdm = crate::limine_req::hhdm_offset().unwrap_or(0);
    crate::serial_println!("[ACPI] RSDP: {:#x}, HHDM: {:#x}", rsdp_addr, hhdm);
    
    let rsdp_virt = to_virt(rsdp_addr) as *const u8;
    
    // Verify Signature "RSD PTR "
    let mut rsdp_sig = [0u8; 8];
    unsafe { core::ptr::copy_nonoverlapping(rsdp_virt, rsdp_sig.as_mut_ptr(), 8); }
    if &rsdp_sig != b"RSD PTR " {
        return Err("RSDP Signature mismatch");
    }

    let revision = unsafe { core::ptr::read_volatile(rsdp_virt.add(15)) };
    crate::serial_println!("[ACPI] RSDP Revision: {}", revision);
    
    let (is_xsdt, root_table_addr) = if revision >= 2 {
        let xsdt = unsafe { core::ptr::read_unaligned(rsdp_virt.add(24) as *const u64) };
        (true, xsdt)
    } else {
        let rsdt = unsafe { core::ptr::read_unaligned(rsdp_virt.add(16) as *const u32) } as u64;
        (false, rsdt)
    };
    
    crate::serial_println!("[ACPI] Root Table: {:#x} ({})", root_table_addr, if is_xsdt { "XSDT" } else { "RSDT" });
    if root_table_addr == 0 { return Err("Root table address is null"); }

    let root_virt = (root_table_addr + hhdm) as *const u8;
    let root_len = unsafe { core::ptr::read_unaligned(root_virt.add(4) as *const u32) };
    crate::serial_println!("[ACPI] Root Table len: {}", root_len);

    if root_len < 36 { return Err("Root table length invalid"); }
    
    let entry_size = if is_xsdt { 8 } else { 4 };
    let entries_count = (root_len - 36) / (entry_size as u32);
    crate::serial_println!("[ACPI] Entries to scan: {}", entries_count);

    let mut lapic = 0;
    let mut ioapic = 0;
    let mut overrides = Vec::new();
    let mut has_8042 = true;
    let mut oem_id = [0u8; 6];
    unsafe {
        core::ptr::copy_nonoverlapping(root_virt.add(10), oem_id.as_mut_ptr(), 6);
    }

    for i in 0..entries_count {
        let entry_offset = 36 + (i as usize * entry_size);
        let table_addr = if is_xsdt {
            unsafe { core::ptr::read_unaligned(root_virt.add(entry_offset) as *const u64) }
        } else {
            unsafe { core::ptr::read_unaligned(root_virt.add(entry_offset) as *const u32) as u64 }
        };
        
        if table_addr == 0 { continue; }
        let table_virt = (table_addr + hhdm) as *const u8;
        
        let mut sig = [0u8; 4];
        unsafe { core::ptr::copy_nonoverlapping(table_virt, sig.as_mut_ptr(), 4); }
        
        if &sig == b"FACP" {
            crate::serial_println!("[ACPI] FADT detectado");
            // FADT is at table_virt, Boot Architecture Flags is at offset 109
            let boot_flags = unsafe { core::ptr::read_unaligned(table_virt.add(109) as *const u16) };
            has_8042 = (boot_flags & (1 << 1)) != 0;
            crate::serial_println!("[ACPI] 8042 Support Flag: {}", has_8042);
        }

        if &sig == b"APIC" {
            crate::serial_println!("[ACPI] MADT detectado");
            let madt_base = table_virt;
            lapic = unsafe { core::ptr::read_unaligned(madt_base.add(36) as *const u32) } as u64;
            let madt_len = unsafe { core::ptr::read_unaligned(madt_base.add(4) as *const u32) };
            
            let mut offset = 44;
            let mut io_count = 0;
            while offset < madt_len {
                let typ = unsafe { *madt_base.add(offset as usize) };
                let len = unsafe { *madt_base.add(offset as usize + 1) };
                
                if typ == 1 { // IOAPIC
                    io_count += 1;
                    let addr = unsafe { core::ptr::read_unaligned(madt_base.add(offset as usize + 4) as *const u32) } as u64;
                    let gsi_base = unsafe { core::ptr::read_unaligned(madt_base.add(offset as usize + 8) as *const u32) };
                    crate::serial_println!("[ACPI] IOAPIC #{} en {:#x} (GSI Base: {})", io_count, addr, gsi_base);
                    if gsi_base == 0 { ioapic = addr; }
                }
                
                if typ == 2 { // Override
                    let source = unsafe { *madt_base.add(offset as usize + 3) };
                    let gsi = unsafe { core::ptr::read_unaligned(madt_base.add(offset as usize + 4) as *const u32) };
                    let flags = unsafe { core::ptr::read_unaligned(madt_base.add(offset as usize + 8) as *const u16) };
                    overrides.push(InterruptOverride { source, global_system_interrupt: gsi, flags });
                    crate::serial_println!("[ACPI] Override: IRQ {} -> GSI {} (flags {:#x})", source, gsi, flags);
                }
                
                if len < 2 { break; }
                offset += len as u32;
            }
        }
    }

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

    unsafe { ACPI_INFO = Some(info); }
    crate::serial_println!("[ACPI] Finalizado correctamente");
    Ok(())
}

pub fn get_info() -> Option<&'static AcpiInfo> {
    unsafe { ACPI_INFO.as_ref() }
}
