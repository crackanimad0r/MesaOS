//! Inyecta un archivo en disk.img con formato MesaFS (Mesa OS).
//!
//! Uso: mesafs_inject <disk.img> <archivo_local> <ruta_en_disco>
//! Ejemplo: mesafs_inject disk.img hello.elf /bin/hello.elf

use std::env;
use std::fs;
use std::io::{Read, Seek, Write};

const SECTOR_SIZE: usize = 512;
const BLOCK_SIZE: usize = SECTOR_SIZE * 2;
const MESAFS_MAGIC: u32 = 0x4D455341;
const MAX_INODES: u32 = 256;
const DIR_ENTRIES_PER_BLOCK: usize = BLOCK_SIZE / 32;

#[repr(C, packed)]
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

#[repr(C, packed)]
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

#[repr(C, packed)]
struct DirEntryDisk {
    inode: u32,
    entry_type: u8,
    name_len: u8,
    name: [u8; 26],
}

fn read_superblock(file: &mut fs::File) -> std::io::Result<Superblock> {
    let mut sector = [0u8; SECTOR_SIZE];
    file.rewind()?;
    file.read_exact(&mut sector)?;
    Ok(unsafe { std::ptr::read(sector.as_ptr() as *const Superblock) })
}

fn read_sector(file: &mut fs::File, sector_idx: u64, buf: &mut [u8; SECTOR_SIZE]) -> std::io::Result<()> {
    file.seek(std::io::SeekFrom::Start(sector_idx * SECTOR_SIZE as u64))?;
    file.read_exact(buf)
}

fn write_sector(file: &mut fs::File, sector_idx: u64, buf: &[u8; SECTOR_SIZE]) -> std::io::Result<()> {
    file.seek(std::io::SeekFrom::Start(sector_idx * SECTOR_SIZE as u64))?;
    file.write_all(buf)
}

fn read_block(file: &mut fs::File, block_num: u32, buf: &mut [u8]) -> std::io::Result<()> {
    let sector0 = (block_num as u64) * 2;
    for i in 0usize..2 {
        let mut sector = [0u8; SECTOR_SIZE];
        read_sector(file, sector0 + i as u64, &mut sector)?;
        buf[i * SECTOR_SIZE..(i + 1) * SECTOR_SIZE].copy_from_slice(&sector);
    }
    Ok(())
}

fn write_block(file: &mut fs::File, block_num: u32, data: &[u8]) -> std::io::Result<()> {
    let sector0 = (block_num as u64) * 2;
    for i in 0usize..2 {
        let mut sector = [0u8; SECTOR_SIZE];
        let start = i * SECTOR_SIZE;
        let end = std::cmp::min(start + SECTOR_SIZE, data.len());
        sector[..end - start].copy_from_slice(&data[start..end]);
        write_sector(file, sector0 + i as u64, &sector)?;
    }
    Ok(())
}

fn read_inode(file: &mut fs::File, sb: &Superblock, inode_num: u32) -> std::io::Result<Inode> {
    let inode_size = std::mem::size_of::<Inode>();
    let inodes_per_sector = SECTOR_SIZE / inode_size;
    let sector_num = 1 + (sb.bitmap_blocks as u64) * 2 + (inode_num as u64 / inodes_per_sector as u64);
    let mut sector = [0u8; SECTOR_SIZE];
    read_sector(file, sector_num, &mut sector)?;
    let off = (inode_num as usize % inodes_per_sector) * inode_size;
    Ok(unsafe { std::ptr::read((sector.as_ptr() as *const u8).add(off) as *const Inode) })
}

fn find_in_dir(file: &mut fs::File, sb: &Superblock, dir_inode: u32, name: &str) -> Result<u32, Box<dyn std::error::Error>> {
    let inode = read_inode(file, sb, dir_inode)?;
    if inode.inode_type != 2 {
        return Err("No es un directorio".into());
    }
    let mut dir_block = [0u8; BLOCK_SIZE];
    for i in 0..12 {
        let block_num = inode.direct_blocks[i];
        if block_num == 0 {
            break;
        }
        read_block(file, block_num, &mut dir_block)?;
        for j in 0..DIR_ENTRIES_PER_BLOCK {
            let entry_ptr = unsafe { (dir_block.as_ptr() as *const u8).add(j * 32) as *const DirEntryDisk };
            let entry = unsafe { &*entry_ptr };
            if entry.inode != 0 {
                let n = entry.name_len as usize;
                if n <= 26 && std::str::from_utf8(&entry.name[..n]).unwrap_or("") == name {
                    return Ok(entry.inode);
                }
            }
        }
    }
    Err(format!("No existe '{}' en el directorio", name).into())
}

