//! RAM Filesystem - Filesystem en memoria

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use spin::RwLock;

use super::{DirEntry, FileSystem, FsError, FsResult, Metadata, NodeType};

/// Nodo del filesystem
#[derive(Debug)]
enum FsNode {
    File {
        data: Vec<u8>,
        meta: Metadata,
    },
    Directory {
        children: BTreeMap<String, FsNode>,
        meta: Metadata,
    },
}

impl FsNode {
    fn new_file(data: Vec<u8>, uid: u32, gid: u32) -> Self {
        FsNode::File {
            meta: Metadata::new_file(data.len() as u64, uid, gid),
            data,
        }
    }
    
    fn new_dir(uid: u32, gid: u32) -> Self {
        FsNode::Directory {
            children: BTreeMap::new(),
            meta: Metadata::new_dir(uid, gid),
        }
    }
    
    fn metadata(&self) -> &Metadata {
        match self {
            FsNode::File { meta, .. } => meta,
            FsNode::Directory { meta, .. } => meta,
        }
    }
    
    fn is_dir(&self) -> bool {
        matches!(self, FsNode::Directory { .. })
    }
    
    fn is_file(&self) -> bool {
        matches!(self, FsNode::File { .. })
    }
}

/// RamFS - Filesystem en RAM
pub struct RamFs {
    root: RwLock<FsNode>,
}

impl RamFs {
    pub fn new() -> Self {
        Self {
            root: RwLock::new(FsNode::new_dir(0, 0)),
        }
    }
    
    /// Parsea un path en componentes
    fn parse_path(path: &str) -> Vec<&str> {
        path.split('/')
            .filter(|s| !s.is_empty() && *s != ".")
            .collect()
    }
    
    /// Navega hasta el nodo padre y retorna el nombre del último componente
    fn navigate_to_parent<'a>(
        node: &'a mut FsNode,
        components: &'a [&'a str],
    ) -> FsResult<(&'a mut FsNode, &'a str)> {
        if components.is_empty() {
            return Err(FsError::InvalidPath);
        }
        
        let (parent_path, name) = components.split_at(components.len() - 1);
        let name = name[0];
        
        let mut current = node;
        for comp in parent_path {
            match current {
                FsNode::Directory { children, .. } => {
                    current = children.get_mut(*comp).ok_or(FsError::NotFound)?;
                }
                _ => return Err(FsError::NotADirectory),
            }
        }
        
        Ok((current, name))
    }
    
    /// Navega hasta un nodo específico
    fn navigate<'a>(node: &'a FsNode, components: &[&str]) -> FsResult<&'a FsNode> {
        let mut current = node;
        for comp in components {
            match current {
                FsNode::Directory { children, .. } => {
                    current = children.get(*comp).ok_or(FsError::NotFound)?;
                }
                _ => return Err(FsError::NotADirectory),
            }
        }
        Ok(current)
    }
    
    /// Navega hasta un nodo específico (mutable)
    fn navigate_mut<'a>(node: &'a mut FsNode, components: &[&str]) -> FsResult<&'a mut FsNode> {
        let mut current = node;
        for comp in components {
            match current {
                FsNode::Directory { children, .. } => {
                    current = children.get_mut(*comp).ok_or(FsError::NotFound)?;
                }
                _ => return Err(FsError::NotADirectory),
            }
        }
        Ok(current)
    }
}

impl FileSystem for RamFs {
    fn name(&self) -> &str {
        "ramfs"
    }
    
    fn stat(&self, path: &str) -> FsResult<Metadata> {
        let root = self.root.read();
        let components = Self::parse_path(path);
        
        if components.is_empty() {
            return Ok(root.metadata().clone());
        }
        
        let node = Self::navigate(&root, &components)?;
        Ok(node.metadata().clone())
    }
    
    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        let root = self.root.read();
        let components = Self::parse_path(path);
        
        let node = if components.is_empty() {
            &*root
        } else {
            Self::navigate(&root, &components)?
        };
        
