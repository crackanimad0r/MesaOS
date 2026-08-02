//! initrd - Init Ramdisk: Extrae archivos embebidos al RamFS durante el boot.
//!
//! Los archivos se empaquetan con tools/inject_to_iso.sh y se compilan
//! dentro del kernel via include_bytes! desde output/initrd.bin
//!
//! Formato binario del initrd:
//!   [u32: count] - número de entradas (little-endian)
//!   Por cada entrada:
//!     [u32: name_len] - longitud del nombre base
//!     [name bytes]     - nombre del archivo (sin ruta, UTF-8)
//!     [u32: path_len]  - longitud del path completo
//!     [path bytes]     - path completo (relativo a /, UTF-8)
//!     [u32: data_len]  - longitud del contenido
//!     [data bytes]     - contenido del archivo

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{mkdir, write, FsError};

/// Módulo generado automáticamente con los datos del initrd
use super::initrd_data;

/// Datos de una entrada del initrd
#[derive(Debug)]
pub struct InitrdEntry {
    pub name: String,
    pub path: String,
    pub data: Vec<u8>,
}

/// Parsea el binario del initrd y retorna las entradas
pub fn parse_initrd(data: &[u8]) -> Result<Vec<InitrdEntry>, &'static str> {
    if data.len() < 4 {
        return Err("Initrd demasiado pequeño");
    }

    let mut offset = 0;

    // Leer count (u32 little-endian)
    let count = u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize;
    offset += 4;

    let mut entries = Vec::with_capacity(count);

    for _ in 0..count {
        // Leer name_len
        if offset + 4 > data.len() {
            return Err("Initrd: EOF prematuro en name_len");
        }
        let name_len = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        // Leer name
        if offset + name_len > data.len() {
            return Err("Initrd: EOF prematuro en name");
        }
        let name_bytes = &data[offset..offset + name_len];
        let name = core::str::from_utf8(name_bytes)
            .map_err(|_| "Initrd: name no es UTF-8 válido")?
            .to_string();
        offset += name_len;

        // Leer path_len
        if offset + 4 > data.len() {
            return Err("Initrd: EOF prematuro en path_len");
        }
        let path_len = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        // Leer path
        if offset + path_len > data.len() {
            return Err("Initrd: EOF prematuro en path");
        }
        let path_bytes = &data[offset..offset + path_len];
        let path = core::str::from_utf8(path_bytes)
            .map_err(|_| "Initrd: path no es UTF-8 válido")?
            .to_string();
        offset += path_len;

        // Leer data_len
        if offset + 4 > data.len() {
            return Err("Initrd: EOF prematuro en data_len");
        }
        let data_len = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        // Leer data
        if offset + data_len > data.len() {
            return Err("Initrd: EOF prematuro en data");
        }
        let file_data = data[offset..offset + data_len].to_vec();
        offset += data_len;

        entries.push(InitrdEntry {
            name,
            path,
            data: file_data,
        });
    }

    Ok(entries)
}

/// Extrae todos los archivos del initrd al RamFS dentro de /inyect/
/// preservando la estructura de directorios.
/// Los archivos NO se extraen a la raíz del sistema, SOLO a /inyect/.
pub fn extract_initrd() -> Result<usize, &'static str> {
    let data = initrd_data::INITRD_DATA;

    if data.len() <= 4 {
        crate::serial_println!("[INITRD] Initrd vacío (sin archivos embebidos)");
        return Ok(0);
    }

    crate::serial_println!("[INITRD] Extrayendo {} bytes de initrd...", data.len());

    let entries = parse_initrd(data)?;
    let mut count = 0;

    // Crear directorio /inyect/ raíz de los archivos inyectados
    let _ = mkdir_recursive("/inyect");

    for entry in &entries {
        let inyect_path = format!("/inyect/{}", entry.path);
        crate::serial_println!(
            "[INITRD] Extrayendo: {} ({} bytes)",
            inyect_path,
            entry.data.len()
        );

        // Crear directorios padre dentro de /inyect/
        let inyect_parent = get_parent_dir(&inyect_path);
        if let Some(parent) = inyect_parent {
            let _ = mkdir_recursive(parent);
        }

        match write(&inyect_path, &entry.data) {
            Ok(_) => {
                count += 1;
                crate::serial_println!("[INITRD] OK: {}", inyect_path);
            }
            Err(e) => {
                crate::serial_println!(
                    "[INITRD] ERROR escribiendo {}: {}",
                    inyect_path,
                    e.as_str()
                );
            }
        }
    }

    crate::serial_println!("[INITRD] Extraídos {} archivos en /inyect/.", count);
    crate::klog_info!("Initrd extracted: {} files to /inyect/", count);

    Ok(count)
}

