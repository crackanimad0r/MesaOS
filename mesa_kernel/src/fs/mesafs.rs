//! MesaFS - Filesystem simple y persistente para Mesa OS

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::collections::BTreeMap;
use spin::RwLock;

use super::{DirEntry, FileSystem, FsError, FsResult, Metadata, NodeType};
use crate::drivers::block::{BlockDevice, SECTOR_SIZE};

/// Tamaño de bloque (2 sectores = 1KB)
const BLOCK_SIZE: usize = SECTOR_SIZE * 2;

/// Magic number
const MESAFS_MAGIC: u32 = 0x4D455341; // "MESA"

/// Versión del filesystem
const MESAFS_VERSION: u16 = 1;

/// Número máximo de inodos
const MAX_INODES: u32 = 256;

/// Entradas por bloque de directorio
const DIR_ENTRIES_PER_BLOCK: usize = BLOCK_SIZE / 32;

/// Superblock (512 bytes = 1 sector)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Superblock {
    magic: u32,
    version: u16,
    block_size: u16,
    total_blocks: u32,
    total_inodes: u32,
    free_blocks: u32,
    free_inodes: u32,
    first_data_block: u32,
    bitmap_blocks: u32,
    inode_blocks: u32,
    root_inode: u32,
    next_free_inode: u32,
    _padding: [u8; 468],
}

impl Superblock {
    fn new(total_blocks: u32) -> Self {
        let bitmap_blocks = ((total_blocks + 7) / 8 + BLOCK_SIZE as u32 - 1) / BLOCK_SIZE as u32;
        let inode_size = core::mem::size_of::<Inode>();
        let inodes_per_block = BLOCK_SIZE / inode_size;
        let inode_blocks = (MAX_INODES + inodes_per_block as u32 - 1) / inodes_per_block as u32;
        let first_data_block = 1 + bitmap_blocks + inode_blocks;
        
        Self {
            magic: MESAFS_MAGIC,
            version: MESAFS_VERSION,
            block_size: BLOCK_SIZE as u16,
            total_blocks,
            total_inodes: MAX_INODES,
            free_blocks: total_blocks - first_data_block,
            free_inodes: MAX_INODES - 1,
            first_data_block,
            bitmap_blocks,
            inode_blocks,
            root_inode: 0,
            next_free_inode: 1,
            _padding: [0; 468],
        }
    }
}

/// Tipo de inodo
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum InodeType {
    Free = 0,
    File = 1,
    Directory = 2,
}

/// Inodo (128 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Inode {
    inode_type: u8,
    permissions: u8,
    uid: u32,
    gid: u32,
    size: u32,
    blocks: u32,
    created: u64,
    modified: u64,
    direct_blocks: [u32; 12],
    indirect_block: u32,
    double_indirect: u32,
    _padding: [u8; 44],
}

impl Inode {
    fn new_empty() -> Self {
        Self {
            inode_type: InodeType::Free as u8,
            permissions: 0,
            uid: 0,
            gid: 0,
            size: 0,
            blocks: 0,
            created: 0,
            modified: 0,
            direct_blocks: [0; 12],
            indirect_block: 0,
            double_indirect: 0,
            _padding: [0; 44],
        }
    }
    
    fn new_file(uid: u32, gid: u32) -> Self {
        let now = crate::curr_arch::get_ticks();
        Self {
            inode_type: InodeType::File as u8,
            permissions: 0b0110_0100,
            uid,
            gid,
            size: 0,
            blocks: 0,
            created: now,
            modified: now,
            direct_blocks: [0; 12],
            indirect_block: 0,
            double_indirect: 0,
            _padding: [0; 44],
        }
    }
    
    fn new_dir(uid: u32, gid: u32) -> Self {
        let now = crate::curr_arch::get_ticks();
        Self {
            inode_type: InodeType::Directory as u8,
            permissions: 0b0111_0101,
            uid,
            gid,
            size: 0,
            blocks: 0,
            created: now,
            modified: now,
            direct_blocks: [0; 12],
            indirect_block: 0,
            double_indirect: 0,
            _padding: [0; 44],
        }
    }
}

/// Entrada de directorio (32 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct DirEntryDisk {
    inode: u32,
    entry_type: u8,
    name_len: u8,
    name: [u8; 26],
}

impl DirEntryDisk {
    fn new(inode: u32, entry_type: InodeType, name: &str) -> Self {
        let mut name_bytes = [0u8; 26];
        let len = core::cmp::min(name.len(), 26);
        name_bytes[..len].copy_from_slice(&name.as_bytes()[..len]);
        
        Self {
            inode,
            entry_type: entry_type as u8,
            name_len: len as u8,
            name: name_bytes,
        }
    }
    
