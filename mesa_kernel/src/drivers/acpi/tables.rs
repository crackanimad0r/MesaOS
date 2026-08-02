// mesa_kernel/src/acpi/tables.rs
//! Estructuras de tablas ACPI

/// RSDP - Root System Description Pointer (ACPI 1.0)
#[repr(C, packed)]
pub struct Rsdp {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
}

/// RSDP extendido (ACPI 2.0+)
#[repr(C, packed)]
pub struct Rsdp2 {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
    // Extended fields
    pub length: u32,
    pub xsdt_address: u64,
    pub extended_checksum: u8,
    pub reserved: [u8; 3],
}

/// Header común de todas las SDT
#[repr(C, packed)]
#[derive(Debug)]
pub struct SdtHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

/// MADT - Multiple APIC Description Table
#[repr(C, packed)]
pub struct Madt {
    pub header: SdtHeader,
    pub local_apic_address: u32,
    pub flags: u32,
    // Seguido de entradas variables
}

/// Entrada MADT: Local APIC
#[repr(C, packed)]
pub struct MadtLocalApic {
    pub entry_type: u8,      // 0
    pub length: u8,          // 8
    pub processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

/// Entrada MADT: I/O APIC
#[repr(C, packed)]
pub struct MadtIoApic {
    pub entry_type: u8,      // 1
    pub length: u8,          // 12
    pub ioapic_id: u8,
    pub reserved: u8,
    pub ioapic_address: u32,
    pub gsi_base: u32,
}

/// Entrada MADT: Interrupt Source Override
#[repr(C, packed)]
pub struct MadtInterruptOverride {
    pub entry_type: u8,      // 2
    pub length: u8,          // 10
    pub bus: u8,
    pub source: u8,
    pub gsi: u32,
    pub flags: u16,
}