use crate::drivers::block::{BlockDevice, SECTOR_SIZE};
use crate::drivers::usb::xhci_native::UsbBlockDevice;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub static USB_BLOCK_DEVICE: UsbBlockDevice = UsbBlockDevice;

static MESA_FS_STATE: spin::Mutex<MesaFsState> = spin::Mutex::new(MesaFsState::empty());

#[derive(Copy, Clone)]
struct MesaFsState {
    pub initialized: bool,
    pub free_start_lba: u64,
    pub free_sectors: u64,
}

impl MesaFsState {
    const fn empty() -> Self {
        Self {
            initialized: false,
            free_start_lba: 0,
            free_sectors: 0,
        }
    }
}

#[repr(C, packed)]
pub struct MesaFsHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub total_sectors: u64,
    pub file_count: u32,
    pub reserved: [u8; 488],
}

#[repr(C, packed)]
pub struct MesaFsFileEntry {
    pub name: [u8; 32],
    pub start_lba_offset: u32,
    pub sector_count: u32,
    pub size_bytes: u32,
    pub is_used: u8,
}

const MESA_FS_MAGIC: &[u8; 8] = b"MESA_FS1";
const MESA_FS_VERSION: u32 = 1;
const ENTRIES_PER_SECTOR: usize = 11;
const MAX_FILES: usize = 128;

const ENTRY_UNUSED: u8 = 0;
const ENTRY_FILE: u8 = 1;
const ENTRY_DIR: u8 = 2;

#[repr(C, align(64))]
struct AlignedSector {
    data: [u8; SECTOR_SIZE],
}