    fn name_str(&self) -> &str {
        let len = self.name_len as usize;
        core::str::from_utf8(&self.name[..len]).unwrap_or("")
    }
    
    fn is_free(&self) -> bool {
        self.inode == 0
    }
}

/// MesaFS
pub struct MesaFs {
    superblock: RwLock<Superblock>,
    bitmap: RwLock<Vec<u8>>,
    inode_bitmap: RwLock<Vec<bool>>,
    start_lba: u64,
    device: alloc::sync::Arc<dyn BlockDevice>,
}

impl MesaFs {
    pub fn create(start_lba: u64, size_blocks: u32) -> Result<Self, &'static str> {
        Self::create_on_dev(alloc::sync::Arc::new(crate::drivers::ata::AtaBlockDevice), start_lba, size_blocks)
    }

    pub fn create_on_dev(dev: alloc::sync::Arc<dyn BlockDevice>, start_lba: u64, size_blocks: u32) -> Result<Self, &'static str> {
        crate::serial_println!("[MESAFS] Creating filesystem...");
        
        let sb = Superblock::new(size_blocks);
        
        // Escribir superblock
        let sb_bytes = unsafe {
            core::slice::from_raw_parts(
                &sb as *const _ as *const u8,
                core::mem::size_of::<Superblock>()
            )
        };
        let mut sector = [0u8; SECTOR_SIZE];
        sector[..sb_bytes.len()].copy_from_slice(sb_bytes);
        dev.write(start_lba, 1, &sector)?;
        
        // Crear bitmap de bloques
        let bitmap_size = ((size_blocks + 7) / 8) as usize;
        let mut bitmap = vec![0u8; bitmap_size];
        
        // Marcar bloques del sistema como usados
        for i in 0..sb.first_data_block {
            let byte = (i / 8) as usize;
            let bit = i % 8;
            bitmap[byte] |= 1 << bit;
        }
        
        // Escribir bitmap
        let sectors_needed = (bitmap_size + SECTOR_SIZE - 1) / SECTOR_SIZE;
        let mut bitmap_buffer = vec![0u8; sectors_needed * SECTOR_SIZE];
        bitmap_buffer[..bitmap.len()].copy_from_slice(&bitmap);
        dev.write(start_lba + 1, sectors_needed as u16, &bitmap_buffer)?;
        
        // Crear inodos vacíos
        let inode_size = core::mem::size_of::<Inode>();
        let inodes_per_sector = SECTOR_SIZE / inode_size;
        let total_inode_sectors = (MAX_INODES as usize + inodes_per_sector - 1) / inodes_per_sector;
        
        let mut sector = [0u8; SECTOR_SIZE];
        for i in 0..total_inode_sectors {
            dev.write(start_lba + 1 + sb.bitmap_blocks as u64 + i as u64, 1, &sector)?;
        }
        
        // Crear inodo root
        let root = Inode::new_dir(0, 0);
        let root_bytes = unsafe {
            core::slice::from_raw_parts(&root as *const _ as *const u8, inode_size)
        };
        sector[..inode_size].copy_from_slice(root_bytes);
        dev.write(start_lba + 1 + sb.bitmap_blocks as u64, 1, &sector)?;
        
        // Crear bitmap de inodos en memoria
        let mut inode_bitmap = vec![false; MAX_INODES as usize];
        inode_bitmap[0] = true;
        
        crate::serial_println!("[MESAFS] Filesystem created ({} blocks, {} inodes)", size_blocks, MAX_INODES);
        
        Ok(Self {
            superblock: RwLock::new(sb),
            bitmap: RwLock::new(bitmap),
            inode_bitmap: RwLock::new(inode_bitmap),
            start_lba,
            device: dev,
        })
    }

    pub fn mount(start_lba: u64) -> Result<Self, &'static str> {
        Self::mount_on_dev(alloc::sync::Arc::new(crate::drivers::ata::AtaBlockDevice), start_lba)
    }

    pub fn mount_on_dev(dev: alloc::sync::Arc<dyn BlockDevice>, start_lba: u64) -> Result<Self, &'static str> {
        crate::serial_println!("[MESAFS] Mounting filesystem...");
        
        // Leer superblock
        let mut sector = [0u8; SECTOR_SIZE];
        dev.read(start_lba, 1, &mut sector)?;
        
        let sb: Superblock = unsafe { core::ptr::read(sector.as_ptr() as *const Superblock) };
        
        if sb.magic != MESAFS_MAGIC {
            return Err("Invalid MesaFS magic");
        }
        
        if sb.version != MESAFS_VERSION {
            return Err("Unsupported MesaFS version");
        }
        
        let free_blocks = sb.free_blocks;
        let free_inodes = sb.free_inodes;
        
        // Leer bitmap de bloques
        let bitmap_size = ((sb.total_blocks + 7) / 8) as usize;
        let sectors_needed = (bitmap_size + SECTOR_SIZE - 1) / SECTOR_SIZE;
        
        let mut bitmap_buffer = vec![0u8; sectors_needed * SECTOR_SIZE];
        dev.read(start_lba + 1, sectors_needed as u16, &mut bitmap_buffer)?;
        
        let mut bitmap = vec![0u8; bitmap_size];
        bitmap.copy_from_slice(&bitmap_buffer[..bitmap_size]);
        
        // Construir bitmap de inodos en memoria
        crate::serial_println!("[MESAFS] Building inode bitmap...");
        let mut inode_bitmap = vec![false; MAX_INODES as usize];
        
        let inode_size = core::mem::size_of::<Inode>();
        let inodes_per_sector = SECTOR_SIZE / inode_size;
        
        for sector_num in 0..sb.inode_blocks as usize {
            let sector_lba = start_lba + 1 + sb.bitmap_blocks as u64 + sector_num as u64;
            dev.read(sector_lba, 1, &mut sector)?;
            
            for i in 0..inodes_per_sector {
                let inode_num = sector_num * inodes_per_sector + i;
                if inode_num >= MAX_INODES as usize {
                    break;
                }
                
                let inode_ptr = unsafe {
                    sector.as_ptr().add(i * inode_size) as *const Inode
                };
                let inode = unsafe { *inode_ptr };
                
                inode_bitmap[inode_num] = inode.inode_type != InodeType::Free as u8;
            }
        }
        
        crate::serial_println!("[MESAFS] Mounted ({} free blocks, {} free inodes)", free_blocks, free_inodes);
        
        Ok(Self {
            superblock: RwLock::new(sb),
            bitmap: RwLock::new(bitmap),
            inode_bitmap: RwLock::new(inode_bitmap),
            start_lba,
            device: dev,
        })
    }
    
    /// Lee un inodo del disco
    fn read_inode(&self, inode_num: u32) -> Result<Inode, &'static str> {
        if inode_num >= MAX_INODES {
            return Err("Invalid inode number");
        }
        
        let sb = self.superblock.read();
        let inode_size = core::mem::size_of::<Inode>();
        let inodes_per_sector = SECTOR_SIZE / inode_size;
        
        let sector_num = (inode_num as usize) / inodes_per_sector;
        let inode_offset = (inode_num as usize) % inodes_per_sector;
        
        let sector_lba = self.start_lba + 1 + sb.bitmap_blocks as u64 + sector_num as u64;
        
        let mut sector = [0u8; SECTOR_SIZE];
        self.device.read(sector_lba, 1, &mut sector)?;
        
        let inode_ptr = unsafe {
            sector.as_ptr().add(inode_offset * inode_size) as *const Inode
        };
        
        Ok(unsafe { *inode_ptr })
    }
    
    /// Escribe un inodo al disco
    fn write_inode(&self, inode_num: u32, inode: &Inode) -> Result<(), &'static str> {
        if inode_num >= MAX_INODES {
            return Err("Invalid inode number");
        }
        
        let sb = self.superblock.read();
        let inode_size = core::mem::size_of::<Inode>();
        let inodes_per_sector = SECTOR_SIZE / inode_size;
        
        let sector_num = (inode_num as usize) / inodes_per_sector;
        let inode_offset = (inode_num as usize) % inodes_per_sector;
        
        let sector_lba = self.start_lba + 1 + sb.bitmap_blocks as u64 + sector_num as u64;
        
        let mut sector = [0u8; SECTOR_SIZE];
        self.device.read(sector_lba, 1, &mut sector)?;
        
        let inode_bytes = unsafe {
            core::slice::from_raw_parts(inode as *const _ as *const u8, inode_size)
        };
        
        let dest = &mut sector[inode_offset * inode_size..][..inode_size];
        dest.copy_from_slice(inode_bytes);
        
        self.device.write(sector_lba, 1, &sector)?;
        
        // Actualizar bitmap en memoria
        self.inode_bitmap.write()[inode_num as usize] = inode.inode_type != InodeType::Free as u8;
        
        Ok(())
    }
    
    /// Allocar un inodo (OPTIMIZADO)
    fn alloc_inode(&self) -> Result<u32, FsError> {
        crate::serial_println!("[MESAFS] Allocating inode...");
        
        let mut sb = self.superblock.write();
        let mut inode_bitmap = self.inode_bitmap.write();
        
        if sb.free_inodes == 0 {
            crate::serial_println!("[MESAFS] No free inodes!");
            return Err(FsError::NoSpace);
        }
        
        // Buscar en el bitmap en memoria (RÁPIDO)
        for i in 1..MAX_INODES as usize {
            if !inode_bitmap[i] {
                crate::serial_println!("[MESAFS] Found free inode: {}", i);
                inode_bitmap[i] = true;
                sb.free_inodes -= 1;
                sb.next_free_inode = (i + 1) as u32;
                return Ok(i as u32);
            }
        }
        
        crate::serial_println!("[MESAFS] No free inodes found!");
        Err(FsError::NoSpace)
    }
    
    /// Allocar un bloque
    fn alloc_block(&self) -> Result<u32, FsError> {
        let mut bitmap = self.bitmap.write();
        let mut sb = self.superblock.write();
        
        if sb.free_blocks == 0 {
            return Err(FsError::NoSpace);
        }
        
        for i in sb.first_data_block..sb.total_blocks.min(sb.first_data_block + 1000) {
            let byte = (i / 8) as usize;
            let bit = i % 8;
            
            if byte >= bitmap.len() {
                break;
            }
            
            if bitmap[byte] & (1 << bit) == 0 {
                bitmap[byte] |= 1 << bit;
                sb.free_blocks -= 1;
                
                // Escribir bitmap actualizado (solo el sector afectado)
                let sector_num = byte / SECTOR_SIZE;
                let sector_start = sector_num * SECTOR_SIZE;
                let sector_end = core::cmp::min(sector_start + SECTOR_SIZE, bitmap.len());
                
                let mut sector = [0u8; SECTOR_SIZE];
                sector[..sector_end - sector_start].copy_from_slice(&bitmap[sector_start..sector_end]);
                let _ = self.device.write(self.start_lba + 1 + sector_num as u64, 1, &sector);
                
                return Ok(i);
            }
        }
        
        Err(FsError::NoSpace)
    }
    
    /// Libera un bloque
    fn free_block(&self, block_num: u32) {
        let mut bitmap = self.bitmap.write();
        let mut sb = self.superblock.write();
        
        let byte = (block_num / 8) as usize;
        let bit = block_num % 8;
        
        if byte < bitmap.len() && bitmap[byte] & (1 << bit) != 0 {
            bitmap[byte] &= !(1 << bit);
            sb.free_blocks += 1;
            
            // Escribir bitmap actualizado al disco
            let sector_num = byte / SECTOR_SIZE;
            let sector_start = sector_num * SECTOR_SIZE;
            let sector_end = core::cmp::min(sector_start + SECTOR_SIZE, bitmap.len());
            
            let mut sector = [0u8; SECTOR_SIZE];
            sector[..sector_end - sector_start].copy_from_slice(&bitmap[sector_start..sector_end]);
            let _ = self.device.write(self.start_lba + 1 + sector_num as u64, 1, &sector);
            
            crate::serial_println!("[MESAFS] Block {} freed", block_num);
        }
    }
    
    /// Flushea el superblock al disco
    fn flush_superblock(&self) -> Result<(), FsError> {
        let sb = self.superblock.read();
        let sb_bytes = unsafe {
            core::slice::from_raw_parts(
                &*sb as *const _ as *const u8,
                core::mem::size_of::<Superblock>()
            )
        };
        let mut sector = [0u8; SECTOR_SIZE];
        sector[..sb_bytes.len()].copy_from_slice(sb_bytes);
        self.device.write(self.start_lba, 1, &sector).map_err(|_| FsError::IoError)?;
        Ok(())
    }
    
    /// Busca un archivo en un directorio
    fn find_in_dir(&self, dir_inode: u32, name: &str) -> Result<u32, FsError> {
        let inode = self.read_inode(dir_inode).map_err(|_| FsError::IoError)?;
        
        if inode.inode_type != InodeType::Directory as u8 {
            return Err(FsError::NotADirectory);
        }
        
        for i in 0..12 {
            let block_num = inode.direct_blocks[i];
            if block_num == 0 {
                break;
            }
            
            let mut block = vec![0u8; BLOCK_SIZE];
            self.read_block(block_num, &mut block)?;
            
            for j in 0..DIR_ENTRIES_PER_BLOCK {
                let entry_offset = j * 32;
                let entry = unsafe {
                    *(block.as_ptr().add(entry_offset) as *const DirEntryDisk)
                };
                
                if !entry.is_free() && entry.name_str() == name {
                    return Ok(entry.inode);
                }
            }
        }
        
        Err(FsError::NotFound)
    }
    
    /// Agrega una entrada a un directorio
    fn add_to_dir(&self, dir_inode: u32, name: &str, target_inode: u32, entry_type: InodeType) -> Result<(), FsError> {
        crate::serial_println!("[MESAFS] Adding '{}' to dir inode {}", name, dir_inode);
        
        let mut dir = self.read_inode(dir_inode).map_err(|_| FsError::IoError)?;
        
        if dir.inode_type != InodeType::Directory as u8 {
            return Err(FsError::NotADirectory);
        }
        
        // Buscar espacio en los bloques existentes
        for i in 0..12 {
            let block_num = dir.direct_blocks[i];
            
            if block_num == 0 {
                // Allocar nuevo bloque
                crate::serial_println!("[MESAFS] Allocating new block for directory");
                let new_block = self.alloc_block()?;
                dir.direct_blocks[i] = new_block;
                dir.blocks += 1;
                
                // Crear entrada
                let entry = DirEntryDisk::new(target_inode, entry_type, name);
                let mut block = vec![0u8; BLOCK_SIZE];
                
                unsafe {
                    let ptr = block.as_mut_ptr() as *mut DirEntryDisk;
                    *ptr = entry;
                }
                
                self.write_block(new_block, &block)?;
                self.write_inode(dir_inode, &dir).map_err(|_| FsError::IoError)?;
                crate::serial_println!("[MESAFS] Entry added successfully");
                return Ok(());
            }
            
            // Leer bloque existente
            let mut block = vec![0u8; BLOCK_SIZE];
            self.read_block(block_num, &mut block)?;
            
            // Buscar entrada libre
            for j in 0..DIR_ENTRIES_PER_BLOCK {
                let entry_offset = j * 32;
                let entry = unsafe {
                    &mut *(block.as_mut_ptr().add(entry_offset) as *mut DirEntryDisk)
                };
                
                if entry.is_free() {
                    *entry = DirEntryDisk::new(target_inode, entry_type, name);
                    self.write_block(block_num, &block)?;
                    crate::serial_println!("[MESAFS] Entry added to existing block");
                    return Ok(());
                }
            }
        }
        
        Err(FsError::NoSpace)
    }
    
    /// Elimina una entrada de un directorio
    fn remove_from_dir(&self, dir_inode: u32, name: &str) -> Result<(), FsError> {
        crate::serial_println!("[MESAFS] Removing '{}' from dir inode {}", name, dir_inode);
        
        let dir = self.read_inode(dir_inode).map_err(|_| FsError::IoError)?;
        
        if dir.inode_type != InodeType::Directory as u8 {
            return Err(FsError::NotADirectory);
        }
        
        // Buscar la entrada en los bloques del directorio
        for i in 0..12 {
            let block_num = dir.direct_blocks[i];
            if block_num == 0 {
                break;
            }
            
            let mut block = vec![0u8; BLOCK_SIZE];
            self.read_block(block_num, &mut block)?;
            
            for j in 0..DIR_ENTRIES_PER_BLOCK {
                let entry_offset = j * 32;
                let entry = unsafe {
                    &mut *(block.as_mut_ptr().add(entry_offset) as *mut DirEntryDisk)
                };
                
                if !entry.is_free() && entry.name_str() == name {
                    // Limpiar la entrada (marcar como libre)
                    entry.inode = 0;
                    entry.entry_type = 0;
                    entry.name_len = 0;
                    entry.name = [0u8; 26];
                    
                    // Escribir bloque actualizado
                    self.write_block(block_num, &block)?;
                    
                    crate::serial_println!("[MESAFS] Entry removed from directory");
                    return Ok(());
                }
            }
        }
        
        Err(FsError::NotFound)
    }
    
    /// Lee un bloque
    fn read_block(&self, block_num: u32, buffer: &mut [u8]) -> Result<(), FsError> {
        if buffer.len() < BLOCK_SIZE {
            return Err(FsError::IoError);
        }
        
        let sectors_per_block = BLOCK_SIZE / SECTOR_SIZE;
        let lba = self.start_lba + (block_num as u64) * sectors_per_block as u64;
        
        self.device.read(lba, sectors_per_block as u16, buffer).map_err(|_| FsError::IoError)?;
        Ok(())
    }
    
    /// Escribe un bloque
    fn write_block(&self, block_num: u32, data: &[u8]) -> Result<(), FsError> {
        if data.len() > BLOCK_SIZE {
            return Err(FsError::IoError);
        }
        
        let mut buffer = vec![0u8; BLOCK_SIZE];
        buffer[..data.len()].copy_from_slice(data);
        
        let sectors_per_block = BLOCK_SIZE / SECTOR_SIZE;
        let lba = self.start_lba + (block_num as u64) * sectors_per_block as u64;
        
        self.device.write(lba, sectors_per_block as u16, &buffer).map_err(|_| FsError::IoError)?;
        Ok(())
    }
    
    /// Resuelve un path a un inodo
    fn resolve_path(&self, path: &str) -> Result<u32, FsError> {
        if path == "/" {
            return Ok(0);
        }
        
        let components: Vec<&str> = path.split('/')
            .filter(|s| !s.is_empty())
            .collect();
        
        let mut current_inode = 0;
        
        for component in components {
            current_inode = self.find_in_dir(current_inode, component)?;
        }
        
        Ok(current_inode)
    }
    pub fn stats(&self) -> (u32, u32) {
        let sb = self.superblock.read();
        (sb.total_blocks - sb.free_blocks, sb.total_blocks)
    }
}

