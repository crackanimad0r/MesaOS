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

pub fn read_mbr(dev: &dyn BlockDevice) -> Result<Mbr, &'static str> {
    let mut buffer = [0u8; SECTOR_SIZE];
    dev.read(0, 1, &mut buffer)?;
    let mbr: Mbr = unsafe { core::ptr::read(buffer.as_ptr() as *const Mbr) };
    if mbr.signature != 0xAA55 { return Err("Invalid MBR signature"); }
    Ok(mbr)
}

pub fn find_mesafs_partition(dev: &dyn BlockDevice) -> Option<(u64, u64)> {
    // 1. Intentar GPT primero
    let mut buffer = [0u8; SECTOR_SIZE];
    if dev.read(1, 1, &mut buffer).is_ok() {
        let header: GptHeader = unsafe { core::ptr::read(buffer.as_ptr() as *const GptHeader) };
        if &header.signature == b"EFI PART" {
            let entry_size = header.partition_entry_size as usize;
            let mut entry_buffer = [0u8; SECTOR_SIZE];
            
            // Escaneamos varios sectores de entradas si es necesario
            for s in 0..4 {
                if dev.read(header.partition_entry_lba + s, 1, &mut entry_buffer).is_ok() {
                    for i in 0..(SECTOR_SIZE / entry_size) {
                        let entry: GptEntry = unsafe { 
                            core::ptr::read(entry_buffer.as_ptr().add(i * entry_size) as *const GptEntry) 
                        };
                        
                        if entry.starting_lba > 0 && entry.ending_lba > entry.starting_lba {
                            // Comprobar si el nombre es "MesaFS" (M=0x4D, e=0x65, s=0x73, a=0x61, F=0x46, S=0x53)
                            // GPT almacena nombres en UTF-16LE
                            let name = entry.partition_name;
                            if name[0] == 0x4D && name[1] == 0x65 && name[2] == 0x73 && name[3] == 0x61 && 
                               name[4] == 0x46 && name[5] == 0x53 {
                                return Some((entry.starting_lba, entry.ending_lba - entry.starting_lba + 1));
                            }
                            
                            // Fallback: Si no tiene nombre pero es de tipo "Linux Data" (no EFI)
                            // GUID: EBD0A0A2... (primeros bytes A2 A0 D0 EB en little endian)
                            if entry.partition_type_guid[0] == 0xA2 && entry.partition_type_guid[1] == 0xA0 {
                                // Evitar la partición 1 si es muy pequeña (usualmente el boot)
                                if entry.starting_lba > 4096 {
                                    return Some((entry.starting_lba, entry.ending_lba - entry.starting_lba + 1));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback a MBR
    if let Ok(mbr) = read_mbr(dev) {
        for p in mbr.partitions.iter() {
            if p.partition_type == 0x7F || p.partition_type == 0x83 { 
                if p.lba_length > 0 {
                    return Some((p.lba_start as u64, p.lba_length as u64));
                }
            }
        }
    }
    None
}

pub fn create_mesafs_partition(dev: &dyn BlockDevice) -> Result<u64, &'static str> {
    let mut buffer = [0u8; SECTOR_SIZE];
    let mut mbr = Mbr {
        bootstrap: [0; 446],
        partitions: [
            PartitionEntry {
                attributes: 0x80,
                chs_start: [0; 3],
                partition_type: 0x7F,
                chs_end: [0; 3],
                lba_start: 2048,
                lba_length: ((dev.capacity() as u32).saturating_sub(2048)).min(409600),
            },
            PartitionEntry { attributes: 0, chs_start: [0; 3], partition_type: 0, chs_end: [0; 3], lba_start: 0, lba_length: 0 },
            PartitionEntry { attributes: 0, chs_start: [0; 3], partition_type: 0, chs_end: [0; 3], lba_start: 0, lba_length: 0 },
            PartitionEntry { attributes: 0, chs_start: [0; 3], partition_type: 0, chs_end: [0; 3], lba_start: 0, lba_length: 0 },
        ],
        signature: 0xAA55,
    };

    unsafe { core::ptr::copy_nonoverlapping(&mbr as *const _ as *const u8, buffer.as_mut_ptr(), 512); }
    dev.write(0, 1, &buffer)?;
    Ok(2048)
}
