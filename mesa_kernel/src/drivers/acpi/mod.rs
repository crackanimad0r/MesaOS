// mesa_kernel/src/acpi/mod.rs

extern crate alloc;

use alloc::{vec::Vec, string::String};
use crate::limine_req;

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
}

static mut ACPI_INFO: Option<AcpiInfo> = None;

/// Inicializa ACPI usando el RSDP de Limine
pub fn init() -> Result<(), &'static str> {
    if let Some(rsdp) = limine_req::rsdp_address() {
        let info = AcpiInfo {
            rsdp_address: rsdp,
            revision: 0,
            oem_id: String::from("MesaOS"),
            local_apic_address: 0,
            ioapic_address: 0,
            cpu_count: limine_req::cpu_count(),
            cpu_ids: Vec::new(),
        };
        unsafe { ACPI_INFO = Some(info); }
        Ok(())
    } else {
        Err("No RSDP encontrado")
    }
}

/// Devuelve la información ACPI si está disponible
pub fn get_info() -> Option<&'static AcpiInfo> {
    unsafe { ACPI_INFO.as_ref() }
}