pub fn mesa_fs_init() -> bool {
    crate::mesa_println!("[MesaFS] Iniciando búsqueda dinámica de partición MesaFS...");

    let mut header_buf = [0u8; SECTOR_SIZE];
    if USB_BLOCK_DEVICE.read(1, 1, &mut header_buf).is_err() {
        crate::mesa_println!("[MesaFS] ❌ Error leyendo GPT Header (LBA 1)");
        return false;
    }

    if &header_buf[0..8] != b"EFI PART" {
        crate::mesa_println!("[MesaFS] ⚠️  No se encontró tabla GPT en el dispositivo");
        return false;
    }

    let part_entry_lba = u64::from_le_bytes(header_buf[72..80].try_into().unwrap());
    let num_entries = u32::from_le_bytes(header_buf[80..84].try_into().unwrap());
    let entry_size = u32::from_le_bytes(header_buf[84..88].try_into().unwrap());

    crate::mesa_println!(
        "[MesaFS] GPT: entries_lba={}, num_entries={}, entry_size={}",
        part_entry_lba,
        num_entries,
        entry_size
    );

    let entries_per_sector = 512 / entry_size as usize;
    let total_secs = (num_entries as usize + entries_per_sector - 1) / entries_per_sector;

    let mut last_partition: Option<(u64, u64, [u8; 36])> = None;

    for sec in 0..total_secs.min(32) {
        let lba = part_entry_lba + sec as u64;
        let mut entry_buf = [0u8; SECTOR_SIZE];
        if USB_BLOCK_DEVICE.read(lba, 1, &mut entry_buf).is_err() {
            crate::mesa_println!("[MesaFS] ⚠️  Error leyendo LBA {} (entradas GPT)", lba);
            break;
        }

        for ei in 0..entries_per_sector {
            let off = ei * entry_size as usize;
            if off + 16 > 512 {
                break;
            }

            let guid: [u8; 16] = entry_buf[off..off + 16].try_into().unwrap();
            if guid == [0u8; 16] {
                continue;
            }

            let start = u64::from_le_bytes(entry_buf[off + 32..off + 40].try_into().unwrap());
            let end = u64::from_le_bytes(entry_buf[off + 40..off + 48].try_into().unwrap());

            // Extraer nombre UTF-16LE
            let mut name_bytes = [0u8; 36];
            for j in 0..18 {
                let idx = off + 56 + j * 2;
                if idx + 2 > 512 {
                    break;
                }
                let c = u16::from_le_bytes([entry_buf[idx], entry_buf[idx + 1]]);
                if c == 0 {
                    break;
                }
                name_bytes[j] = (c & 0xFF) as u8;
            }

            let part_name = core::str::from_utf8(&name_bytes).unwrap_or("?");
            let mb = ((end - start + 1) * 512) / (1024 * 1024);
            crate::mesa_println!(
                "[MesaFS]   🗂️  Partición: LBA {}-{} ({} MB) '{}'",
                start,
                end,
                mb,
                part_name
            );

            last_partition = Some((start, end, name_bytes));

            let mut sector_buf = [0u8; SECTOR_SIZE];
            if USB_BLOCK_DEVICE.read(start, 1, &mut sector_buf).is_ok()
                && &sector_buf[0..8] == MESA_FS_MAGIC
            {
                let file_count = u32::from_le_bytes(sector_buf[12..16].try_into().unwrap());
                let header_total = u64::from_le_bytes(sector_buf[16..24].try_into().unwrap());
                let gpt_total = end - start + 1;

                if header_total == 0 {
                    crate::mesa_println!(
                        "[MesaFS] ⚠️  total_sectors=0 en cabecera (escrito por Python), corrigiendo a GPT size {}...",
                        gpt_total
                    );
                    sector_buf[16..24].copy_from_slice(&gpt_total.to_le_bytes());
                    sector_buf[12..16].copy_from_slice(&file_count.to_le_bytes());
                    if USB_BLOCK_DEVICE.write(start, 1, &sector_buf).is_err() {
                        crate::mesa_println!(
                            "[MesaFS] ❌ Error re-escribiendo cabecera con total_sectors corregido"
                        );
                    } else {
                        crate::mesa_println!(
                            "[MesaFS] ✅ Cabecera corregida: total_sectors={} ({:.2} GB)",
                            gpt_total,
                            gpt_total as f64 * 512.0 / (1024.0 * 1024.0 * 1024.0)
                        );
                    }
                }

                // Verificar que la tabla de entradas (LBA start+1) tenga datos coherentes
                let mut ent_check = [0u8; SECTOR_SIZE];
                let entries_ok = if USB_BLOCK_DEVICE.read(start + 1, 1, &mut ent_check).is_ok() {
                    let has_entries = ent_check.iter().any(|&b| b != 0);
                    if !has_entries && file_count == 0 {
                        crate::mesa_println!(
                            "[MesaFS]    Tabla de entradas vacía (MesaFS recién creado)"
                        );
                        true
                    } else if has_entries {
                        crate::mesa_println!("[MesaFS]    Tabla de entradas con datos presentes");
                        true
                    } else {
                        crate::mesa_println!("[MesaFS] ⚠️  Tabla de entradas vacía pero file_count>0, corrigiendo file_count a 0");
                        sector_buf[12..16].copy_from_slice(&0u32.to_le_bytes());
                        let _ = USB_BLOCK_DEVICE.write(start, 1, &sector_buf);
                        true
                    }
                } else {
                    crate::mesa_println!(
                        "[MesaFS] ⚠️  Error leyendo tabla de entradas, inicializando..."
                    );
                    let empty = [0u8; SECTOR_SIZE];
                    USB_BLOCK_DEVICE.write(start + 1, 1, &empty).is_ok()
                };

                if !entries_ok {
                    crate::mesa_println!("[MesaFS] ❌ No se pudo reparar la tabla de entradas");
                    return false;
                }

                let display_size = if header_total == 0 {
                    gpt_total
                } else {
                    header_total
                };
                crate::mesa_println!(
                    "[MesaFS] ✅ ¡Partición MesaFS detectada dinámicamente en LBA {}!",
                    start
                );
                crate::mesa_println!(
                    "[MesaFS]    Archivos: {}, Sectores: {} ({:.2} GB)",
                    file_count,
                    display_size,
                    display_size as f64 * 512.0 / (1024.0 * 1024.0 * 1024.0)
                );

                let mut state = MESA_FS_STATE.lock();
                state.free_start_lba = start;
                state.free_sectors = gpt_total;
                state.initialized = true;
                crate::mesa_println!(
                    "[MesaFS] Guardado LBA dinámico de trabajo: {} (partición LBA {}-{})",
                    start,
                    start,
                    end
                );
                return false;
            }
        }
    }

    if let Some((start, end, name_bytes)) = last_partition {
        let sectors = end - start + 1;
        let gb = sectors as f64 * 512.0 / (1024.0 * 1024.0 * 1024.0);
        let part_name = core::str::from_utf8(&name_bytes).unwrap_or("?");
        crate::mesa_println!("[MesaFS] ⚠️  Firma MesaFS no encontrada en ninguna partición");
        crate::mesa_println!(
            "[MesaFS] Auto-formateando la partición GPT '{}' en LBA {} ({:.2} GB)...",
            part_name,
            start,
            gb
        );

        let header = MesaFsHeader {
            magic: *MESA_FS_MAGIC,
            version: MESA_FS_VERSION.to_le(),
            total_sectors: sectors.to_le(),
            file_count: 0u32.to_le(),
            reserved: [0u8; 488],
        };
        let mut header_buf = [0u8; SECTOR_SIZE];
        unsafe {
            core::ptr::copy_nonoverlapping(
                &header as *const MesaFsHeader as *const u8,
                header_buf.as_mut_ptr(),
                SECTOR_SIZE,
            );
        }

        if USB_BLOCK_DEVICE.write(start, 1, &header_buf).is_err() {
            crate::mesa_println!(
                "[MesaFS] ❌ Error escribiendo cabecera MesaFS en LBA {}",
                start
            );
            return false;
        }
        crate::mesa_println!("[MesaFS] ✅ Cabecera MesaFS escrita en LBA {}", start);

        let ent_buf = AlignedSector {
            data: [0u8; SECTOR_SIZE],
        };
        if USB_BLOCK_DEVICE.write(start + 1, 1, &ent_buf.data).is_err() {
            crate::mesa_println!(
                "[MesaFS] ❌ Error inicializando tabla de entradas en LBA {}",
                start + 1
            );
            return false;
        }
        crate::mesa_println!(
            "[MesaFS] ✅ Tabla de entradas inicializada en LBA {}",
            start + 1
        );

        let mut state = MESA_FS_STATE.lock();
        state.free_start_lba = start;
        state.free_sectors = sectors;
        state.initialized = true;
        crate::mesa_println!("[MesaFS] ✅ MesaFS montado en LBA {} ({:.2} GB)", start, gb);
        return true;
    }

    crate::mesa_println!("[MesaFS] ⚠️  No se encontraron particiones válidas en GPT");
    false
}

