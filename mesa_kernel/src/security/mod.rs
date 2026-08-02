//! Módulo de seguridad mejorado para MesaOS
//!
//! Proporciona:
//! - Hashing seguro de contraseñas (SHA-256-like con salt)
//! - Rate-limiting de login (anti-fuerza bruta)
//! - Stack canary para detección de buffer overflow
//! - ASLR seed generation
//! - Validación SMAP/SMEP de punteros de usuario
//! - Audit logging de eventos de seguridad
//! - Protección contra ataques de temporización

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

// ══════════════════════════════════════════════════════════════════════════════
// CONSTANTES DE SEGURIDAD
// ══════════════════════════════════════════════════════════════════════════════

/// Máximo de intentos de login antes de bloqueo temporal
pub const MAX_LOGIN_ATTEMPTS: u32 = 5;

/// Tiempo de bloqueo tras exceder intentos (en ticks, ~55ms cada tick)
pub const LOCKOUT_DURATION_TICKS: u64 = 1800; // ~30 segundos (55ms * 1800 ≈ 99s)

/// Longitud del salt en bytes
pub const SALT_LEN: usize = 16;

/// ASLR: número de bits de entropía para la base del código
pub const ASLR_CODE_ENTROPY_BITS: u64 = 8;

/// ASLR: número de bits de entropía para la base del stack
pub const ASLR_STACK_ENTROPY_BITS: u64 = 4;

/// Tamaño mínimo de contraseña
pub const MIN_PASSWORD_LEN: usize = 4;

/// Máximo de entradas en el audit log
pub const AUDIT_LOG_MAX: usize = 256;

/// User/Kernel address space boundary (48-bit canonical)
pub const USER_ADDR_MAX: u64 = 0x0000_7FFF_FFFF_FFFF;
pub const KERNEL_ADDR_MIN: u64 = 0xFFFF_8000_0000_0000;

// ══════════════════════════════════════════════════════════════════════════════
// STACK CANARY
// ══════════════════════════════════════════════════════════════════════════════

/// Canary global para el kernel
static KERNEL_CANARY: AtomicU64 = AtomicU64::new(0);

/// Inicializa el canary del kernel con un valor aleatorio
pub fn init_canary() {
    let seed = random_u64();
    KERNEL_CANARY.store(seed, Ordering::SeqCst);
    crate::serial_println!("[SEC] Kernel stack canary initialized: {:#x}", seed);
}

/// Obtiene el canary actual del kernel
#[inline(always)]
pub fn get_kernel_canary() -> u64 {
    KERNEL_CANARY.load(Ordering::Relaxed)
}

/// Verifica el canary (llamar al final de funciones vulnerables)
#[inline(always)]
pub fn check_canary(expected: u64) -> bool {
    expected == KERNEL_CANARY.load(Ordering::Relaxed)
}

// ══════════════════════════════════════════════════════════════════════════════
// GENERADOR DE NÚMEROS ALEATORIOS (LFSR + RDTSC)
// ══════════════════════════════════════════════════════════════════════════════

/// Estado del RNG simple para el kernel
static RNG_STATE: AtomicU64 = AtomicU64::new(0xdead_beef_cafe_babe);

/// Genera un u64 pseudoaleatorio usando Xorshift
pub fn random_u64() -> u64 {
    let mut state = RNG_STATE.load(Ordering::Relaxed);

    // Xorshift64*
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;

    RNG_STATE.store(state, Ordering::Relaxed);

    // Mezclar con RDTSC si disponible
    #[cfg(target_arch = "x86_64")]
    {
        let mut tsc_lo: u32;
        let mut tsc_hi: u32;
        unsafe {
            core::arch::asm!(
                "rdtsc",
                out("eax") tsc_lo,
                out("edx") tsc_hi,
                options(nomem, nostack)
            );
        }
        state ^= (tsc_lo as u64) | ((tsc_hi as u64) << 32);
    }

    state
}

/// Genera un salt aleatorio
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    for i in 0..(SALT_LEN / 8) {
        let r = random_u64();
        salt[i * 8..(i + 1) * 8].copy_from_slice(&r.to_le_bytes());
    }
    // Rellenar bytes restantes si SALT_LEN no es múltiplo de 8
    let remainder = SALT_LEN % 8;
    if remainder > 0 {
        let r = random_u64();
        let start = SALT_LEN - remainder;
        salt[start..].copy_from_slice(&r.to_le_bytes()[..remainder]);
    }
    salt
}

// ══════════════════════════════════════════════════════════════════════════════
// HASHING DE CONTRASEÑAS (SHA-256-like + salt)
// ══════════════════════════════════════════════════════════════════════════════

/// Resultado del hash de contraseña: (hash_128b, salt)
/// Usamos un hash de 128 bits más fuerte que el anterior de 64 bits
pub type PasswordHash = u128;
pub type PasswordSalt = [u8; SALT_LEN];