impl FileSystem for MesaFs {
    fn stats(&self) -> (u64, u64) {
        let (used, total) = self.stats();
        (used as u64, total as u64)
    }
    
    fn name(&self) -> &str {
        "mesafs"
    }
    
    fn stat(&self, path: &str) -> FsResult<Metadata> {
        let inode_num = self.resolve_path(path)?;
        let inode = self.read_inode(inode_num).map_err(|_| FsError::IoError)?;
        
        let node_type = match inode.inode_type {
            1 => NodeType::File,
            2 => NodeType::Directory,
            _ => return Err(FsError::NotFound),
        };
        
        Ok(Metadata {
            node_type,
            size: inode.size as u64,
            owner_uid: inode.uid,
            owner_gid: inode.gid,
            permissions: super::Permissions::all(),
            created: inode.created,
            modified: inode.modified,
        })
    }
    
    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        let inode_num = self.resolve_path(path)?;
        let inode = self.read_inode(inode_num).map_err(|_| FsError::IoError)?;
        
        if inode.inode_type != InodeType::Directory as u8 {
            return Err(FsError::NotADirectory);
        }
        
        let mut entries = Vec::new();
        
        for i in 0..12 {
            let block_num = inode.direct_blocks[i];
            if block_num == 0 {
                break;
            }
            
            let mut block = vec![0u8; BLOCK_SIZE];
            self.read_block(block_num, &mut block)?;
            
            for j in 0..DIR_ENTRIES_PER_BLOCK {
                let entry_offset = j * 32;
                let entry = unsafe {
                    *(block.as_ptr().add(entry_offset) as *const DirEntryDisk)
                };
                
                if !entry.is_free() {
                    let entry_inode = self.read_inode(entry.inode).map_err(|_| FsError::IoError)?;
                    
                    let node_type = match entry_inode.inode_type {
                        1 => NodeType::File,
                        2 => NodeType::Directory,
                        _ => continue,
                    };
                    
                    entries.push(DirEntry {
                        name: String::from(entry.name_str()),
                        node_type,
                        size: entry_inode.size as u64,
                    });
                }
            }
        }
        
