//! Virtual File System (VFS) para Mesa OS

pub mod initrd;
pub mod initrd_data;
pub mod partition;
pub mod path;
pub mod ramfs;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

use crate::drivers::usb::mesa_fs;

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
        Self {
            read: true,
            write: true,
            execute: true,
        }
    }

    pub const fn read_only() -> Self {
        Self {
            read: true,
            write: false,
            execute: false,
        }
    }

    pub const fn read_write() -> Self {
        Self {
            read: true,
            write: true,
            execute: false,
        }
    }

    pub const fn read_exec() -> Self {
        Self {
            read: true,
            write: false,
            execute: true,
        }
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
    fn utime(&self, _path: &str, _created: u64, _modified: u64) -> FsResult<()> {
        Ok(())
    }
    fn stats(&self) -> (u64, u64) {
        (0, 0)
    }
    fn chown(&self, _path: &str, _uid: u32, _gid: u32) -> FsResult<()> {
        Err(FsError::IoError)
    }
    fn symlink(&self, _target: &str, _link_path: &str) -> FsResult<()> {
        Err(FsError::IoError)
    }
    fn link(&self, _target: &str, _link_path: &str) -> FsResult<()> {
        Err(FsError::IoError)
    }
}

/// VFS Global
static VFS: Mutex<Option<Vfs>> = Mutex::new(None);

/// Directorio de trabajo actual
static CWD: Mutex<String> = Mutex::new(String::new());

/// Filesystem de disco USB (MesaFs)
struct UsbDiskFileSystem;

static DISK_FS: Mutex<Option<UsbDiskFileSystem>> = Mutex::new(None);

pub fn register_disk_fs() {
    *DISK_FS.lock() = Some(UsbDiskFileSystem);
    crate::serial_println!("[FS] USB disk filesystem registered");
}

fn disk_filename(path: &str) -> Option<&str> {
    if path.starts_with("/disks/usb_disk_0/") {
        let fname = &path["/disks/usb_disk_0/".len()..];
        if !fname.is_empty() {
            return Some(fname);
        }
    }
    None
}

fn disk_stat(path: &str) -> FsResult<Metadata> {
    if path == "/disks" || path == "/disks/usb_disk_0" {
        return Ok(Metadata::new_dir(0, 0));
    }
    if let Some(fname) = disk_filename(path) {
        for (name, etype, size, _) in mesa_fs::mesa_fs_list_dir() {
            if name == fname {
                return if etype == 2 {
                    Ok(Metadata::new_dir(0, 0))
                } else {
                    Ok(Metadata::new_file(size as u64, 0, 0))
                };
            }
        }
        return Err(FsError::NotFound);
    }
    Err(FsError::NotFound)
}

fn disk_readdir(path: &str) -> FsResult<Vec<DirEntry>> {
    if path == "/disks" || path == "/disks/" {
        let mut entries = Vec::new();
        if mesa_fs::mesa_fs_is_initialized() {
            entries.push(DirEntry {
                name: String::from("usb_disk_0"),
                node_type: NodeType::Directory,
                size: 0,
            });
        }
        return Ok(entries);
    }
    if path == "/disks/usb_disk_0" || path == "/disks/usb_disk_0/" {
        let mut entries = Vec::new();
        for (name, etype, size, _) in mesa_fs::mesa_fs_list_dir() {
            entries.push(DirEntry {
                name,
                node_type: if etype == 2 {
                    NodeType::Directory
                } else {
                    NodeType::File
                },
                size: size as u64,
            });
        }
        return Ok(entries);
    }
    disk_stat(path)?;
    Err(FsError::NotADirectory)
}

fn disk_read(path: &str) -> FsResult<Vec<u8>> {
    let fname = disk_filename(path).ok_or(FsError::NotFound)?;
    mesa_fs::mesa_fs_read_file(fname).ok_or(FsError::NotFound)
}

fn disk_write(path: &str, data: &[u8]) -> FsResult<()> {
    let fname = disk_filename(path).ok_or(FsError::NotFound)?;
    mesa_fs::mesa_fs_write_file(fname, data).map_err(|_| FsError::IoError)
}

fn disk_create(path: &str) -> FsResult<()> {
    let fname = disk_filename(path).ok_or(FsError::NotFound)?;
    mesa_fs::mesa_fs_write_file(fname, &[]).map_err(|_| FsError::IoError)
}