        match node {
            FsNode::Directory { children, .. } => {
                let mut entries = Vec::new();
                for (name, child) in children.iter() {
                    entries.push(DirEntry {
                        name: name.clone(),
                        node_type: child.metadata().node_type,
                        size: child.metadata().size,
                    });
                }
                Ok(entries)
            }
            _ => Err(FsError::NotADirectory),
        }
    }
    
    fn read(&self, path: &str) -> FsResult<Vec<u8>> {
        let root = self.root.read();
        let components = Self::parse_path(path);
        
        if components.is_empty() {
            return Err(FsError::IsADirectory);
        }
        
        let node = Self::navigate(&root, &components)?;
        
        match node {
            FsNode::File { data, .. } => Ok(data.clone()),
            FsNode::Directory { .. } => Err(FsError::IsADirectory),
        }
    }
    
    fn write(&self, path: &str, data: &[u8]) -> FsResult<()> {
        let mut root = self.root.write();
        let components = Self::parse_path(path);
        
        if components.is_empty() {
            return Err(FsError::IsADirectory);
        }
        
        let uid = crate::users::current_uid();
        let gid = crate::users::current_gid();
        
        let (parent, name) = Self::navigate_to_parent(&mut root, &components)?;
        
        match parent {
            FsNode::Directory { children, meta } => {
                meta.modified = crate::curr_arch::get_ticks();
                children.insert(
                    String::from(name),
                    FsNode::new_file(data.to_vec(), uid, gid),
                );
                Ok(())
            }
            _ => Err(FsError::NotADirectory),
        }
    }
    
    fn mkdir(&self, path: &str) -> FsResult<()> {
        let mut root = self.root.write();
        let components = Self::parse_path(path);
        
        if components.is_empty() {
            return Err(FsError::AlreadyExists);
        }
        
        let uid = crate::users::current_uid();
        let gid = crate::users::current_gid();
        
        let (parent, name) = Self::navigate_to_parent(&mut root, &components)?;
        
        match parent {
            FsNode::Directory { children, meta } => {
                if children.contains_key(name) {
                    return Err(FsError::AlreadyExists);
                }
                
                meta.modified = crate::curr_arch::get_ticks();
                children.insert(String::from(name), FsNode::new_dir(uid, gid));
                Ok(())
            }
            _ => Err(FsError::NotADirectory),
        }
    }
    
    fn create(&self, path: &str) -> FsResult<()> {
        self.write(path, &[])
    }
    
    fn remove(&self, path: &str) -> FsResult<()> {
        let mut root = self.root.write();
        let components = Self::parse_path(path);
        
        if components.is_empty() {
            return Err(FsError::PermissionDenied);
        }
        
        let (parent, name) = Self::navigate_to_parent(&mut root, &components)?;
        
        match parent {
            FsNode::Directory { children, meta } => {
                let node = children.get(name).ok_or(FsError::NotFound)?;
                
                if node.is_dir() {
                    return Err(FsError::IsADirectory);
                }
                
                meta.modified = crate::curr_arch::get_ticks();
                children.remove(name);
                Ok(())
            }
            _ => Err(FsError::NotADirectory),
        }
    }
    
    fn rmdir(&self, path: &str) -> FsResult<()> {
        let mut root = self.root.write();
        let components = Self::parse_path(path);
        
        if components.is_empty() {
            return Err(FsError::PermissionDenied);
        }
        
        let (parent, name) = Self::navigate_to_parent(&mut root, &components)?;
        
        match parent {
            FsNode::Directory { children, meta } => {
                let node = children.get(name).ok_or(FsError::NotFound)?;
                
                match node {
                    FsNode::File { .. } => Err(FsError::NotADirectory),
                    FsNode::Directory { children: dir_children, .. } => {
                        if !dir_children.is_empty() {
                            return Err(FsError::NotEmpty);
                        }
                        meta.modified = crate::curr_arch::get_ticks();
                        children.remove(name);
                        Ok(())
                    }
                }
            }
            _ => Err(FsError::NotADirectory),
        }
    }
    
    fn rename(&self, from: &str, to: &str) -> FsResult<()> {
        let data = self.read(from);
        let meta = self.stat(from)?;
        
        match meta.node_type {
            NodeType::File => {
                let data = data?;
                self.write(to, &data)?;
                self.remove(from)?;
            }
            NodeType::Directory => {
                return Err(FsError::IsADirectory);
            }
            _ => return Err(FsError::IoError),
        }
        
        Ok(())
    }
    fn utime(&self, path: &str, created: u64, modified: u64) -> FsResult<()> {
        let mut root = self.root.write();
        let components = Self::parse_path(path);
        let node = Self::navigate_mut(&mut root, &components)?;
        
        match node {
            FsNode::File { meta, .. } | FsNode::Directory { meta, .. } => {
                meta.created = created;
                meta.modified = modified;
                Ok(())
            }
        }
    }
}

unsafe impl Send for RamFs {}
unsafe impl Sync for RamFs {}