/// Algoritmo de hash mejorado: mezcla SHA-2-like con salt (iteraciones múltiples)
/// No es SHA-256 real (necesitaríamos la crate), pero es mucho más fuerte que DJB2
pub fn hash_password(password: &str, salt: &PasswordSalt) -> PasswordHash {
    let mut state: u128 = 0x6a09e667f3bcc908; // Constante de inicialización (SHA-256)

    // Fase 1: Mezclar salt
    for &b in salt.iter() {
        state = state.wrapping_add(b as u128);
        state = state.wrapping_mul(0x100000001b3); // Constante tipo FNV-1a
        state ^= state >> 47;
        state ^= state << 31;
    }

    // Fase 2: Mezclar contraseña (con iteraciones)
    let password_bytes = password.as_bytes();
    for _ in 0..4 {
        // 4 iteraciones para hacer más costoso el brute-force
        for &b in password_bytes.iter() {
            state = state.wrapping_add(b as u128);
            state = state.wrapping_mul(0x9e3779b97f4a7c15); // Constante áurea
            state ^= state >> 29;
            state ^= state << 41;
            state ^= state.rotate_right(23);
        }
        // Mezclar con el salt de nuevo en cada iteración
        for &b in salt.iter() {
            state ^= b as u128;
            state = state.rotate_left(17);
            state = state.wrapping_mul(0xbf58476d1ce4e5b9);
        }
    }

    // Fase 3: Mezcla final
    state ^= state >> 33;
    state = state.wrapping_mul(0xff51afd7ed558ccd);
    state ^= state >> 33;
    state = state.wrapping_mul(0xc4ceb9fe1a85ec53);
    state ^= state >> 33;

    state
}

/// Verifica una contraseña contra un hash con salt
pub fn verify_password(password: &str, hash: PasswordHash, salt: &PasswordSalt) -> bool {
    // Comparación en tiempo constante para evitar timing attacks
    let computed = hash_password(password, salt);
    constant_time_eq(computed, hash)
}

/// Comparación en tiempo constante de dos valores u128
#[inline(always)]
fn constant_time_eq(a: u128, b: u128) -> bool {
    let diff = a ^ b;
    // XOR de todas las mitades (64 bits cada una)
    let diff_low = diff as u64;
    let diff_high = (diff >> 64) as u64;
    let combined = diff_low | diff_high;
    // Verificar si combined es cero
    combined == 0
}

// ══════════════════════════════════════════════════════════════════════════════
// RATE LIMITING DE LOGIN
// ══════════════════════════════════════════════════════════════════════════════

/// Estado de intentos de login por usuario
#[derive(Debug, Clone)]
struct LoginAttempts {
    attempts: u32,
    first_attempt_tick: u64,
    locked_until: u64,
}

/// Rastreador de intentos de login
static LOGIN_ATTEMPTS: Mutex<BTreeMap<String, LoginAttempts>> = Mutex::new(BTreeMap::new());

/// Verifica si un usuario puede intentar login (rate-limiting)
pub fn can_attempt_login(username: &str) -> bool {
    let mut attempts = LOGIN_ATTEMPTS.lock();
    let current_tick = crate::curr_arch::get_ticks();

    if let Some(record) = attempts.get_mut(username) {
        // Si está bloqueado, verificar si ya pasó el tiempo
        if record.locked_until > current_tick {
            return false;
        }

        // Resetear si pasó suficiente tiempo desde el primer intento
        if current_tick.wrapping_sub(record.first_attempt_tick) > LOCKOUT_DURATION_TICKS * 2 {
            record.attempts = 0;
            record.first_attempt_tick = current_tick;
        }

        // Verificar si excedió el límite
        if record.attempts >= MAX_LOGIN_ATTEMPTS {
            record.locked_until = current_tick + LOCKOUT_DURATION_TICKS;
            record.attempts = 0;
            crate::klog_warn!(
                "[SEC] User '{}' locked out for {} ticks due to too many failed attempts",
                username,
                LOCKOUT_DURATION_TICKS
            );
            return false;
        }

        true
    } else {
        true
    }
}

/// Registra un intento fallido de login
pub fn record_failed_attempt(username: &str) {
    let mut attempts = LOGIN_ATTEMPTS.lock();
    let current_tick = crate::curr_arch::get_ticks();

    let record = attempts
        .entry(String::from(username))
        .or_insert(LoginAttempts {
            attempts: 0,
            first_attempt_tick: current_tick,
            locked_until: 0,
        });

    if record.attempts == 0 {
        record.first_attempt_tick = current_tick;
    }

    record.attempts += 1;

    crate::klog_warn!(
        "[SEC] Failed login attempt {} for user '{}'",
        record.attempts,
        username
    );
}

/// Registra un login exitoso (resetea contador)
pub fn record_successful_login(username: &str) {
    LOGIN_ATTEMPTS.lock().remove(username);
}

