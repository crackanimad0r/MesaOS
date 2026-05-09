//! Virtual File System (VFS) para Mesa OS

pub mod ramfs;
pub mod path;
#[cfg(target_arch = "x86_64")]
pub mod mesafs;
pub mod partition;
pub mod sync;

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

/// Virtual File System
pub struct Vfs {
    root: Box<dyn FileSystem>,
    backend: Option<Box<dyn FileSystem>>,
    fs_type: String,
    persistent: bool,
}

impl Vfs {
    pub fn new(root: Box<dyn FileSystem>, fs_type: &str, persistent: bool) -> Self {
        Self { 
            root,
            backend: None,
            fs_type: String::from(fs_type),
            persistent,
        }
    }

    pub fn new_live_ram(ram: Box<dyn FileSystem>, backend: Box<dyn FileSystem>) -> Self {
        Self {
            root: ram,
            backend: Some(backend),
            fs_type: String::from("live-ram"),
            persistent: true,
        }
    }
    
    pub fn filesystem_type(&self) -> &str {
        &self.fs_type
    }
    
    pub fn is_persistent(&self) -> bool {
        self.persistent
    }
}

/// Resultado de la inicialización
pub enum InitResult {
    MesaFs,
    LiveRam,
    RamFs,
    NoDisk,
}

/// Inicializa el VFS - intenta MesaFS primero, luego RamFS
pub fn init() -> InitResult {
    crate::serial_println!("[FS] Inicializando VFS...");
    
    #[cfg(target_arch = "x86_64")]
    {
        // 1. Intentar NVMe primero
        if crate::drivers::nvme::NVME.lock().is_some() {
            let nvme_dev = alloc::sync::Arc::new(crate::drivers::nvme::NvmeBlockDevice);
            let start_lba = partition::find_mesafs_partition(nvme_dev.as_ref()).map(|(s, _)| s).unwrap_or(0);
            
            if let Ok(m) = try_mount_and_initialize(nvme_dev.clone(), start_lba, "NVMe") {
                return m;
            }
        }

        // 2. Intentar ATA
        if let Some(ata_info) = crate::drivers::ata::disk_info() {
            let ata_dev = alloc::sync::Arc::new(crate::drivers::ata::AtaBlockDevice);
            let start_lba = partition::find_mesafs_partition(ata_dev.as_ref()).map(|(s, _)| s).unwrap_or(0);
            
            if let Ok(m) = try_mount_and_initialize(ata_dev.clone(), start_lba, "ATA") {
                return m;
            }
        }

        // 3. Intentar dispositivos USB
        let usb_devices = crate::drivers::usb::msc::MSC_DEVICES.lock();
        for (i, dev) in usb_devices.iter().enumerate() {
            crate::serial_println!("[FS] Intentando disco USB {}", i);
            let start_lba = partition::find_mesafs_partition(dev.as_ref()).map(|(s, _)| s).unwrap_or(0);
            
            if let Ok(m) = try_mount_and_initialize(dev.clone(), start_lba, "USB") {
                return m;
            }
        }
        drop(usb_devices);

        // 4. Si hay discos pero no tienen filesystem, intentar auto-crear en el primero disponible (Priorizar NVMe, luego ATA)
        if crate::drivers::nvme::NVME.lock().is_some() {
            let nvme_dev = alloc::sync::Arc::new(crate::drivers::nvme::NvmeBlockDevice);
            if let Ok(mesafs) = auto_create_mesafs_on_dev(nvme_dev, "NVMe") {
                return init_live_ram(Box::new(mesafs));
            }
        }
        if let Some(ata_info) = crate::drivers::ata::disk_info() {
            let ata_dev = alloc::sync::Arc::new(crate::drivers::ata::AtaBlockDevice);
            if let Ok(mesafs) = auto_create_mesafs_on_dev(ata_dev, "ATA") {
                return init_live_ram(Box::new(mesafs));
            }
        }

        let usb_devices = crate::drivers::usb::msc::MSC_DEVICES.lock();
        if let Some(dev) = usb_devices.first() {
             if let Ok(mesafs) = auto_create_mesafs_on_dev(dev.clone(), "USB") {
                return init_live_ram(Box::new(mesafs));
             }
        }
        drop(usb_devices);
    }
    
    // Fallback a RamFS
    crate::serial_println!("[FS] Usando RamFS como fallback...");
    init_ramfs();
    
    #[cfg(target_arch = "x86_64")]
    if crate::drivers::nvme::NVME.lock().is_some() || crate::drivers::ata::disk_info().is_some() || !crate::drivers::usb::msc::MSC_DEVICES.lock().is_empty() {
        return InitResult::RamFs;
    }
    
    InitResult::NoDisk
}