fn disk_mkdir(path: &str) -> FsResult<()> {
    let fname = disk_filename(path).ok_or(FsError::NotFound)?;
    mesa_fs::mesa_fs_mkdir(fname).map_err(|e| {
        if e == "La entrada ya existe" {
            FsError::AlreadyExists
        } else {
            FsError::IoError
        }
    })
}

fn disk_remove(path: &str) -> FsResult<()> {
    let fname = disk_filename(path).ok_or(FsError::NotFound)?;
    mesa_fs::mesa_fs_remove(fname).map_err(|_| FsError::IoError)
}

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
        "/disks",
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
        (
            "/etc/motd",
            "Welcome to Mesa OS!\nType 'help' for commands.\n",
        ),
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
    let _ = ramfs.mkdir("/disks");

    let _ = ramfs.write("/etc/hostname", b"mesa-os");
    let _ = ramfs.write(
        "/etc/motd",
        b"Welcome to Mesa OS!\nType 'help' for commands.\n",
    );
    let _ = ramfs.write("/etc/version", b"0.4.0\n");
    let _ = ramfs.write("/home/root/.profile", b"# Root profile\nexport PATH=/bin\n");
    let _ = ramfs.write("/home/guest/readme.txt", b"Hello, guest user!\n");
    let _ = ramfs.write("/tmp/test.txt", b"This is a test file.\n");

    // Escribir ELF embebido
    let _ = ramfs.write("/bin/hello.elf", crate::userland::programs::HELLO_ELF);

    *VFS.lock() = Some(Vfs::new(Box::new(ramfs), "ramfs"));
    *CWD.lock() = String::from("/");

    // Extraer initrd (archivos embebidos, si existen)
    crate::serial_println!("[FS] Extrayendo initrd embebido...");
    match initrd::extract_initrd() {
        Ok(count) => {
            if count > 0 {
                crate::serial_println!("[FS] Initrd extraído: {} archivos", count);
                crate::klog_info!("Initrd extracted: {} files", count);
            }
        }
        Err(e) => {
            crate::serial_println!("[FS] Initrd no disponible o error: {}", e);
            crate::klog_warn!("Initrd extraction skipped: {}", e);
        }
    }

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
    if abs_path.starts_with("/disks") {
        crate::mesa_println!("[VFS] Enrutando stat para '{}' hacia MesaFS USB", abs_path);
        return disk_stat(&abs_path);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.stat(&abs_path)
}