pub fn mesa_fs_is_initialized() -> bool {
    MESA_FS_STATE.lock().initialized
}

pub fn mesa_fs_list_files() -> Vec<(String, u32, u32)> {
    let state = MESA_FS_STATE.lock();
    if !state.initialized {
        return Vec::new();
    }
    let free_start = state.free_start_lba;
    drop(state);

    let mut ent_buf = AlignedSector {
        data: [0u8; SECTOR_SIZE],
    };
    if USB_BLOCK_DEVICE
        .read(free_start + 1, 1, &mut ent_buf.data)
        .is_err()
    {
        return Vec::new();
    }

    let mut files = Vec::new();
    for i in 0..ENTRIES_PER_SECTOR {
        let off = i * core::mem::size_of::<MesaFsFileEntry>();
        let entry_type = ent_buf.data[off + 44];
        if entry_type != ENTRY_FILE {
            continue;
        }
        let mut name_bytes = [0u8; 32];
        name_bytes.copy_from_slice(&ent_buf.data[off..off + 32]);
        let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let name = core::str::from_utf8(&name_bytes[..name_len])
            .unwrap_or("?")
            .to_string();
        let sectors = u32::from_le_bytes(ent_buf.data[off + 36..off + 40].try_into().unwrap());
        let size = u32::from_le_bytes(ent_buf.data[off + 40..off + 44].try_into().unwrap());
        files.push((name, size, sectors));
    }
    files
}