/// Crea un directorio en el parent y añade la entrada. Retorna (inodo del nuevo dir, bloques extra usados en parent).
fn create_dir(
    file: &mut fs::File,
    sb: &Superblock,
    bitmap: &mut [u8],
    parent_inode: u32,
    name: &str,
) -> Result<(u32, u32), Box<dyn std::error::Error>> {
    let first_data_block = sb.first_data_block;
    let bitmap_blocks = sb.bitmap_blocks;
    let inode_size = std::mem::size_of::<Inode>();
    let inodes_per_sector = SECTOR_SIZE / inode_size;

    // Allocar inodo para el nuevo directorio
    let mut new_inode_num = 0u32;
    for i in 1..MAX_INODES {
        let sector_num = 1 + (bitmap_blocks as u64) * 2 + (i as u64 / inodes_per_sector as u64);
        let mut sector = [0u8; SECTOR_SIZE];
        read_sector(file, sector_num, &mut sector)?;
        let inode_ptr = unsafe { (sector.as_ptr() as *const u8).add((i as usize % inodes_per_sector) * inode_size) as *const Inode };
        let inode = unsafe { &*inode_ptr };
        if inode.inode_type == 0 {
            new_inode_num = i;
            break;
        }
    }
    if new_inode_num == 0 {
        return Err("No hay inodos libres para crear directorio".into());
    }

    // Allocar un bloque para el directorio (vacío)
    let mut new_block = 0u32;
    for i in first_data_block..sb.total_blocks {
        let byte = (i / 8) as usize;
        let bit = i % 8;
        if byte < bitmap.len() && (bitmap[byte] & (1 << bit)) == 0 {
            new_block = i;
            bitmap[byte] |= 1 << bit;
            break;
        }
    }
    if new_block == 0 {
        return Err("No hay bloques libres para crear directorio".into());
    }

    let now = 0u64;
    let dir_inode = Inode {
        inode_type: 2,
        permissions: 0b0111_0101,
        uid: 0,
        gid: 0,
        size: 0,
        blocks: 1,
        created: now,
        modified: now,
        direct_blocks: {
            let mut a = [0u32; 12];
            a[0] = new_block;
            a
        },
        indirect_block: 0,
        double_indirect: 0,
        _padding: [0; 44],
    };

    let inode_sector = 1 + (bitmap_blocks as u64) * 2 + (new_inode_num as u64 / inodes_per_sector as u64);
    let mut sector = [0u8; SECTOR_SIZE];
    read_sector(file, inode_sector, &mut sector)?;
    let off = (new_inode_num as usize % inodes_per_sector) * inode_size;
    let inode_bytes = unsafe { std::slice::from_raw_parts(&dir_inode as *const _ as *const u8, inode_size) };
    sector[off..off + inode_size].copy_from_slice(inode_bytes);
    write_sector(file, inode_sector, &sector)?;

    let empty_block = [0u8; BLOCK_SIZE];
    write_block(file, new_block, &empty_block)?;

    // Añadir entrada al parent (root)
    let parent_sector = 1 + (bitmap_blocks as u64) * 2 + (parent_inode as u64 / inodes_per_sector as u64);
    let mut sector = [0u8; SECTOR_SIZE];
    read_sector(file, parent_sector, &mut sector)?;
    let parent_off = (parent_inode as usize % inodes_per_sector) * inode_size;
    let parent_ptr = unsafe { (sector.as_ptr() as *const u8).add(parent_off) as *const Inode };
    let parent = unsafe { std::ptr::read_unaligned(parent_ptr) };
    let mut parent_blocks = parent.direct_blocks;
    let mut parent_blocks_count = parent.blocks;
    let mut parent_size = parent.size;
    let mut dir_block_allocated = 0u32;

    let mut dir_block = [0u8; BLOCK_SIZE];
    for i in 0..12 {
        let block_num = parent_blocks[i];
        if block_num == 0 {
            dir_block_allocated = 1;
            let mut new_parent_block = 0u32;
            for j in first_data_block..sb.total_blocks {
                let byte = (j / 8) as usize;
                let bit = j % 8;
                if byte < bitmap.len() && (bitmap[byte] & (1 << bit)) == 0 {
                    new_parent_block = j;
                    bitmap[byte] |= 1 << bit;
                    break;
                }
            }
            if new_parent_block == 0 {
                return Err("No hay bloque libre para entrada en directorio padre".into());
            }
            let entry = DirEntryDisk {
                inode: new_inode_num,
                entry_type: 2,
                name_len: name.len() as u8,
                name: {
                    let mut n = [0u8; 26];
                    n[..name.len()].copy_from_slice(name.as_bytes());
                    n
                },
            };
            let entry_bytes = unsafe { std::slice::from_raw_parts(&entry as *const _ as *const u8, std::mem::size_of::<DirEntryDisk>()) };
            dir_block[..entry_bytes.len()].copy_from_slice(entry_bytes);
            write_block(file, new_parent_block, &dir_block)?;
            parent_blocks[i] = new_parent_block;
            parent_blocks_count += 1;
            parent_size += 32;
            let mut parent_mut = Inode {
                inode_type: 2,
                permissions: parent.permissions,
                uid: parent.uid,
                gid: parent.gid,
                size: parent_size,
                blocks: parent_blocks_count,
                created: parent.created,
                modified: now,
                direct_blocks: parent_blocks,
                indirect_block: parent.indirect_block,
                double_indirect: parent.double_indirect,
                _padding: [0; 44],
            };
            let parent_bytes = unsafe { std::slice::from_raw_parts(&parent_mut as *const _ as *const u8, inode_size) };
            sector[parent_off..parent_off + inode_size].copy_from_slice(parent_bytes);
            write_sector(file, parent_sector, &sector)?;
            return Ok((new_inode_num, 1 + dir_block_allocated));
        }
        read_block(file, block_num, &mut dir_block)?;
        for j in 0..DIR_ENTRIES_PER_BLOCK {
            let entry_ptr = unsafe { (dir_block.as_ptr() as *const u8).add(j * 32) as *const DirEntryDisk };
            let entry = unsafe { &*entry_ptr };
            if entry.inode == 0 {
                let entry_new = DirEntryDisk {
                    inode: new_inode_num,
                    entry_type: 2,
                    name_len: name.len() as u8,
                    name: {
                        let mut n = [0u8; 26];
                        n[..name.len()].copy_from_slice(name.as_bytes());
                        n
                    },
                };
                let entry_bytes = unsafe { std::slice::from_raw_parts(&entry_new as *const _ as *const u8, std::mem::size_of::<DirEntryDisk>()) };
                dir_block[j * 32..j * 32 + entry_bytes.len()].copy_from_slice(entry_bytes);
                write_block(file, block_num, &dir_block)?;
                parent_size += 32;
                let parent_mut = Inode {
                    inode_type: 2,
                    permissions: parent.permissions,
                    uid: parent.uid,
                    gid: parent.gid,
                    size: parent_size,
                    blocks: parent_blocks_count,
                    created: parent.created,
                    modified: now,
                    direct_blocks: parent_blocks,
                    indirect_block: parent.indirect_block,
                    double_indirect: parent.double_indirect,
                    _padding: [0; 44],
                };
                let parent_bytes = unsafe { std::slice::from_raw_parts(&parent_mut as *const _ as *const u8, inode_size) };
                sector[parent_off..parent_off + inode_size].copy_from_slice(parent_bytes);
                write_sector(file, parent_sector, &sector)?;
                return Ok((new_inode_num, 1));
            }
        }
    }

    Ok((new_inode_num, 1))
}