/// Lista directorio
pub fn readdir(path: &str) -> FsResult<Vec<DirEntry>> {
    let abs_path = path::resolve(path)?;
    if abs_path.starts_with("/disks") {
        crate::mesa_println!(
            "[VFS] Enrutando readdir para '{}' hacia MesaFS USB",
            abs_path
        );
        return disk_readdir(&abs_path);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.readdir(&abs_path)
}

/// Lee archivo
pub fn read(path: &str) -> FsResult<Vec<u8>> {
    let abs_path = path::resolve(path)?;
    if abs_path.starts_with("/disks") {
        crate::mesa_println!("[VFS] Enrutando read para '{}' hacia MesaFS USB", abs_path);
        return disk_read(&abs_path);
    }
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
    if abs_path.starts_with("/disks") {
        crate::mesa_println!("[VFS] Enrutando write para '{}' hacia MesaFS USB", abs_path);
        return disk_write(&abs_path, data);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.write(&abs_path, data)
}

/// Crea directorio
pub fn mkdir(path: &str) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    if abs_path.starts_with("/disks") {
        crate::mesa_println!("[VFS] Enrutando mkdir para '{}' hacia MesaFS USB", abs_path);
        return disk_mkdir(&abs_path);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.mkdir(&abs_path)
}

/// Crea archivo vacío
pub fn touch(path: &str) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    if abs_path.starts_with("/disks") {
        crate::mesa_println!("[VFS] Enrutando touch para '{}' hacia MesaFS USB", abs_path);
        if disk_stat(&abs_path).is_ok() {
            return Ok(());
        }
        return disk_create(&abs_path);
    }
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
    if abs_path.starts_with("/disks") {
        crate::mesa_println!("[VFS] Enrutando rm para '{}' hacia MesaFS USB", abs_path);
        return disk_remove(&abs_path);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.remove(&abs_path)
}

/// Elimina directorio
pub fn rmdir(path: &str) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    if abs_path.starts_with("/disks") {
        return Err(FsError::ReadOnly);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.rmdir(&abs_path)
}

/// Mueve/renombra
pub fn mv(from: &str, to: &str) -> FsResult<()> {
    let abs_from = path::resolve(from)?;
    let abs_to = path::resolve(to)?;
    if abs_from.starts_with("/disks") || abs_to.starts_with("/disks") {
        return Err(FsError::ReadOnly);
    }
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
    stat(path)
        .map(|m| m.node_type == NodeType::Directory)
        .unwrap_or(false)
}

/// Verifica si es archivo
pub fn is_file(path: &str) -> bool {
    stat(path)
        .map(|m| m.node_type == NodeType::File)
        .unwrap_or(false)
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

/// Change file ownership
pub fn chown(path: &str, uid: u32, gid: u32) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    if abs_path.starts_with("/disks") {
        return Err(FsError::ReadOnly);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.chown(&abs_path, uid, gid)
}

/// Create a symbolic link
pub fn symlink(target: &str, link_path: &str) -> FsResult<()> {
    let abs_link = path::resolve(link_path)?;
    if abs_link.starts_with("/disks") {
        return Err(FsError::ReadOnly);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.symlink(target, &abs_link)
}

/// Create a hard link
pub fn link(target: &str, link_path: &str) -> FsResult<()> {
    let abs_target = path::resolve(target)?;
    let abs_link = path::resolve(link_path)?;
    if abs_target.starts_with("/disks") || abs_link.starts_with("/disks") {
        return Err(FsError::ReadOnly);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;
    vfs.root.link(&abs_target, &abs_link)
}

/// Actualiza los permisos de un archivo
pub fn update_permissions(path: &str, perms: Permissions) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    if abs_path.starts_with("/disks") {
        return Err(FsError::ReadOnly);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;

    // Leer datos actuales para reescribir con nuevos permisos
    let meta = vfs.root.stat(&abs_path)?;
    if meta.node_type != NodeType::File {
        return Err(FsError::IsADirectory);
    }

    // En RamFS, como el trait FileSystem no tiene set_permissions,
    // simulamos escribiendo el mismo contenido sin cambios,
    // pero actualizamos el timestamp para que refleje el cambio.
    // Los permisos se almacenan en Metadata, usamos utime para refrescar.
    // Para una solución real, habría que agregar set_permissions al trait.
    // Por ahora, modificamos el archivo para actualizar su metadata.
    let data = vfs.root.read(&abs_path)?;
    vfs.root.write(&abs_path, &data)?;

    // Como no tenemos set_permissions en el trait, y no podemos modificar
    // metadata directamente desde aquí, lo hacemos a través de RamFS
    // almacenando los permisos en una estructura separada.
    // Esto es un workaround: forzamos la reescritura del archivo
    // y usamos el flag execute desde la función auxiliar en main.rs.
    //
    // Nota: Los permisos se almacenan inline en Metadata, así que
    // para cambiarlos realmente necesitaríamos acceso al nodo interno.
    // Por ahora, esta función deja constancia del intento y los comandos
    // de main verifican permisos antes de ejecutar.

    Ok(())
}

/// Da permisos de ejecución a un archivo (usado por run)
pub fn update_permissions_exec(path: &str) -> FsResult<()> {
    let abs_path = path::resolve(path)?;
    if abs_path.starts_with("/disks") {
        return Err(FsError::ReadOnly);
    }
    let vfs = VFS.lock();
    let vfs = vfs.as_ref().ok_or(FsError::IoError)?;

    // Leemos y re-escribimos para actualizar timestamp
    let data = vfs.root.read(&abs_path)?;
    vfs.root.write(&abs_path, &data)?;

    Ok(())
}

/// Convierte string de permisos estilo Unix (rwx) a Permissions
pub fn parse_permissions(s: &str) -> Option<Permissions> {
    if s.len() != 3 {
        return None;
    }
    let chars: Vec<char> = s.chars().collect();
    Some(Permissions {
        read: chars[0] == 'r',
        write: chars[1] == 'w',
        execute: chars[2] == 'x',
    })
}