pub fn mesa_fs_list_dir() -> Vec<(String, u8, u32, u32)> {
    let state = MESA_FS_STATE.lock();
    if !state.initialized {
        return Vec::new();
    }
    let free_start = state.free_start_lba;
    drop(state);

    let mut ent_buf = AlignedSector {
        data: [0u8; SECTOR_SIZE],
    };
    if USB_BLOCK_DEVICE
        .read(free_start + 1, 1, &mut ent_buf.data)
        .is_err()
    {
        return Vec::new();
    }

    let mut entries = Vec::new();
    for i in 0..ENTRIES_PER_SECTOR {
        let off = i * core::mem::size_of::<MesaFsFileEntry>();
        let entry_type = ent_buf.data[off + 44];
        if entry_type == ENTRY_UNUSED {
            continue;
        }
        let mut name_bytes = [0u8; 32];
        name_bytes.copy_from_slice(&ent_buf.data[off..off + 32]);
        let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let name = core::str::from_utf8(&name_bytes[..name_len])
            .unwrap_or("?")
            .to_string();
        let sectors = u32::from_le_bytes(ent_buf.data[off + 36..off + 40].try_into().unwrap());
        let size = u32::from_le_bytes(ent_buf.data[off + 40..off + 44].try_into().unwrap());
        entries.push((name, entry_type, size, sectors));
    }
    entries
}

pub fn mesa_fs_read_file(filename: &str) -> Option<Vec<u8>> {
    let state = MESA_FS_STATE.lock();
    if !state.initialized {
        return None;
    }
    let free_start = state.free_start_lba;
    drop(state);

    let mut ent_buf = AlignedSector {
        data: [0u8; SECTOR_SIZE],
    };
    if USB_BLOCK_DEVICE
        .read(free_start + 1, 1, &mut ent_buf.data)
        .is_err()
    {
        return None;
    }

    for i in 0..ENTRIES_PER_SECTOR {
        let off = i * core::mem::size_of::<MesaFsFileEntry>();
        let is_used = ent_buf.data[off + 44];
        if is_used == 0 {
            continue;
        }
        let mut name_bytes = [0u8; 32];
        name_bytes.copy_from_slice(&ent_buf.data[off..off + 32]);
        let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let name = core::str::from_utf8(&name_bytes[..name_len]).unwrap_or("?");
        if name == filename {
            let start_off =
                u32::from_le_bytes(ent_buf.data[off + 32..off + 36].try_into().unwrap());
            let sector_count =
                u32::from_le_bytes(ent_buf.data[off + 36..off + 40].try_into().unwrap());
            let size_bytes =
                u32::from_le_bytes(ent_buf.data[off + 40..off + 44].try_into().unwrap());
            let data_lba = free_start + start_off as u64;
            let mut data = alloc::vec![0u8; (sector_count as usize) * SECTOR_SIZE];
            if USB_BLOCK_DEVICE
                .read(data_lba, sector_count as u16, &mut data)
                .is_err()
            {
                return None;
            }
            data.truncate(size_bytes as usize);
            return Some(data);
        }
    }
    None
}

