// mesa_kernel/src/users/mod.rs
//! Sistema de usuarios con contraseñas y permisos
//!
//! Mejorado: usa hash con salt del módulo security para mayor seguridad

use crate::security;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;

/// ID de usuario
pub type Uid = u32;
/// ID de grupo  
pub type Gid = u32;

/// Usuario del sistema
#[derive(Debug, Clone)]
pub struct User {
    pub uid: Uid,
    pub gid: Gid,
    pub name: String,
    pub password_hash: security::PasswordHash,
    pub password_salt: security::PasswordSalt,
    pub home: String,
    pub shell: String,
}

impl User {
    pub fn root() -> Self {
        let salt = security::generate_salt();
        Self {
            uid: 0,
            gid: 0,
            name: String::from("root"),
            password_hash: security::hash_password("", &salt),
            password_salt: salt,
            home: String::from("/root"),
            shell: String::from("/bin/sh"),
        }
    }

    pub fn guest() -> Self {
        let salt = security::generate_salt();
        Self {
            uid: 1000,
            gid: 1000,
            name: String::from("guest"),
            password_hash: security::hash_password("guest", &salt),
            password_salt: salt,
            home: String::from("/home/guest"),
            shell: String::from("/bin/sh"),
        }
    }

    pub fn mesa() -> Self {
        let salt = security::generate_salt();
        Self {
            uid: 1001,
            gid: 1000,
            name: String::from("mesa"),
            password_hash: security::hash_password("mesa", &salt),
            password_salt: salt,
            home: String::from("/home/mesa"),
            shell: String::from("/bin/sh"),
        }
    }

    pub fn check_password(&self, password: &str) -> bool {
        security::verify_password(password, self.password_hash, &self.password_salt)
    }
}

/// Grupo del sistema
#[derive(Debug, Clone)]
pub struct Group {
    pub gid: Gid,
    pub name: String,
    pub members: Vec<Uid>,
}

// Estado global
static USERS: Mutex<Vec<User>> = Mutex::new(Vec::new());
static GROUPS: Mutex<Vec<Group>> = Mutex::new(Vec::new());
static CURRENT_UID: Mutex<Uid> = Mutex::new(0);
static CURRENT_GID: Mutex<Gid> = Mutex::new(0);
static CURRENT_NAME: Mutex<String> = Mutex::new(String::new());
static LOGGED_IN: Mutex<bool> = Mutex::new(false);

pub fn init() {
    *CURRENT_NAME.lock() = String::from("root");
    crate::serial_println!("[USERS] Inicializando sistema de usuarios...");

    // Intentar cargar desde /etc/passwd
    if !load_users() {
        crate::serial_println!("[USERS] No se encontró /etc/passwd, usando usuarios por defecto.");
        let mut users = USERS.lock();
        users.push(User::root());
        users.push(User::guest());
        users.push(User::mesa());

        let mut groups = GROUPS.lock();
        groups.push(Group {
            gid: 0,
            name: String::from("root"),
            members: vec![0],
        });
        groups.push(Group {
            gid: 1000,
            name: String::from("users"),
            members: vec![1000, 1001],
        });

        // Guardar por primera vez
        drop(users);
        drop(groups);
        save_users();
    }

    crate::serial_println!("[USERS] Sistema de usuarios listo");
}

/// Convierte un u128 a string decimal
fn hash_to_string(hash: security::PasswordHash) -> String {
    let mut result = String::new();
    let mut h = hash;
    if h == 0 {
        return String::from("0");
    }
    while h > 0 {
        let digit = (h % 10) as u8;
        result.insert(0, (b'0' + digit) as char);
        h /= 10;
    }
    result
}

/// Convierte un string decimal a u128
fn string_to_hash(s: &str) -> Option<security::PasswordHash> {
    let mut result: security::PasswordHash = 0;
    for c in s.chars() {
        if c < '0' || c > '9' {
            return None;
        }
        result = result
            .wrapping_mul(10)
            .wrapping_add((c as u8 - b'0') as u128);
    }
    Some(result)
}

/// Convierte un salt a string hex
fn salt_to_hex(salt: &security::PasswordSalt) -> String {
    let mut result = String::with_capacity(security::SALT_LEN * 2);
    for &b in salt.iter() {
        result.push_str(&alloc::format!("{:02x}", b));
    }
    result
}

/// Convierte un string hex a salt
fn hex_to_salt(hex: &str) -> Option<security::PasswordSalt> {
    if hex.len() != security::SALT_LEN * 2 {
        return None;
    }
    let mut salt = [0u8; security::SALT_LEN];
    for i in 0..security::SALT_LEN {
        let byte_str = &hex[i * 2..(i + 1) * 2];
        salt[i] = u8::from_str_radix(byte_str, 16).ok()?;
    }
    Some(salt)
}

