//! Virtual File System (VFS) para Mesa OS

pub mod ramfs;
pub mod path;
pub mod partition;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

/// Tipos de nodo en el filesystem
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    File,
    Directory,
    Symlink,
    Device,
}

/// Representa un archivo abierto con su posición actual
#[derive(Debug, Clone)]
pub struct FileHandle {
    pub path: String,
    pub pos: usize,
    pub node_type: NodeType,
}

/// Permisos de archivo (estilo Unix)
#[derive(Debug, Clone, Copy)]
pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Permissions {
    pub const fn all() -> Self {
        Self { read: true, write: true, execute: true }
    }
    
    pub const fn read_only() -> Self {
        Self { read: true, write: false, execute: false }
    }
    
    pub const fn read_write() -> Self {
        Self { read: true, write: true, execute: false }
    }
    
    pub const fn read_exec() -> Self {
        Self { read: true, write: false, execute: true }
    }
    
    pub fn to_string(&self) -> String {
        let mut s = String::with_capacity(3);
        s.push(if self.read { 'r' } else { '-' });
        s.push(if self.write { 'w' } else { '-' });
        s.push(if self.execute { 'x' } else { '-' });
        s
    }
}

/// Metadatos de un archivo/directorio
#[derive(Debug, Clone)]
pub struct Metadata {
    pub node_type: NodeType,
    pub size: u64,
    pub owner_uid: u32,
    pub owner_gid: u32,
    pub permissions: Permissions,
    pub created: u64,
    pub modified: u64,
}

impl Metadata {
    pub fn new_file(size: u64, uid: u32, gid: u32) -> Self {
        let now = crate::curr_arch::get_ticks();
        Self {
            node_type: NodeType::File,
            size,
            owner_uid: uid,
            owner_gid: gid,
            permissions: Permissions::read_write(),
            created: now,
            modified: now,
        }
    }
    
    pub fn new_dir(uid: u32, gid: u32) -> Self {
        let now = crate::curr_arch::get_ticks();
        Self {
            node_type: NodeType::Directory,
            size: 0,
            owner_uid: uid,
            owner_gid: gid,
            permissions: Permissions::all(),
            created: now,
            modified: now,
        }
    }
}

/// Entrada de directorio
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub node_type: NodeType,
    pub size: u64,
}

/// Errores del filesystem
#[derive(Debug, Clone)]
pub enum FsError {
    NotFound,
    AlreadyExists,
    NotADirectory,
    NotAFile,
    IsADirectory,
    NotEmpty,
    PermissionDenied,
    InvalidPath,
    NoSpace,
    ReadOnly,
    IoError,
}

impl FsError {
    pub fn as_str(&self) -> &'static str {
        match self {
            FsError::NotFound => "No such file or directory",
            FsError::AlreadyExists => "File exists",
            FsError::NotADirectory => "Not a directory",
            FsError::NotAFile => "Not a file",
            FsError::IsADirectory => "Is a directory",
            FsError::NotEmpty => "Directory not empty",
            FsError::PermissionDenied => "Permission denied",
            FsError::InvalidPath => "Invalid path",
            FsError::NoSpace => "No space left on device",
            FsError::ReadOnly => "Read-only file system",
            FsError::IoError => "I/O error",
        }
    }
}

pub type FsResult<T> = Result<T, FsError>;

/// Trait para filesystems
pub trait FileSystem: Send + Sync {
    fn name(&self) -> &str;
    fn stat(&self, path: &str) -> FsResult<Metadata>;
    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>>;
    fn read(&self, path: &str) -> FsResult<Vec<u8>>;
    fn write(&self, path: &str, data: &[u8]) -> FsResult<()>;
    fn mkdir(&self, path: &str) -> FsResult<()>;
    fn create(&self, path: &str) -> FsResult<()>;
    fn remove(&self, path: &str) -> FsResult<()>;
    fn rmdir(&self, path: &str) -> FsResult<()>;
    fn rename(&self, from: &str, to: &str) -> FsResult<()>;
    fn utime(&self, _path: &str, _created: u64, _modified: u64) -> FsResult<()> { Ok(()) }
    fn stats(&self) -> (u64, u64) { (0, 0) }
}