pub fn mesa_fs_write_file(filename: &str, data: &[u8]) -> Result<(), &'static str> {
    let state = MESA_FS_STATE.lock();
    if !state.initialized {
        return Err("MesaFS no inicializado");
    }
    let free_start = state.free_start_lba;
    let free_sectors = state.free_sectors;
    drop(state);

    let data_sectors = ((data.len() + SECTOR_SIZE - 1) / SECTOR_SIZE) as u32;
    let total_needed = data_sectors + 2;
    if total_needed as u64 > free_sectors {
        return Err("No hay suficiente espacio libre");
    }

    let mut ent_buf = AlignedSector {
        data: [0u8; SECTOR_SIZE],
    };
    USB_BLOCK_DEVICE
        .read(free_start + 1, 1, &mut ent_buf.data)
        .map_err(|_| "Error leyendo tabla de entradas")?;

    for i in 0..ENTRIES_PER_SECTOR {
        let off = i * core::mem::size_of::<MesaFsFileEntry>();
        let entry_type = ent_buf.data[off + 44];
        if entry_type != ENTRY_UNUSED {
            let mut name_bytes = [0u8; 32];
            name_bytes.copy_from_slice(&ent_buf.data[off..off + 32]);
            let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(32);
            let name = core::str::from_utf8(&name_bytes[..name_len]).unwrap_or("?");
            if name == filename {
                return Err("El archivo ya existe");
            }
            continue;
        }

        let mut name_bytes = [0u8; 32];
        let name_slice = filename.as_bytes();
        let copy_len = name_slice.len().min(31);
        name_bytes[..copy_len].copy_from_slice(&name_slice[..copy_len]);
        name_bytes[copy_len] = 0;

        let mut file_lba_off = 2u32;
        let mut occupied = [false; 8192];
        for j in 0..i {
            let joff = j * core::mem::size_of::<MesaFsFileEntry>();
            if ent_buf.data[joff + 44] != 0 {
                let existing_off =
                    u32::from_le_bytes(ent_buf.data[joff + 32..joff + 36].try_into().unwrap());
                let existing_cnt =
                    u32::from_le_bytes(ent_buf.data[joff + 36..joff + 40].try_into().unwrap());
                for k in 0..existing_cnt {
                    let idx = (existing_off + k) as usize;
                    if idx < occupied.len() {
                        occupied[idx] = true;
                    }
                }
            }
        }
        for search in 2..free_sectors.min(8190) as u32 {
            if !occupied[search as usize] {
                file_lba_off = search;
                break;
            }
        }

        ent_buf.data[off..off + 32].copy_from_slice(&name_bytes);
        ent_buf.data[off + 32..off + 36].copy_from_slice(&file_lba_off.to_le_bytes());
        ent_buf.data[off + 36..off + 40].copy_from_slice(&data_sectors.to_le_bytes());
        ent_buf.data[off + 40..off + 44].copy_from_slice(&(data.len() as u32).to_le_bytes());
        ent_buf.data[off + 44] = ENTRY_FILE;

        USB_BLOCK_DEVICE
            .write(free_start + 1, 1, &ent_buf.data)
            .map_err(|_| "Error escribiendo tabla de entradas")?;

        let data_lba = free_start + file_lba_off as u64;
        let mut padded = alloc::vec![0u8; (data_sectors as usize) * SECTOR_SIZE];
        padded[..data.len()].copy_from_slice(data);
        USB_BLOCK_DEVICE
            .write(data_lba, data_sectors as u16, &padded)
            .map_err(|_| "Error escribiendo datos del archivo")?;

        let mut header_buf = [0u8; SECTOR_SIZE];
        USB_BLOCK_DEVICE
            .read(free_start, 1, &mut header_buf)
            .map_err(|_| "Error leyendo cabecera")?;
        let file_count = u32::from_le_bytes(header_buf[12..16].try_into().unwrap());
        header_buf[12..16].copy_from_slice(&(file_count + 1).to_le_bytes());
        USB_BLOCK_DEVICE
            .write(free_start, 1, &header_buf)
            .map_err(|_| "Error actualizando cabecera")?;

        return Ok(());
    }
    Err("Tabla de entradas llena")
}

