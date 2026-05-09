//! Sincronización entre filesystems (RAM <-> Disco)

use alloc::vec::Vec;
use alloc::string::String;
use super::{FileSystem, FsError, FsResult, NodeType};

/// Sincroniza recursivamente un origen (RAM) a un destino (Disco)
pub fn sync_recursive(src: &dyn FileSystem, dst: &dyn FileSystem, path: &str) -> FsResult<()> {
    // 1. Obtener entradas del directorio actual
    let entries = src.readdir(path)?;
    
    for entry in entries {
        let mut full_path = String::from(path);
        if !full_path.ends_with('/') {
            full_path.push('/');
        }
        full_path.push_str(&entry.name);
        
        match entry.node_type {
            NodeType::File => {
                // Leer del origen
                let data = src.read(&full_path)?;
                
                // Verificar si existe en destino y si ha cambiado
                let src_meta = src.stat(&full_path)?;
                let should_write = match dst.stat(&full_path) {
                    Ok(meta) => {
                        // Escribir si el tamaño es diferente o el origen es más reciente
                        meta.size != src_meta.size || src_meta.modified > meta.modified
                    }
                    Err(FsError::NotFound) => true,
                    Err(e) => return Err(e),
                };
                
                if should_write {
                    crate::serial_println!("[SYNC] Guardando: {}", full_path);
                    dst.write(&full_path, &data)?;
                    // Preservar timestamps
                    dst.utime(&full_path, src_meta.created, src_meta.modified)?;
                }
            }
            NodeType::Directory => {
                // Asegurar que el directorio existe en el destino
                let src_meta = src.stat(&full_path)?;
                match dst.stat(&full_path) {
                    Ok(meta) => {
                        if meta.node_type != NodeType::Directory {
                            return Err(FsError::IoError);
                        }
                    }
                    Err(FsError::NotFound) => {
                        crate::serial_println!("[SYNC] Creando directorio: {}", full_path);
                        dst.mkdir(&full_path)?;
                        dst.utime(&full_path, src_meta.created, src_meta.modified)?;
                    }
                    Err(e) => return Err(e),
                }
                
                // Recursión
                sync_recursive(src, dst, &full_path)?;
            }
            _ => {} // Otros tipos no soportados para sync simple
        }
    }
    
    // 2. Eliminar archivos/directorios en el destino que ya no existen en el origen
    if let Ok(dst_entries) = dst.readdir(path) {
        for entry in dst_entries {
            let mut full_path = String::from(path);
            if !full_path.ends_with('/') {
                full_path.push('/');
            }
            full_path.push_str(&entry.name);
            
            // Si no existe en el origen, eliminarlo del destino
            if src.stat(&full_path).is_err() {
                crate::serial_println!("[SYNC] Eliminando: {}", full_path);
                if entry.node_type == NodeType::Directory {
                    // Borrado recursivo para asegurar que el directorio no esté lleno
                    let _ = delete_recursive(dst, &full_path);
                } else {
                    let _ = dst.remove(&full_path);
                }
            }
        }
    }
    
    Ok(())
}

/// Borra un directorio y todo su contenido recursivamente en el sistema de archivos dado
fn delete_recursive(fs: &dyn FileSystem, path: &str) -> FsResult<()> {
    if let Ok(entries) = fs.readdir(path) {
        for entry in entries {
            let mut full_path = String::from(path);
            if !full_path.ends_with('/') {
                full_path.push('/');
            }
            full_path.push_str(&entry.name);
            
            if entry.node_type == NodeType::Directory {
                let _ = delete_recursive(fs, &full_path);
            } else {
                let _ = fs.remove(&full_path);
            }
        }
    }
    fs.rmdir(path)
}

/// Carga recursivamente de un origen (Disco) a un destino (RAM)
pub fn load_recursive(src: &dyn FileSystem, dst: &dyn FileSystem, path: &str) -> FsResult<()> {
    let entries = src.readdir(path)?;
    
    for entry in entries {
        let mut full_path = String::from(path);
        if !full_path.ends_with('/') {
            full_path.push('/');
        }
        full_path.push_str(&entry.name);
        
        match entry.node_type {
            NodeType::File => {
                let src_meta = src.stat(&full_path)?;
                let data = src.read(&full_path)?;
                dst.write(&full_path, &data)?;
                dst.utime(&full_path, src_meta.created, src_meta.modified)?;
            }
            NodeType::Directory => {
                let src_meta = src.stat(&full_path)?;
                if let Err(FsError::AlreadyExists) = dst.mkdir(&full_path) {
                    // Ignorar si ya existe
                }
                dst.utime(&full_path, src_meta.created, src_meta.modified)?;
                load_recursive(src, dst, &full_path)?;
            }
            _ => {}
        }
    }
    
    Ok(())
}