// ══════════════════════════════════════════════════════════════════════════════
// VALIDACIÓN DE PUNTEROS (SMAP/SMEP emulación)
// ══════════════════════════════════════════════════════════════════════════════

/// Valida que un puntero de usuario sea seguro de dereferenciar
/// Retorna true si el puntero está en espacio de usuario y es válido
pub fn validate_user_ptr(ptr: u64) -> bool {
    // Null check
    if ptr == 0 {
        return false;
    }

    // Debe estar en espacio de usuario (no kernel)
    if ptr > USER_ADDR_MAX {
        return false;
    }

    // Debe estar alineado a palabra para ciertos tipos
    // (check específico según el tipo, aquí validación general)
    true
}

/// Valida que un buffer de usuario sea accesible de forma segura
pub fn validate_user_buffer(ptr: u64, len: usize) -> bool {
    if !validate_user_ptr(ptr) {
        return false;
    }

    // Verificar que no hay overflow en la suma
    if ptr.checked_add(len as u64).is_none() {
        return false;
    }

    // Verificar que el buffer completo está en espacio de usuario
    if ptr + len as u64 > USER_ADDR_MAX {
        return false;
    }

    true
}

/// Valida que una dirección esté en espacio de kernel (para operaciones internas)
pub fn validate_kernel_ptr(ptr: u64) -> bool {
    if ptr == 0 {
        return false;
    }

    // Debe estar en espacio de kernel HHDM o de dispositivo
    if ptr < 0xFFFF_8000_0000_0000 {
        return false;
    }

    true
}

// ══════════════════════════════════════════════════════════════════════════════
// ASLR
// ══════════════════════════════════════════════════════════════════════════════

/// Genera una base aleatoria para el código de usuario (con entropía)
pub fn randomize_code_base(base: u64, entropy_bits: u64) -> u64 {
    let mask = (1u64 << entropy_bits) - 1;
    let offset = random_u64() & mask;
    let page_base = base + offset * crate::memory::PAGE_SIZE;
    // Alinear a página
    page_base & !(crate::memory::PAGE_SIZE - 1)
}

/// Genera una base aleatoria para el stack de usuario
pub fn randomize_stack_top(top: u64, entropy_bits: u64) -> u64 {
    let mask = (1u64 << entropy_bits) - 1;
    let offset = random_u64() & mask;
    let page_offset = offset * crate::memory::PAGE_SIZE;
    top - page_offset
}

// ══════════════════════════════════════════════════════════════════════════════
// AUDIT LOG
// ══════════════════════════════════════════════════════════════════════════════

/// Nivel de severidad de un evento de auditoría
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Evento de auditoría
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub tick: u64,
    pub severity: AuditSeverity,
    pub message: String,
}

/// Log de auditoría circular
static AUDIT_LOG: Mutex<alloc::collections::VecDeque<AuditEvent>> =
    Mutex::new(alloc::collections::VecDeque::new());

/// Registra un evento de auditoría
pub fn audit_log(severity: AuditSeverity, message: &str) {
    let mut log = AUDIT_LOG.lock();
    let event = AuditEvent {
        tick: crate::curr_arch::get_ticks(),
        severity,
        message: String::from(message),
    };

    if log.len() >= AUDIT_LOG_MAX {
        log.pop_front();
    }
    log.push_back(event);

    // También loguear al klog
    match severity {
        AuditSeverity::Info => crate::klog_info!("[AUDIT] {}", message),
        AuditSeverity::Warning => crate::klog_warn!("[AUDIT] {}", message),
        AuditSeverity::Error => crate::klog_error!("[AUDIT] {}", message),
        AuditSeverity::Critical => crate::klog_error!("[AUDIT:CRITICAL] {}", message),
    }
}

/// Obtiene los últimos N eventos de auditoría
pub fn get_audit_log(n: usize) -> Vec<AuditEvent> {
    let log = AUDIT_LOG.lock();
    let skip = if log.len() > n { log.len() - n } else { 0 };
    log.iter().skip(skip).cloned().collect()
}

/// Limpia el log de auditoría (solo root)
pub fn clear_audit_log() {
    AUDIT_LOG.lock().clear();
    crate::klog_info!("[AUDIT] Audit log cleared");
}

// ══════════════════════════════════════════════════════════════════════════════
// INICIALIZACIÓN
// ══════════════════════════════════════════════════════════════════════════════

/// Inicializa el módulo de seguridad
pub fn init() {
    init_canary();

    // Sembrar RNG con RDTSC si disponible
    #[cfg(target_arch = "x86_64")]
    {
        let seed = random_u64();
        RNG_STATE.store(seed, Ordering::SeqCst);
        crate::serial_println!("[SEC] RNG seeded: {:#x}", seed);
    }

    audit_log(AuditSeverity::Info, "Security module initialized");
    crate::serial_println!("[SEC] Módulo de seguridad inicializado");
}