pub fn save_users() {
    let content = {
        let users = USERS.lock();
        let mut c = String::new();

        for u in users.iter() {
            c.push_str(&alloc::format!(
                "{}:{}:{}:{}:{}:{}:{}\n",
                u.uid,
                u.gid,
                u.name,
                hash_to_string(u.password_hash),
                salt_to_hex(&u.password_salt),
                u.home,
                u.shell
            ));
        }
        c
    };

    let _ = crate::fs::write("/etc/passwd", content.as_bytes());
}

pub fn load_users() -> bool {
    if let Ok(content) = crate::fs::read_to_string("/etc/passwd") {
        let mut users = USERS.lock();
        users.clear();

        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            // Nuevo formato (7 campos): uid:gid:name:hash:salt_hex:home:shell
            // Compatible con formato antiguo (6 campos) - migración automática
            if parts.len() == 7 {
                if let (Ok(uid), Ok(gid)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                    if let Some(hash) = string_to_hash(parts[3]) {
                        if let Some(salt) = hex_to_salt(parts[4]) {
                            users.push(User {
                                uid,
                                gid,
                                name: String::from(parts[2]),
                                password_hash: hash,
                                password_salt: salt,
                                home: String::from(parts[5]),
                                shell: String::from(parts[6]),
                            });
                        }
                    }
                }
            } else if parts.len() == 6 {
                // Formato antiguo: migrar automáticamente con nuevo salt
                if let (Ok(uid), Ok(gid)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                    let name = String::from(parts[2]);
                    let salt = security::generate_salt();
                    let hash = security::hash_password("", &salt);
                    users.push(User {
                        uid,
                        gid,
                        name,
                        password_hash: hash,
                        password_salt: salt,
                        home: String::from(parts[4]),
                        shell: String::from(parts[5]),
                    });
                }
            }
        }
        return !users.is_empty();
    }
    false
}

pub fn add_user(name: &str, password: &str, is_admin: bool) -> Result<(), &'static str> {
    if password.len() < security::MIN_PASSWORD_LEN {
        return Err("Contraseña demasiado corta (mínimo 4 caracteres)");
    }

    let mut users = USERS.lock();
    if users.iter().any(|u| u.name == name) {
        return Err("El usuario ya existe");
    }

    let max_uid = users.iter().map(|u| u.uid).max().unwrap_or(1000);
    let new_uid = if is_admin { 0 } else { max_uid + 1 };
    let salt = security::generate_salt();

    users.push(User {
        uid: new_uid,
        gid: if is_admin { 0 } else { 1000 },
        name: String::from(name),
        password_hash: security::hash_password(password, &salt),
        password_salt: salt,
        home: alloc::format!("/home/{}", name),
        shell: String::from("/bin/sh"),
    });

    drop(users);
    save_users();
    security::audit_log(
        security::AuditSeverity::Info,
        &alloc::format!("User '{}' created", name),
    );
    Ok(())
}

pub fn remove_user(name: &str) -> Result<(), &'static str> {
    if name == "root" {
        return Err("No se puede borrar root");
    }

    if !crate::users::is_root() {
        return Err("Permiso denegado: solo root puede eliminar usuarios");
    }

    let mut users = USERS.lock();
    let pos = users
        .iter()
        .position(|u| u.name == name)
        .ok_or("Usuario no encontrado")?;
    users.remove(pos);

    drop(users);
    save_users();
    security::audit_log(
        security::AuditSeverity::Warning,
        &alloc::format!("User '{}' removed", name),
    );
    Ok(())
}

/// Intenta login con usuario y contraseña
pub fn login(username: &str, password: &str) -> Result<(), &'static str> {
    // Verificar rate-limiting antes de intentar
    if !security::can_attempt_login(username) {
        security::audit_log(
            security::AuditSeverity::Warning,
            &alloc::format!("Login blocked for '{}' (rate-limited)", username),
        );
        return Err("Demasiados intentos. Espere unos segundos.");
    }

    // Buscar el usuario y verificar la contraseña
    let result = {
        let users = USERS.lock();

        let mut found_user: Option<Uid> = None;
        let mut password_ok = false;

        for user in users.iter() {
            if user.name == username {
                if user.check_password(password) {
                    found_user = Some(user.uid);
                    password_ok = true;
                } else {
                    found_user = Some(user.uid);
                    password_ok = false;
                }
                break;
            }
        }

        match (found_user, password_ok) {
            (Some(uid), true) => Ok(uid),
            (Some(_), false) => Err("Contraseña incorrecta"),
            (None, _) => Err("Usuario no encontrado"),
        }
    };

    // Si el login fue exitoso, actualizar el estado
    match result {
        Ok(uid) => {
            *CURRENT_UID.lock() = uid;
            *CURRENT_NAME.lock() = String::from(username);
            *LOGGED_IN.lock() = true;
            security::record_successful_login(username);
            security::audit_log(
                security::AuditSeverity::Info,
                &alloc::format!("User '{}' logged in successfully", username),
            );
            Ok(())
        }
        Err(e) => {
            security::record_failed_attempt(username);
            Err(e)
        }
    }
}