fn format_disk(disk_path: &str, size_mb: u32) -> Result<(), Box<dyn std::error::Error>> {
    let sectors_per_mb = 1024 * 1024 / SECTOR_SIZE;
    let total_sectors = size_mb as usize * sectors_per_mb;
    let total_blocks = (total_sectors / 2) as u32;

    println!("Formateando {} ({} MB, {} bloques)...", disk_path, size_mb, total_blocks);

    let mut file = fs::OpenOptions::new().read(true).write(true).create(true).open(disk_path)?;
    file.set_len((total_sectors * SECTOR_SIZE) as u64)?;

    // Calcular estructura
    let bitmap_blocks = ((total_blocks + 7) / 8 + BLOCK_SIZE as u32 - 1) / BLOCK_SIZE as u32;
    let inode_size = std::mem::size_of::<Inode>() as u32;
    let inodes_per_block = BLOCK_SIZE as u32 / inode_size;
    let inode_blocks = (MAX_INODES + inodes_per_block - 1) / inodes_per_block;
    let first_data_block = 1 + bitmap_blocks + inode_blocks;

    let mut sb = Superblock {
        magic: MESAFS_MAGIC,
        version: 1,
        block_size: BLOCK_SIZE as u16,
        total_blocks,
        total_inodes: MAX_INODES,
        free_blocks: total_blocks - first_data_block,
        free_inodes: MAX_INODES - 1, // Root consume 1
        first_data_block,
        bitmap_blocks,
        inode_blocks,
        root_inode: 0,
        next_free_inode: 1,
        _padding: [0; 468],
    };

    // 1. Escribir Superblock (Sector 0) - Inicial
    let sb_bytes = unsafe { std::slice::from_raw_parts(&sb as *const _ as *const u8, SECTOR_SIZE) };
    write_sector(&mut file, 0, &{
        let mut s = [0u8; SECTOR_SIZE];
        s.copy_from_slice(sb_bytes);
        s
    })?;

    // 2. Inicializar Bitmap (bloques ocupados por metadatos)
    let bitmap_size = ((total_blocks + 7) / 8) as usize;
    let mut bitmap = vec![0u8; bitmap_size];
    for i in 0..first_data_block {
        let byte = (i / 8) as usize;
        let bit = i % 8;
        if byte < bitmap.len() {
            bitmap[byte] |= 1 << bit;
        }
    }

    // 3. Limpiar área de inodos
    let inode_start_sector = 1 + bitmap_blocks as u64;
    let inode_area_sectors = inode_blocks as u64 * 2; // block = 2 sectors
    let zero_sector = [0u8; SECTOR_SIZE];
    for i in 0..inode_area_sectors {
        write_sector(&mut file, inode_start_sector + i, &zero_sector)?;
    }

    // 4. Crear Inodo Root (Inodo 0)
    let root_inode = Inode {
        inode_type: 2, // Directory
        permissions: 0b0111_0101,
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
    };
    
    // El inodo 0 está en el primer sector de inodos
    let inode_bytes = unsafe { std::slice::from_raw_parts(&root_inode as *const _ as *const u8, inode_size as usize) };
    let mut sector = [0u8; SECTOR_SIZE];
    sector[..inode_bytes.len()].copy_from_slice(inode_bytes);
    write_sector(&mut file, inode_start_sector, &sector)?;

    // 5. Crear directorios por defecto
    let dirs = ["bin", "etc", "home", "tmp", "var"];
    for dir in dirs {
        print!("Creando /{}... ", dir);
        let (new_inode, blocks) = create_dir(&mut file, &sb, &mut bitmap, 0, dir)?;
        sb.free_inodes -= 1;
        sb.free_blocks -= blocks;
        println!("Inodo {}", new_inode);
    }
    
    // 6. Escribir Bitmap Final
    let sectors_needed = (bitmap_size + SECTOR_SIZE - 1) / SECTOR_SIZE;
    for i in 0..sectors_needed {
        let start = i * SECTOR_SIZE;
        let end = (start + SECTOR_SIZE).min(bitmap.len());
        let mut sector = [0u8; SECTOR_SIZE];
        if start < bitmap.len() {
            sector[..end - start].copy_from_slice(&bitmap[start..end]);
        }
        write_sector(&mut file, 1 + i as u64, &sector)?;
    }

    // 7. Escribir Superblock Final (con free_blocks actualizados)
    let sb_bytes = unsafe { std::slice::from_raw_parts(&sb as *const _ as *const u8, SECTOR_SIZE) };
    write_sector(&mut file, 0, &{
        let mut s = [0u8; SECTOR_SIZE];
        s.copy_from_slice(sb_bytes);
        s
    })?;

    println!("Formato completado exitosamente.");
    Ok(())
}