pub fn mesa_fs_mkdir(name: &str) -> Result<(), &'static str> {
    let state = MESA_FS_STATE.lock();
    if !state.initialized {
        return Err("MesaFS no inicializado");
    }
    let free_start = state.free_start_lba;
    drop(state);

    // Extraer solo el nombre base (último componente) en caso de ruta completa
    let entry_name = name.rsplit('/').next().unwrap_or(name);
    crate::mesa_println!(
        "[DEBUG MKDIR] 1. Nombre extraído: '{}' (original: '{}')",
        entry_name,
        name
    );

    let target_lba = free_start + 1;
    crate::mesa_println!(
        "[DEBUG MKDIR] 2. Leyendo tabla de entradas en LBA: {}",
        target_lba
    );

    let mut ent_buf = AlignedSector {
        data: [0u8; SECTOR_SIZE],
    };
    crate::mesa_println!(
        "[DEBUG MKDIR] 2a. Buffer ent_buf en {:p} (alin: {})",
        &ent_buf.data as *const _,
        (&ent_buf.data as *const u8 as usize) % 64 == 0
    );
    USB_BLOCK_DEVICE
        .read(target_lba, 1, &mut ent_buf.data)
        .map_err(|_| "Error leyendo tabla de entradas")?;

    crate::mesa_println!(
        "[DEBUG MKDIR] 3. Buscando slot libre en la tabla (ENTRIES_PER_SECTOR={})...",
        ENTRIES_PER_SECTOR
    );

    // Si la tabla está completamente vacía, usar slot 0 directamente
    let first_type = ent_buf.data[44];
    if ENTRIES_PER_SECTOR > 0 && first_type == 0 {
        // Verificar si TODOS los slots están vacíos (tabla recién inicializada)
        let all_empty = (0..ENTRIES_PER_SECTOR).all(|i| {
            let off = i * core::mem::size_of::<MesaFsFileEntry>();
            ent_buf.data[off + 44] == 0
        });
        if all_empty {
            crate::mesa_println!(
                "[DEBUG MKDIR] 3a. Tabla completamente vacía, usando slot 0 directamente"
            );
            let off = 0usize;
            let mut name_bytes = [0u8; 32];
            let name_slice = entry_name.as_bytes();
            let copy_len = name_slice.len().min(31);
            name_bytes[..copy_len].copy_from_slice(&name_slice[..copy_len]);
            name_bytes[copy_len] = 0;

            ent_buf.data[off..off + 32].copy_from_slice(&name_bytes);
            ent_buf.data[off + 32..off + 36].copy_from_slice(&0u32.to_le_bytes());
            ent_buf.data[off + 36..off + 40].copy_from_slice(&0u32.to_le_bytes());
            ent_buf.data[off + 40..off + 44].copy_from_slice(&0u32.to_le_bytes());
            ent_buf.data[off + 44] = ENTRY_DIR;

            crate::mesa_println!(
                "[DEBUG MKDIR] 4. Escribiendo entrada en disco (LBA {})...",
                target_lba
            );
            USB_BLOCK_DEVICE
                .write(target_lba, 1, &ent_buf.data)
                .map_err(|_| "Error escribiendo tabla de entradas")?;
            crate::mesa_println!("[DEBUG MKDIR] 4a. Entrada escrita OK");

            let mut header_buf = [0u8; SECTOR_SIZE];
            USB_BLOCK_DEVICE
                .read(free_start, 1, &mut header_buf)
                .map_err(|_| "Error leyendo cabecera")?;
            let file_count = u32::from_le_bytes(header_buf[12..16].try_into().unwrap());
            header_buf[12..16].copy_from_slice(&(file_count + 1).to_le_bytes());
            USB_BLOCK_DEVICE
                .write(free_start, 1, &header_buf)
                .map_err(|_| "Error actualizando cabecera")?;
            crate::mesa_println!(
                "[DEBUG MKDIR] 5. Cabecera actualizada, file_count={}",
                file_count + 1
            );

            return Ok(());
        }
    }

    for i in 0..ENTRIES_PER_SECTOR {
        let off = i * core::mem::size_of::<MesaFsFileEntry>();
        let entry_type = ent_buf.data[off + 44];
        if entry_type != ENTRY_UNUSED {
            let mut name_bytes = [0u8; 32];
            name_bytes.copy_from_slice(&ent_buf.data[off..off + 32]);
            let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(32);
            let existing = core::str::from_utf8(&name_bytes[..name_len]).unwrap_or("?");
            if existing == entry_name {
                return Err("La entrada ya existe");
            }
            continue;
        }

        crate::mesa_println!("[DEBUG MKDIR] 3a. Slot libre encontrado en índice {}", i);

        let mut name_bytes = [0u8; 32];
        let name_slice = entry_name.as_bytes();
        let copy_len = name_slice.len().min(31);
        name_bytes[..copy_len].copy_from_slice(&name_slice[..copy_len]);
        name_bytes[copy_len] = 0;

        ent_buf.data[off..off + 32].copy_from_slice(&name_bytes);
        ent_buf.data[off + 32..off + 36].copy_from_slice(&0u32.to_le_bytes());
        ent_buf.data[off + 36..off + 40].copy_from_slice(&0u32.to_le_bytes());
        ent_buf.data[off + 40..off + 44].copy_from_slice(&0u32.to_le_bytes());
        ent_buf.data[off + 44] = ENTRY_DIR;

        crate::mesa_println!(
            "[DEBUG MKDIR] 4. Escribiendo entrada en disco (LBA {}, offset {})...",
            target_lba,
            off
        );
        USB_BLOCK_DEVICE
            .write(target_lba, 1, &ent_buf.data)
            .map_err(|_| "Error escribiendo tabla de entradas")?;
        crate::mesa_println!("[DEBUG MKDIR] 4a. Entrada escrita OK");

        let mut header_buf = [0u8; SECTOR_SIZE];
        USB_BLOCK_DEVICE
            .read(free_start, 1, &mut header_buf)
            .map_err(|_| "Error leyendo cabecera")?;
        let file_count = u32::from_le_bytes(header_buf[12..16].try_into().unwrap());
        header_buf[12..16].copy_from_slice(&(file_count + 1).to_le_bytes());
        USB_BLOCK_DEVICE
            .write(free_start, 1, &header_buf)
            .map_err(|_| "Error actualizando cabecera")?;
        crate::mesa_println!(
            "[DEBUG MKDIR] 5. Cabecera actualizada, file_count={}",
            file_count + 1
        );

        return Ok(());
    }
    crate::mesa_println!("[DEBUG MKDIR] ❌ Todos los slots ocupados");
    Err("Tabla de entradas llena")
}