/// Cierra la sesión actual
pub fn logout() {
    let username = CURRENT_NAME.lock().clone();
    *LOGGED_IN.lock() = false;
    *CURRENT_UID.lock() = 0;
    *CURRENT_NAME.lock() = String::from("root");
    security::audit_log(
        security::AuditSeverity::Info,
        &alloc::format!("User '{}' logged out", username),
    );
}

/// Verifica si hay sesión activa
pub fn is_logged_in() -> bool {
    *LOGGED_IN.lock()
}

/// Obtiene el UID actual
pub fn current_uid() -> Uid {
    *CURRENT_UID.lock()
}

/// Obtiene el GID actual
pub fn current_gid() -> Gid {
    get_user(current_uid()).map(|u| u.gid).unwrap_or(0)
}

/// Cambia el usuario actual (requiere permisos)
pub fn set_current_user_by_name(name: &str) -> Result<(), &'static str> {
    if let Some(user) = get_user_by_name(name) {
        *CURRENT_UID.lock() = user.uid;
        *CURRENT_NAME.lock() = user.name.clone();
        Ok(())
    } else {
        Err("Usuario no encontrado")
    }
}

pub fn set_current_user(uid: Uid) -> Result<(), &'static str> {
    if let Some(user) = get_user(uid) {
        *CURRENT_UID.lock() = uid;
        *CURRENT_NAME.lock() = user.name.clone();
        Ok(())
    } else {
        Err("Usuario no encontrado")
    }
}

pub fn set_uid(uid: Uid) {
    *CURRENT_UID.lock() = uid;
}

pub fn set_gid(gid: Gid) {
    *CURRENT_GID.lock() = gid;
}

/// Obtiene un usuario por UID
pub fn get_user(uid: Uid) -> Option<User> {
    USERS.lock().iter().find(|u| u.uid == uid).cloned()
}

/// Obtiene un usuario por nombre
pub fn get_user_by_name(name: &str) -> Option<User> {
    USERS.lock().iter().find(|u| u.name == name).cloned()
}

/// Lista todos los usuarios
pub fn list_users() -> Vec<User> {
    USERS.lock().clone()
}

/// Obtiene un grupo por GID
pub fn get_group(gid: Gid) -> Option<Group> {
    GROUPS.lock().iter().find(|g| g.gid == gid).cloned()
}

/// Verifica si el usuario actual es root
pub fn is_root() -> bool {
    current_uid() == 0
}

/// Nombre del usuario actual
pub fn current_username() -> String {
    CURRENT_NAME.lock().clone()
}

/// Cambia la contraseña de un usuario
pub fn change_password(username: &str, new_password: &str) -> Result<(), &'static str> {
    if new_password.len() < security::MIN_PASSWORD_LEN {
        return Err("Contraseña demasiado corta (mínimo 4 caracteres)");
    }

    let current = current_uid();
    let mut users = USERS.lock();

    for user in users.iter_mut() {
        if user.name == username {
            if current != 0 && current != user.uid {
                return Err("Permiso denegado");
            }
            let salt = security::generate_salt();
            user.password_hash = security::hash_password(new_password, &salt);
            user.password_salt = salt;

            security::audit_log(
                security::AuditSeverity::Info,
                &alloc::format!("Password changed for user '{}'", username),
            );
            return Ok(());
        }
    }

    Err("Usuario no encontrado")
}

// ══════════════════════════════════════════════════════════════════════════════
// PERMISOS
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
pub struct Permissions {
    pub owner_read: bool,
    pub owner_write: bool,
    pub owner_exec: bool,
    pub group_read: bool,
    pub group_write: bool,
    pub group_exec: bool,
    pub other_read: bool,
    pub other_write: bool,
    pub other_exec: bool,
}

impl Permissions {
    pub fn default_file() -> Self {
        Self {
            owner_read: true,
            owner_write: true,
            owner_exec: false,
            group_read: true,
            group_write: false,
            group_exec: false,
            other_read: true,
            other_write: false,
            other_exec: false,
        }
    }

    pub fn default_dir() -> Self {
        Self {
            owner_read: true,
            owner_write: true,
            owner_exec: true,
            group_read: true,
            group_write: false,
            group_exec: true,
            other_read: true,
            other_write: false,
            other_exec: true,
        }
    }

    pub fn to_string(&self) -> String {
        let mut s = String::with_capacity(9);
        s.push(if self.owner_read { 'r' } else { '-' });
        s.push(if self.owner_write { 'w' } else { '-' });
        s.push(if self.owner_exec { 'x' } else { '-' });
        s.push(if self.group_read { 'r' } else { '-' });
        s.push(if self.group_write { 'w' } else { '-' });
        s.push(if self.group_exec { 'x' } else { '-' });
        s.push(if self.other_read { 'r' } else { '-' });
        s.push(if self.other_write { 'w' } else { '-' });
        s.push(if self.other_exec { 'x' } else { '-' });
        s
    }
}