/// Obtiene el directorio padre de un path
fn get_parent_dir(path: &str) -> Option<&str> {
    let trimmed = path.trim_end_matches('/');
    let idx = trimmed.rfind('/')?;
    if idx == 0 {
        None // No hay directorio padre (archivo en raíz)
    } else {
        Some(&trimmed[..idx])
    }
}

/// Crea directorios recursivamente
fn mkdir_recursive(path: &str) -> Result<(), FsError> {
    // Normalizar: eliminar trailing slash
    let path = path.trim_end_matches('/');

    if path.is_empty() || path == "/" {
        return Ok(());
    }

    // Intentar crear el directorio padre primero
    if let Some(parent) = get_parent_dir(path) {
        let abs_parent = if parent.starts_with('/') {
            String::from(parent)
        } else {
            format!("/{}", parent)
        };
        let _ = mkdir_recursive(&abs_parent);
    }

    // Crear este directorio
    match mkdir(path) {
        Ok(_) => Ok(()),
        Err(FsError::AlreadyExists) => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_initrd() {
        // 0 entries
        let data = [0x00, 0x00, 0x00, 0x00];
        let entries = parse_initrd(&data).unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_parse_single_entry() {
        // 1 entry: name="test.txt", path="mydir/test.txt", data="Hello"
        let mut data = Vec::new();
        // count = 1
        data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        // name_len = 8
        data.extend_from_slice(&[0x08, 0x00, 0x00, 0x00]);
        // name = "test.txt"
        data.extend_from_slice(b"test.txt");
        // path_len = 14
        data.extend_from_slice(&[0x0E, 0x00, 0x00, 0x00]);
        // path = "mydir/test.txt"
        data.extend_from_slice(b"mydir/test.txt");
        // data_len = 5
        data.extend_from_slice(&[0x05, 0x00, 0x00, 0x00]);
        // data = "Hello"
        data.extend_from_slice(b"Hello");

        let entries = parse_initrd(&data).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "test.txt");
        assert_eq!(entries[0].path, "mydir/test.txt");
        assert_eq!(entries[0].data, b"Hello");
    }

    #[test]
    fn test_parse_multiple_entries() {
        let mut data = Vec::new();
        // count = 2
        data.extend_from_slice(&[0x02, 0x00, 0x00, 0x00]);

        // Entry 1: "file1.txt" en "/" con "data1"
        data.extend_from_slice(&[0x09, 0x00, 0x00, 0x00]); // name_len
        data.extend_from_slice(b"file1.txt"); // name
        data.extend_from_slice(&[0x09, 0x00, 0x00, 0x00]); // path_len
        data.extend_from_slice(b"file1.txt"); // path
        data.extend_from_slice(&[0x05, 0x00, 0x00, 0x00]); // data_len
        data.extend_from_slice(b"data1"); // data

        // Entry 2: "script.sh" en "/bin/" con "#!/bin/sh"
        data.extend_from_slice(&[0x09, 0x00, 0x00, 0x00]); // name_len
        data.extend_from_slice(b"script.sh"); // name
        data.extend_from_slice(&[0x0D, 0x00, 0x00, 0x00]); // path_len
        data.extend_from_slice(b"bin/script.sh"); // path
        data.extend_from_slice(&[0x09, 0x00, 0x00, 0x00]); // data_len
        data.extend_from_slice(b"#!/bin/sh\n"); // data

        let entries = parse_initrd(&data).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "file1.txt");
        assert_eq!(entries[0].path, "file1.txt");
        assert_eq!(entries[1].name, "script.sh");
        assert_eq!(entries[1].path, "bin/script.sh");
    }
}