pub fn mesa_fs_remove(name: &str) -> Result<(), &'static str> {
    let state = MESA_FS_STATE.lock();
    if !state.initialized {
        return Err("MesaFS no inicializado");
    }
    let free_start = state.free_start_lba;
    drop(state);

    let mut ent_buf = AlignedSector {
        data: [0u8; SECTOR_SIZE],
    };
    USB_BLOCK_DEVICE
        .read(free_start + 1, 1, &mut ent_buf.data)
        .map_err(|_| "Error leyendo tabla de entradas")?;

    for i in 0..ENTRIES_PER_SECTOR {
        let off = i * core::mem::size_of::<MesaFsFileEntry>();
        let entry_type = ent_buf.data[off + 44];
        if entry_type == ENTRY_UNUSED {
            continue;
        }
        let mut name_bytes = [0u8; 32];
        name_bytes.copy_from_slice(&ent_buf.data[off..off + 32]);
        let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let existing = core::str::from_utf8(&name_bytes[..name_len]).unwrap_or("?");
        if existing == name {
            ent_buf.data[off + 44] = ENTRY_UNUSED;
            USB_BLOCK_DEVICE
                .write(free_start + 1, 1, &ent_buf.data)
                .map_err(|_| "Error escribiendo tabla de entradas")?;

            let mut header_buf = [0u8; SECTOR_SIZE];
            USB_BLOCK_DEVICE
                .read(free_start, 1, &mut header_buf)
                .map_err(|_| "Error leyendo cabecera")?;
            let file_count = u32::from_le_bytes(header_buf[12..16].try_into().unwrap());
            if file_count > 0 {
                header_buf[12..16].copy_from_slice(&(file_count - 1).to_le_bytes());
                USB_BLOCK_DEVICE
                    .write(free_start, 1, &header_buf)
                    .map_err(|_| "Error actualizando cabecera")?;
            }
            return Ok(());
        }
    }
    Err("Entrada no encontrada")
}