#[cfg(target_arch = "x86_64")]
fn try_mount_and_initialize(dev: alloc::sync::Arc<dyn crate::drivers::block::BlockDevice>, start_lba: u64, name: &str) -> Result<InitResult, &'static str> {
    for attempt in 1..=2 {
        match mesafs::MesaFs::mount_on_dev(dev.clone(), start_lba) {
            Ok(mesafs) => {
                crate::serial_println!("[FS] MesaFS detectado en {}, activando Live RAM...", name);
                return Ok(init_live_ram(Box::new(mesafs)));
            }
            Err(_) => {
                if attempt < 2 { core::hint::spin_loop(); }
            }
        }
    }
    Err("Mount failed")
}

#[cfg(target_arch = "x86_64")]
fn auto_create_mesafs_on_dev(dev: alloc::sync::Arc<dyn crate::drivers::block::BlockDevice>, name: &str) -> Result<mesafs::MesaFs, &'static str> {
    let capacity = dev.capacity();
    if capacity == 0 { return Err("No capacity"); }

    crate::mesa_println!("  [FS] ¿Deseas inicializar disco {}? (s/n)", name);
    // En un sistema real preguntaríamos, aquí asumimos que si no hay nada, el usuario quiere usarlo.
    
    let start_lba = match partition::read_mbr(dev.as_ref()) {
        Ok(_) => partition::find_mesafs_partition(dev.as_ref()).map(|(s, _)| s).unwrap_or(0),
        Err(_) => {
            crate::serial_println!("[FS] Creando tabla de particiones en {}...", name);
            partition::create_mesafs_partition(dev.as_ref())?
        }
    };

    let blocks = ((capacity.saturating_sub(start_lba)) / 2) as u32; // 1KB blocks from 512B sectors
    let blocks = blocks.min(204800); // Max 200MB for auto-init

    crate::serial_println!("[FS] Formateando MesaFS en {} LBA {}...", name, start_lba);
    mesafs::MesaFs::create_on_dev(dev, start_lba, blocks)
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

/// Inicializa con RamFS (fallback)
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
    
    *VFS.lock() = Some(Vfs::new(Box::new(ramfs), "ramfs", false));
    *CWD.lock() = String::from("/");
    
    crate::klog_info!("VFS initialized with RamFS (volatile)");
}

/// Inicializa Live RAM usando un backend persistente
pub fn init_live_ram(backend: Box<dyn FileSystem>) -> InitResult {
    crate::serial_println!("[FS] Inicializando Live RAM Mode...");
    
    let ram = ramfs::RamFs::new();
    
    // Cargar contenido inicial desde el disco a la RAM
    if let Err(e) = sync::load_recursive(backend.as_ref(), &ram, "/") {
        crate::serial_println!("[FS] Error cargando datos a RAM: {}", e.as_str());
    } else {
        crate::serial_println!("[FS] Datos cargados exitosamente a RAM");
    }
    
    *VFS.lock() = Some(Vfs::new_live_ram(Box::new(ram), backend));
    *CWD.lock() = String::from("/");
    
    if needs_initial_structure() {
        create_initial_structure();
    }
    
    InitResult::LiveRam
}

/// Sincroniza la RAM con el disco si existe un backend
pub fn sync() -> FsResult<()> {
    let vfs_guard = VFS.lock();
    if let Some(ref vfs) = *vfs_guard {
        if let Some(ref backend) = vfs.backend {
            crate::serial_println!("[FS] Sincronizando RAM -> Disco...");
            return sync::sync_recursive(vfs.root.as_ref(), backend.as_ref(), "/");
        }
    }
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
pub fn mount(fs: Box<dyn FileSystem>, fs_type: &str, persistent: bool) {
    *VFS.lock() = Some(Vfs::new(fs, fs_type, persistent));
    crate::serial_println!("[FS] Mounted {} filesystem", fs_type);
    crate::klog_info!("Filesystem mounted: {}", fs_type);
}
pub fn stats() -> (u64, u64) {
    let vfs = VFS.lock();
    if let Some(ref v) = *vfs {
        if let Some(ref backend) = v.backend {
            return backend.stats();
        }
        return v.root.stats();
    }
    (0, 0)
}