fn inject_file(disk_path: &str, local_path: &str, dest_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dest_path = dest_path.trim_matches('/');
    let data = fs::read(local_path)?;
    let num_blocks_needed = (data.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;
    if num_blocks_needed > 12 {
        eprintln!("Archivo demasiado grande (máx {} bytes = 12 bloques)", 12 * BLOCK_SIZE);
        std::process::exit(1);
    }

    let mut file = fs::OpenOptions::new().read(true).write(true).open(disk_path)?;
    let sb = read_superblock(&mut file)?;

    if sb.magic != MESAFS_MAGIC {
        let magic = sb.magic;
        eprintln!("No es un disco MesaFS válido (Magic: {:x}). Usa 'format' primero.", magic);
        std::process::exit(1);
    }
    
    if sb.free_inodes == 0 || sb.free_blocks == 0 {
        eprintln!("Disco lleno (sin inodos o bloques libres)");
        std::process::exit(1);
    }

    let components: Vec<&str> = dest_path.split('/').filter(|s: &&str| !s.is_empty()).collect();
    let first_data_block = sb.first_data_block;
    let bitmap_blocks = sb.bitmap_blocks;
    let inode_size = std::mem::size_of::<Inode>();
    let inodes_per_sector = SECTOR_SIZE / inode_size;
    let bitmap_byte_count = ((sb.total_blocks + 7) / 8) as usize;

    // Leer bitmap antes de cualquier asignación
    let mut bitmap = vec![0u8; bitmap_byte_count];
    for b in 0..(bitmap_byte_count + SECTOR_SIZE - 1) / SECTOR_SIZE {
        let mut sector = [0u8; SECTOR_SIZE];
        read_sector(&mut file, 1 + b as u64, &mut sector)?;
        let start = b * SECTOR_SIZE;
        let end = (start + SECTOR_SIZE).min(bitmap_byte_count);
        bitmap[start..end].copy_from_slice(&sector[..end - start]);
    }

    let mut extra_inodes = 0u32;
    let mut extra_blocks = 0u32;
    let (dir_inode, file_name): (u32, &str) = if components.len() == 1 {
        (0u32, components[0])
    } else if components.len() == 2 {
        match find_in_dir(&mut file, &sb, 0, components[0]) {
            Ok(di) => (di, components[1]),
            Err(_) => {
                // Crear el directorio (ej. /bin) si no existe
                let dir_name = components[0];
                if dir_name.len() > 26 {
                    eprintln!("Nombre de directorio '{}' demasiado largo", dir_name);
                    std::process::exit(1);
                }
                let (new_dir_inode, blocks_used) = create_dir(&mut file, &sb, &mut bitmap, 0, dir_name)?;
                extra_inodes = 1;
                extra_blocks = blocks_used;
                (new_dir_inode, components[1])
            }
        }
    } else {
        eprintln!("Solo se soporta ruta en raíz o un subdirectorio (ej: /hello.elf o /bin/hello.elf)");
        std::process::exit(1);
    };
    
    if file_name.len() > 26 {
        eprintln!("Nombre de archivo demasiado largo (máx 26 caracteres)");
        std::process::exit(1);
    }

    // Buscar inodo libre (desde 1, 0 es root)
    let mut free_inode = 0u32;
    for i in 1..MAX_INODES {
        let sector_num = 1 + (bitmap_blocks as u64) * 2 + (i as u64 / inodes_per_sector as u64);
        let mut sector = [0u8; SECTOR_SIZE];
        read_sector(&mut file, sector_num, &mut sector)?;
        let inode_ptr = unsafe { (sector.as_ptr() as *const u8).add((i as usize % inodes_per_sector) * inode_size) as *const Inode };
        let inode = unsafe { &*inode_ptr };
        if inode.inode_type == 0 {
            free_inode = i;
            break;
        }
    }
    if free_inode == 0 {
        eprintln!("No hay inodos libres");
        std::process::exit(1);
    }

    // Buscar bloques libres en el bitmap (uno por cada bloque del archivo)
    let mut file_blocks = Vec::with_capacity(num_blocks_needed);
    for _ in 0..num_blocks_needed {
        let mut found = 0u32;
        for i in first_data_block..sb.total_blocks {
            let byte = (i / 8) as usize;
            let bit = i % 8;
            if byte < bitmap.len() && (bitmap[byte] & (1 << bit)) == 0 {
                found = i;
                bitmap[byte] |= 1 << bit;
                break;
            }
        }
        if found == 0 {
            eprintln!("No hay suficientes bloques libres (se necesitan {})", num_blocks_needed);
            std::process::exit(1);
        }
        file_blocks.push(found);
    }

    // Crear inodo para el archivo
    let now = 0u64;
    let mut direct_blocks = [0u32; 12];
    for (i, &bn) in file_blocks.iter().enumerate() {
        direct_blocks[i] = bn;
    }
    let file_inode = Inode {
        inode_type: 1,
        permissions: 0b0110_0100,
        uid: 0,
        gid: 0,
        size: data.len() as u32,
        blocks: num_blocks_needed as u32,
        created: now,
        modified: now,
        direct_blocks,
        indirect_block: 0,
        double_indirect: 0,
        _padding: [0; 44],
    };

    // Escribir inodo
    let inode_sector = 1 + (bitmap_blocks as u64) * 2 + (free_inode as u64 / inodes_per_sector as u64);
    let mut sector = [0u8; SECTOR_SIZE];
    read_sector(&mut file, inode_sector, &mut sector)?;
    let inode_bytes = unsafe {
        std::slice::from_raw_parts(&file_inode as *const _ as *const u8, inode_size)
    };
    let off = (free_inode as usize % inodes_per_sector) * inode_size;
    sector[off..off + inode_size].copy_from_slice(inode_bytes);
    write_sector(&mut file, inode_sector, &sector)?;

    // Escribir datos del archivo
    for (idx, &block_num) in file_blocks.iter().enumerate() {
        let start = idx * BLOCK_SIZE;
        let end = (start + BLOCK_SIZE).min(data.len());
        let chunk = &data[start..end];
        let mut block_data = [0u8; BLOCK_SIZE];
        block_data[..chunk.len()].copy_from_slice(chunk);
        write_block(&mut file, block_num, &block_data)?;
    }

    // Añadir entrada al directorio
    let dir_inode_sector = 1 + (bitmap_blocks as u64) * 2 + (dir_inode as u64 / (SECTOR_SIZE / inode_size) as u64);
    let mut sector = [0u8; SECTOR_SIZE];
    read_sector(&mut file, dir_inode_sector, &mut sector)?;
    let dir_inode_off = (dir_inode as usize % (SECTOR_SIZE / inode_size)) * inode_size;
    let current_dir_ptr = unsafe { (sector.as_ptr() as *const u8).add(dir_inode_off) as *const Inode };
    let current_dir = unsafe { std::ptr::read_unaligned(current_dir_ptr) };
    let dir_blocks = current_dir.direct_blocks;
    let dir_blocks_count = current_dir.blocks;
    let dir_size = current_dir.size;

    let mut dir_block = [0u8; BLOCK_SIZE];
    let mut added = false;
    let mut dir_block_allocated = 0u32;
    for i in 0..12 {
        let block_num = dir_blocks[i];
        if block_num == 0 {
            // Asignar nuevo bloque
            dir_block_allocated = 1;
            let mut new_block = 0u32;
            for j in first_data_block..sb.total_blocks {
                let byte = (j / 8) as usize;
                let bit = j % 8;
                if byte < bitmap.len() && (bitmap[byte] & (1 << bit)) == 0 {
                    new_block = j;
                    bitmap[byte] |= 1 << bit;
                    break;
                }
            }
            if new_block == 0 {
                eprintln!("No hay bloque libre para entrada de directorio");
                std::process::exit(1);
            }
            let entry = DirEntryDisk {
                inode: free_inode,
                entry_type: 1,
                name_len: file_name.len() as u8,
                name: {
                    let mut n = [0u8; 26];
                    n[..file_name.len()].copy_from_slice(file_name.as_bytes());
                    n
                },
            };
            let entry_bytes = unsafe {
                std::slice::from_raw_parts(&entry as *const _ as *const u8, std::mem::size_of::<DirEntryDisk>())
            };
            dir_block[..entry_bytes.len()].copy_from_slice(entry_bytes);
            write_block(&mut file, new_block, &dir_block)?;
             let mut dir_inode_mut = Inode {
                 inode_type: 2,
                 permissions: current_dir.permissions,
                 uid: current_dir.uid,
                 gid: current_dir.gid,
                 size: dir_size + 32,
                 blocks: dir_blocks_count + 1,
                 created: current_dir.created,
                 modified: now,
                 direct_blocks: dir_blocks,
                 indirect_block: current_dir.indirect_block,
                 double_indirect: current_dir.double_indirect,
                 _padding: [0; 44],
             };
            dir_inode_mut.direct_blocks[i] = new_block;
            let dir_bytes = unsafe {
                std::slice::from_raw_parts(&dir_inode_mut as *const _ as *const u8, inode_size)
            };
            sector[dir_inode_off..dir_inode_off + inode_size].copy_from_slice(dir_bytes);
            write_sector(&mut file, dir_inode_sector, &sector)?;
            added = true;
            break;
        }
        
        read_block(&mut file, block_num, &mut dir_block)?;
        for j in 0..DIR_ENTRIES_PER_BLOCK {
            let entry_ptr = unsafe { (dir_block.as_ptr() as *const u8).add(j * 32) as *const DirEntryDisk };
            let entry = unsafe { &*entry_ptr };
            if entry.inode == 0 {
                let entry_new = DirEntryDisk {
                    inode: free_inode,
                    entry_type: 1,
                    name_len: file_name.len() as u8,
                    name: {
                        let mut n = [0u8; 26];
                        n[..file_name.len()].copy_from_slice(file_name.as_bytes());
                        n
                    },
                };
                let entry_bytes = unsafe {
                    std::slice::from_raw_parts(&entry_new as *const _ as *const u8, std::mem::size_of::<DirEntryDisk>())
                };
                dir_block[j * 32..j * 32 + entry_bytes.len()].copy_from_slice(entry_bytes);
                write_block(&mut file, block_num, &dir_block)?;
                let dir_inode_mut = Inode {
                    inode_type: 2,
                     permissions: current_dir.permissions,
                     uid: current_dir.uid,
                     gid: current_dir.gid,
                     size: dir_size + 32,
                     blocks: dir_blocks_count,
                     created: current_dir.created,
                     modified: now,
                     direct_blocks: dir_blocks,
                     indirect_block: current_dir.indirect_block,
                     double_indirect: current_dir.double_indirect,
                     _padding: [0; 44],
                 };
                let dir_bytes = unsafe {
                    std::slice::from_raw_parts(&dir_inode_mut as *const _ as *const u8, inode_size)
                };
                sector[dir_inode_off..dir_inode_off + inode_size].copy_from_slice(dir_bytes);
                write_sector(&mut file, dir_inode_sector, &sector)?;
                added = true;
                break;
            }
        }
        if added { break; }
    }
    if !added {
        eprintln!("No hay espacio en el directorio");
        std::process::exit(1);
    }

    // Escribir bitmap actualizado
    for b in 0..(bitmap_byte_count + SECTOR_SIZE - 1) / SECTOR_SIZE {
        let mut sector = [0u8; SECTOR_SIZE];
        let start = b * SECTOR_SIZE;
        let end = (start + SECTOR_SIZE).min(bitmap_byte_count);
        sector[..end - start].copy_from_slice(&bitmap[start..end]);
        write_sector(&mut file, 1 + b as u64, &sector)?;
    }

    // Actualizar superblock
    let mut sb_new = sb;
    sb_new.free_blocks -= num_blocks_needed as u32 + dir_block_allocated + extra_blocks;
    sb_new.free_inodes -= 1 + extra_inodes;
    let sb_bytes = unsafe {
        std::slice::from_raw_parts(&sb_new as *const _ as *const u8, SECTOR_SIZE)
    };
    let mut sector = [0u8; SECTOR_SIZE];
    sector[..sb_bytes.len()].copy_from_slice(sb_bytes);
    file.rewind()?;
    file.write_all(&sector)?;

    println!("OK: {} -> {} (inodo {}, {} bloques)", local_path, dest_path, free_inode, num_blocks_needed);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    if args[1] == "format" {
        if args.len() != 4 {
            eprintln!("Uso: {} format <disk.img> <size_mb>", args[0]);
            std::process::exit(1);
        }
        let disk_path = &args[2];
        let size_mb: u32 = args[3].parse().expect("size_mb debe ser un número");
        format_disk(disk_path, size_mb)?;
    } else if args.len() == 4 {
        // Modo legacy: mesafs_inject <disk> <file> <dest>
        inject_file(&args[1], &args[2], &args[3])?;
    } else if args[1] == "inject" {
         if args.len() != 5 {
            eprintln!("Uso: {} inject <disk.img> <local_file> <dest_root>", args[0]);
            std::process::exit(1);
        }
        inject_file(&args[2], &args[3], &args[4])?;
    } else {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    Ok(())
}

fn print_usage(prog: &str) {
    eprintln!("Uso:");
    eprintln!("  Format: {} format <disk.img> <size_mb>", prog);
    eprintln!("  Inject: {} inject <disk.img> <local_file> <dest_path>", prog);
    eprintln!("  Legacy: {} <disk.img> <local_file> <dest_path>", prog);
}
