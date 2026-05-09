use crate::drivers::block::{BlockDevice, SECTOR_SIZE};

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PartitionEntry {
    pub attributes: u8,
    pub chs_start: [u8; 3],
    pub partition_type: u8,
    pub chs_end: [u8; 3],
    pub lba_start: u32,
    pub lba_length: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Mbr {
    pub bootstrap: [u8; 446],
    pub partitions: [PartitionEntry; 4],
    pub signature: u16,
}

// Estructuras GPT
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct GptHeader {
    pub signature: [u8; 8], // "EFI PART"
    pub revision: u32,
    pub header_size: u32,
    pub header_crc32: u32,
    pub reserved: u32,
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: [u8; 16],
    pub partition_entry_lba: u64,
    pub num_partition_entries: u32,
    pub partition_entry_size: u32,
    pub partition_entry_array_crc32: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct GptEntry {
    pub partition_type_guid: [u8; 16],
    pub unique_partition_guid: [u8; 16],
    pub starting_lba: u64,
    pub ending_lba: u64,
    pub attributes: u64,
    pub partition_name: [u16; 36],
}

pub fn find_mesafs_partition(_dev: &dyn BlockDevice) -> Option<(u64, u64)> {
    // Disabled for safety
    None
}

pub fn create_mesafs_partition(_dev: &dyn BlockDevice) -> Result<u64, &'static str> {
    // Disabled for safety
    Err("Partition creation disabled")
}