        Ok(entries)
    }
    
    fn read(&self, path: &str) -> FsResult<Vec<u8>> {
        let inode_num = self.resolve_path(path)?;
        let inode = self.read_inode(inode_num).map_err(|_| FsError::IoError)?;
        
        if inode.inode_type != InodeType::File as u8 {
            return Err(FsError::IsADirectory);
        }
        
        let mut data = Vec::with_capacity(inode.size as usize);
        
        for i in 0..12 {
            if data.len() >= inode.size as usize {
                break;
            }
            
            let block_num = inode.direct_blocks[i];
            if block_num == 0 {
                break;
            }
            
            let mut block = vec![0u8; BLOCK_SIZE];
            self.read_block(block_num, &mut block)?;
            
            let to_read = core::cmp::min(BLOCK_SIZE, inode.size as usize - data.len());
            data.extend_from_slice(&block[..to_read]);
        }
        
        Ok(data)
    }
    
    fn write(&self, path: &str, data: &[u8]) -> FsResult<()> {
        crate::serial_println!("[MESAFS] Writing {} bytes to '{}'", data.len(), path);
        
        let inode_num = match self.resolve_path(path) {
            Ok(num) => num,
            Err(FsError::NotFound) => {
                crate::serial_println!("[MESAFS] File not found, creating...");
                self.create(path)?;
                self.resolve_path(path)?
            }
            Err(e) => return Err(e),
        };
        
        let mut inode = self.read_inode(inode_num).map_err(|_| FsError::IoError)?;
        
        if inode.inode_type != InodeType::File as u8 {
            return Err(FsError::IsADirectory);
        }
        
        let blocks_needed = (data.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;
        
        for i in 0..core::cmp::min(blocks_needed, 12) {
            if inode.direct_blocks[i] == 0 {
                inode.direct_blocks[i] = self.alloc_block()?;
                inode.blocks += 1;
            }
            
            let offset = i * BLOCK_SIZE;
            let end = core::cmp::min(offset + BLOCK_SIZE, data.len());
            
            self.write_block(inode.direct_blocks[i], &data[offset..end])?;
        }
        
        inode.size = data.len() as u32;
        inode.modified = crate::curr_arch::get_ticks();
        
        self.write_inode(inode_num, &inode).map_err(|_| FsError::IoError)?;
        
        self.flush_superblock()?;
        crate::serial_println!("[MESAFS] Write complete");
        
        Ok(())
    }
    
    fn mkdir(&self, path: &str) -> FsResult<()> {
        crate::serial_println!("[MESAFS] Creating directory '{}'", path);
        
        let pos = path.rfind('/').ok_or(FsError::InvalidPath)?;
        let (parent_path, name) = if pos == 0 {
            ("/", &path[1..])
        } else {
            (&path[..pos], &path[pos + 1..])
        };
        
        if name.is_empty() {
            return Err(FsError::InvalidPath);
        }
        
        if self.resolve_path(path).is_ok() {
            return Err(FsError::AlreadyExists);
        }
        
        let parent_inode = self.resolve_path(parent_path)?;
        
        let new_inode_num = self.alloc_inode()?;
        let new_inode = Inode::new_dir(0, 0);
        
        self.write_inode(new_inode_num, &new_inode).map_err(|_| FsError::IoError)?;
        self.add_to_dir(parent_inode, name, new_inode_num, InodeType::Directory)?;
        
        self.flush_superblock()?;
        crate::serial_println!("[MESAFS] Directory created");
        Ok(())
    }
    
    fn create(&self, path: &str) -> FsResult<()> {
        crate::serial_println!("[MESAFS] Creating file '{}'", path);
        
        let pos = path.rfind('/').ok_or(FsError::InvalidPath)?;
        let (parent_path, name) = if pos == 0 {
            ("/", &path[1..])
        } else {
            (&path[..pos], &path[pos + 1..])
        };
        
        if name.is_empty() {
            return Err(FsError::InvalidPath);
        }
        
        if self.resolve_path(path).is_ok() {
            return Ok(());
        }
        
        let parent_inode = self.resolve_path(parent_path)?;
        
        let new_inode_num = self.alloc_inode()?;
        let new_inode = Inode::new_file(0, 0);
        
        self.write_inode(new_inode_num, &new_inode).map_err(|_| FsError::IoError)?;
        self.add_to_dir(parent_inode, name, new_inode_num, InodeType::File)?;
        
        self.flush_superblock()?;
        crate::serial_println!("[MESAFS] File created");
        Ok(())
    }
    
    fn remove(&self, path: &str) -> FsResult<()> {
        crate::serial_println!("[MESAFS] Removing file '{}'", path);
        
        // No permitir eliminar root
        if path == "/" {
            return Err(FsError::PermissionDenied);
        }
        
        // Parsear path
        let pos = path.rfind('/').ok_or(FsError::InvalidPath)?;
        let (parent_path, name) = if pos == 0 {
            ("/", &path[1..])
        } else {
            (&path[..pos], &path[pos + 1..])
        };
        
        if name.is_empty() {
            return Err(FsError::InvalidPath);
        }
        
        // Verificar que el archivo existe y obtener su inodo
        let file_inode_num = self.resolve_path(path)?;
        let file_inode = self.read_inode(file_inode_num).map_err(|_| FsError::IoError)?;
        
        // Verificar que es un archivo, no directorio
        if file_inode.inode_type != InodeType::File as u8 {
            return Err(FsError::IsADirectory);
        }
        
        // Liberar los bloques de datos del archivo
        for i in 0..12 {
            let block_num = file_inode.direct_blocks[i];
            if block_num != 0 {
                self.free_block(block_num);
            }
        }
        
        // Marcar inodo como libre
        let empty_inode = Inode::new_empty();
        self.write_inode(file_inode_num, &empty_inode).map_err(|_| FsError::IoError)?;
        
        // Actualizar bitmap de inodos
        {
            let mut inode_bitmap = self.inode_bitmap.write();
            inode_bitmap[file_inode_num as usize] = false;
        }
        
        // Actualizar superblock
        {
            let mut sb = self.superblock.write();
            sb.free_inodes += 1;
        }
        
        // Eliminar entrada del directorio padre
        let parent_inode_num = self.resolve_path(parent_path)?;
        self.remove_from_dir(parent_inode_num, name)?;
        
        self.flush_superblock()?;
        crate::serial_println!("[MESAFS] File removed successfully");
        Ok(())
    }
    
    fn rmdir(&self, path: &str) -> FsResult<()> {
        crate::serial_println!("[MESAFS] Removing directory '{}'", path);
        
        // No permitir eliminar root
        if path == "/" {
            return Err(FsError::PermissionDenied);
        }
        
        // Parsear path
        let pos = path.rfind('/').ok_or(FsError::InvalidPath)?;
        let (parent_path, name) = if pos == 0 {
            ("/", &path[1..])
        } else {
            (&path[..pos], &path[pos + 1..])
        };
        
        if name.is_empty() {
            return Err(FsError::InvalidPath);
        }
        
        // Verificar que el directorio existe
        let dir_inode_num = self.resolve_path(path)?;
        let dir_inode = self.read_inode(dir_inode_num).map_err(|_| FsError::IoError)?;
        
        // Verificar que es un directorio
        if dir_inode.inode_type != InodeType::Directory as u8 {
            return Err(FsError::NotADirectory);
        }
        
        // Verificar que está vacío
        let entries = self.readdir(path)?;
        if !entries.is_empty() {
            return Err(FsError::NotEmpty);
        }
        
        // Liberar los bloques del directorio
        for i in 0..12 {
            let block_num = dir_inode.direct_blocks[i];
            if block_num != 0 {
                self.free_block(block_num);
            }
        }
        
        // Marcar inodo como libre
        let empty_inode = Inode::new_empty();
        self.write_inode(dir_inode_num, &empty_inode).map_err(|_| FsError::IoError)?;
        
        // Actualizar bitmap de inodos
        {
            let mut inode_bitmap = self.inode_bitmap.write();
            inode_bitmap[dir_inode_num as usize] = false;
        }
        
        // Actualizar superblock
        {
            let mut sb = self.superblock.write();
            sb.free_inodes += 1;
        }
        
        // Eliminar entrada del directorio padre
        let parent_inode_num = self.resolve_path(parent_path)?;
        self.remove_from_dir(parent_inode_num, name)?;
        
        self.flush_superblock()?;
        crate::serial_println!("[MESAFS] Directory removed successfully");
        Ok(())
    }
    
    fn rename(&self, from: &str, to: &str) -> FsResult<()> {
        crate::serial_println!("[MESAFS] Renaming '{}' to '{}'", from, to);
        
        // Verificar que origen existe
        let from_inode_num = self.resolve_path(from)?;
        let from_inode = self.read_inode(from_inode_num).map_err(|_| FsError::IoError)?;
        
        // No permitir mover directorios (simplificación)
        if from_inode.inode_type == InodeType::Directory as u8 {
            return Err(FsError::IsADirectory);
        }
        
        // Parsear destino
        let pos = to.rfind('/').ok_or(FsError::InvalidPath)?;
        let (dest_parent_path, dest_name) = if pos == 0 {
            ("/", &to[1..])
        } else {
            (&to[..pos], &to[pos + 1..])
        };
        
        if dest_name.is_empty() {
            return Err(FsError::InvalidPath);
        }
        
        // Verificar que destino no existe
        if self.resolve_path(to).is_ok() {
            return Err(FsError::AlreadyExists);
        }
        
        // Parsear origen
        let pos = from.rfind('/').ok_or(FsError::InvalidPath)?;
        let (src_parent_path, src_name) = if pos == 0 {
            ("/", &from[1..])
        } else {
            (&from[..pos], &from[pos + 1..])
        };
        
        // Obtener directorio padre origen
        let src_parent_inode = self.resolve_path(src_parent_path)?;
        
        // Obtener directorio padre destino
        let dest_parent_inode = self.resolve_path(dest_parent_path)?;
        
        // Eliminar del directorio origen
        self.remove_from_dir(src_parent_inode, src_name)?;
        
        // Agregar al directorio destino
        let entry_type = if from_inode.inode_type == InodeType::File as u8 {
            InodeType::File
        } else {
            InodeType::Directory
        };
        
        self.add_to_dir(dest_parent_inode, dest_name, from_inode_num, entry_type)?;
        
        self.flush_superblock()?;
        crate::serial_println!("[MESAFS] Rename successful");
        Ok(())
    }
    
    fn utime(&self, path: &str, created: u64, modified: u64) -> FsResult<()> {
        let inode_num = self.resolve_path(path)?;
        let mut inode = self.read_inode(inode_num).map_err(|_| FsError::IoError)?;
        inode.created = created;
        inode.modified = modified;
        self.write_inode(inode_num, &inode).map_err(|_| FsError::IoError)?;
        Ok(())
    }
}

unsafe impl Send for MesaFs {}
unsafe impl Sync for MesaFs {}