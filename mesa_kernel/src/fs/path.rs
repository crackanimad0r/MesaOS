//! Utilidades para manejo de paths

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;  // AGREGAR ESTA LÍNEA
use super::{FsError, FsResult};

/// Resuelve un path relativo a absoluto
pub fn resolve(path: &str) -> FsResult<String> {
    let path = path.trim();
    
    if path.is_empty() {
        return Ok(super::cwd());
    }
    
    // Si es absoluto, normalizar
    if path.starts_with('/') {
        return normalize(path);
    }
    
    // Relativo: combinar con CWD
    let cwd = super::cwd();
    let full = if cwd == "/" {
        format!("/{}", path)
    } else {
        format!("{}/{}", cwd, path)
    };
    
    normalize(&full)
}

/// Normaliza un path (resuelve . y ..)
pub fn normalize(path: &str) -> FsResult<String> {
    let mut components: Vec<&str> = Vec::new();
    
    for part in path.split('/') {
        match part {
            "" | "." => continue,
            ".." => {
                if components.pop().is_none() {
                    // Intentar ir más arriba de root
                    return Err(FsError::InvalidPath);
                }
            }
            name => components.push(name),
        }
    }
    
    if components.is_empty() {
        Ok(String::from("/"))
    } else {
        let mut result = String::new();
        for comp in components {
            result.push('/');
            result.push_str(comp);
        }
        Ok(result)
    }
}

/// Obtiene el nombre del archivo/directorio (último componente)
pub fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Obtiene el directorio padre
pub fn dirname(path: &str) -> String {
    if let Some(pos) = path.rfind('/') {
        if pos == 0 {
            String::from("/")
        } else {
            String::from(&path[..pos])
        }
    } else {
        String::from(".")
    }
}

/// Une dos paths
pub fn join(base: &str, name: &str) -> String {
    if name.starts_with('/') {
        String::from(name)
    } else if base == "/" {
        format!("/{}", name)
    } else {
        format!("{}/{}", base, name)
    }
}