/// VFS Global
static VFS: Mutex<Option<Vfs>> = Mutex::new(None);

/// Directorio de trabajo actual
static CWD: Mutex<String> = Mutex::new(String::new());

/// Virtual File System/// Virtual File System
pub struct Vfs {
    root: Box<dyn FileSystem>,
    fs_type: String,
}

impl Vfs {
    pub fn new(root: Box<dyn FileSystem>, fs_type: &str) -> Self {
        Self { 
            root,
            fs_type: String::from(fs_type),
        }
    }
    
    pub fn filesystem_type(&self) -> &str {
        &self.fs_type
    }
    
    pub fn is_persistent(&self) -> bool {
        false
    }
}

/// Resultado de la inicialización
pub enum InitResult {
    RamFs,
}

/// Inicializa el VFS - Solo RamFS (persistencias eliminadas por seguridad)
pub fn init() -> InitResult {
    crate::serial_println!("[FS] Inicializando VFS (Modo solo RAM)...");
    init_ramfs();
    InitResult::RamFs
}

/// Verifica si el filesystem necesita estructura inicial
pub fn needs_initial_structure() -> bool {
    // Si /etc no existe, necesitamos crear la estructura
    match readdir("/") {
        Ok(entries) => entries.is_empty() || !entries.iter().any(|e| e.name == "etc"),
        Err(_) => true,
    }
}

/// Crea la estructura inicial de directorios
pub fn create_initial_structure() {
    let dirs = [
        "/bin",
        "/etc",
        "/home",
        "/home/root",
        "/home/guest",
        "/home/mesa",
        "/tmp",
        "/var",
        "/var/log",
    ];
    
    for dir in dirs {
        if let Err(e) = mkdir(dir) {
            if !matches!(e, FsError::AlreadyExists) {
                crate::serial_println!("[FS] Warning: No se pudo crear {}: {}", dir, e.as_str());
            }
        }
    }
    
    // Crear archivos de configuración
    let files = [
        ("/etc/hostname", "mesa-os"),
        ("/etc/version", "0.4.0"),
        ("/etc/motd", "Welcome to Mesa OS!\nType 'help' for commands.\n"),
        ("/home/root/.profile", "# Root profile\n"),
        ("/home/guest/readme.txt", "Hello, guest user!\n"),
    ];
    
    for (path, content) in files {
        if let Err(e) = write(path, content.as_bytes()) {
            crate::serial_println!("[FS] Warning: No se pudo crear {}: {}", path, e.as_str());
        }
    }
    
    // Escribir ELF embebido
    crate::serial_println!("[FS] Escribiendo binarios embebidos...");
    match write("/bin/hello.elf", crate::userland::programs::HELLO_ELF) {
        Ok(_) => crate::serial_println!("[FS] /bin/hello.elf OK"),
        Err(e) => crate::serial_println!("[FS] Error escribiendo /bin/hello.elf: {}", e.as_str()),
    }
    
    crate::serial_println!("[FS] Estructura inicial creada");
}

/// Inicializa con RamFS (único modo soportado)
fn init_ramfs() {
    let ramfs = ramfs::RamFs::new();
    
    // Crear estructura en RamFS
    let _ = ramfs.mkdir("/bin");
    let _ = ramfs.mkdir("/etc");
    let _ = ramfs.mkdir("/home");
    let _ = ramfs.mkdir("/home/root");
    let _ = ramfs.mkdir("/home/guest");
    let _ = ramfs.mkdir("/home/mesa");
    let _ = ramfs.mkdir("/tmp");
    let _ = ramfs.mkdir("/var");
    let _ = ramfs.mkdir("/var/log");
    let _ = ramfs.mkdir("/mnt");
    
    let _ = ramfs.write("/etc/hostname", b"mesa-os");
    let _ = ramfs.write("/etc/motd", b"Welcome to Mesa OS!\nType 'help' for commands.\n");
    let _ = ramfs.write("/etc/version", b"0.4.0\n");
    let _ = ramfs.write("/home/root/.profile", b"# Root profile\nexport PATH=/bin\n");
    let _ = ramfs.write("/home/guest/readme.txt", b"Hello, guest user!\n");
    let _ = ramfs.write("/tmp/test.txt", b"This is a test file.\n");
    
    // Escribir ELF embebido
    let _ = ramfs.write("/bin/hello.elf", crate::userland::programs::HELLO_ELF);
    
    *VFS.lock() = Some(Vfs::new(Box::new(ramfs), "ramfs"));
    *CWD.lock() = String::from("/");
    
    crate::klog_info!("VFS initialized with RamFS (volatile)");
}

/// Sincronización deshabilitada
pub fn sync() -> FsResult<()> {
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// API PÚBLICA
// ══════════════════════════════════════════════════════════════════════════════

/// Obtiene el directorio de trabajo actual
pub fn cwd() -> String {
    CWD.lock().clone()
}

/// Cambia el directorio de trabajo
pub fn chdir(path: &str) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    
    let meta = stat(&abs_path)?;
    if meta.node_type != NodeType::Directory {
        return Err(FsError::NotADirectory);
    }
    
    *CWD.lock() = abs_path;
    Ok(())
}

/// Lee metadatos
pub fn stat(path: &str) -> FsResult<Metadata> {
    let abs_path = path::resolve(path)?;
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.stat(&abs_path)
}

/// Lista directorio
pub fn readdir(path: &str) -> FsResult<Vec<DirEntry>> {
    let abs_path = path::resolve(path)?;
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.readdir(&abs_path)
}

/// Lee archivo
pub fn read(path: &str) -> FsResult<Vec<u8>> {
    let abs_path = path::resolve(path)?;
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.read(&abs_path)
}

/// Lee archivo como string
pub fn read_to_string(path: &str) -> FsResult<String> {
    let data = read(path)?;
    String::from_utf8(data).map_err(|_| FsError::IoError)
}

/// Escribe archivo
pub fn write(path: &str, data: &[u8]) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.write(&abs_path, data)
}

/// Crea directorio
pub fn mkdir(path: &str) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.mkdir(&abs_path)
}

/// Crea archivo vacío
pub fn touch(path: &str) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    
    match vfs.root.stat(&abs_path) {
        Ok(_) => Ok(()),
        Err(FsError::NotFound) => vfs.root.create(&abs_path),
        Err(e) => Err(e),
    }
}

/// Elimina archivo
pub fn rm(path: &str) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.remove(&abs_path)
}

/// Elimina directorio
pub fn rmdir(path: &str) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.rmdir(&abs_path)
}

/// Mueve/renombra
pub fn mv(from: &str, to: &str) -> FsResult<()> {
    let abs_from = path::resolve(from)?;
    let abs_to = path::resolve(to)?;
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.rename(&abs_from, &abs_to)
}

/// Verifica si un path existe
pub fn exists(path: &str) -> bool {
    stat(path).is_ok()
}

/// Verifica si es directorio
pub fn is_dir(path: &str) -> bool {
    stat(path).map(|m| m.node_type == NodeType::Directory).unwrap_or(false)
}

/// Verifica si es archivo
pub fn is_file(path: &str) -> bool {
    stat(path).map(|m| m.node_type == NodeType::File).unwrap_or(false)
}

/// Obtiene el tipo de filesystem montado
pub fn filesystem_type() -> String {
    VFS.lock()
        .as_ref()
        .map(|vfs| vfs.filesystem_type().to_string())
        .unwrap_or_else(|| String::from("none"))
}

/// Verifica si el filesystem es persistente
pub fn is_persistent() -> bool {
    VFS.lock()
        .as_ref()
        .map(|vfs| vfs.is_persistent())
        .unwrap_or(false)
}

/// Monta un nuevo filesystem
pub fn mount(fs: Box<dyn FileSystem>, fs_type: &str) {
    *VFS.lock() = Some(Vfs::new(fs, fs_type));
    crate::serial_println!("[FS] Mounted {} filesystem", fs_type);
    crate::klog_info!("Filesystem mounted: {}", fs_type);
}
pub fn stats() -> (u64, u64) {
    let vfs = VFS.lock();
    if let Some(ref v) = *vfs {
        return v.root.stats();
    }
    (0, 0)
}
