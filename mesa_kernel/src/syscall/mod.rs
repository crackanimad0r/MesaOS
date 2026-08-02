// mesa_kernel/src/syscall/mod.rs
#![cfg(target_arch = "x86_64")]

//! Sistema de llamadas al sistema (syscalls) usando SYSCALL/SYSRET
//!
//! Mejorado: validación de punteros de usuario (SMAP/SMEP), auditoría

extern crate alloc;

use crate::security;
use core::arch::naked_asm;
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};
use x86_64::registers::rflags::RFlags;
use x86_64::VirtAddr;

/// Inicializa el mecanismo de syscalls
pub fn init() {
    crate::serial_println!("[SYSCALL] Inicializando syscalls...");
    init_cpu();
    crate::klog_info!("Syscalls initialized (SYSCALL/SYSRET)");
    crate::serial_println!("[SYSCALL] Syscalls listos");
}

/// Configura los MSRs de SYSCALL/SYSRET. Los MSRs son por-CPU, por lo que
/// debe llamarse en CADA núcleo (BSP en init(), APs en su entry point).
pub fn init_cpu() {
    unsafe {
        // Habilitar SYSCALL/SYSRET
        let efer = Efer::read();
        Efer::write(efer | EferFlags::SYSTEM_CALL_EXTENSIONS);

        let sysret_base: u16 = 0x10; // Base para user
        let syscall_base: u16 = 0x08; // Base para kernel
        Star::write_raw(sysret_base, syscall_base);

        // LSTAR: dirección del handler
        LStar::write(VirtAddr::new(syscall_entry as u64));

        // SFMASK: flags a limpiar (deshabilitar interrupts durante syscall)
        SFMask::write(RFlags::INTERRUPT_FLAG);
    }
}

/// Números de syscall
pub mod numbers {
    pub const SYS_READ: u64 = 0;
    pub const SYS_WRITE: u64 = 1;
    pub const SYS_OPEN: u64 = 2;
    pub const SYS_CLOSE: u64 = 3;
    pub const SYS_STAT: u64 = 4;
    pub const SYS_LSEEK: u64 = 8;
    pub const SYS_YIELD: u64 = 24;
    pub const SYS_SLEEP: u64 = 35;
    pub const SYS_GETPID: u64 = 39;
    pub const SYS_PIPE: u64 = 42;
    pub const SYS_EXIT: u64 = 60;
    pub const SYS_GETUID: u64 = 102;
    pub const SYS_BIOS_ANALYZE: u64 = 200;
}

/// Return path for fork/clone child.
/// Sets RAX=0 and returns to user space like a normal syscall return.
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn fork_return() {
    core::arch::naked_asm!(
        "cli",
        "xor rax, rax",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "pop r11",
        "pop rcx",
        "pop rsp",
        "sysretq",
    );
}

/// Entry point de syscall (ensamblador)
/// RCX = user RIP, R11 = user RFLAGS, RAX = syscall number
/// RDI, RSI, RDX, R10, R8, R9 = argumentos (hasta 6)
#[unsafe(naked)]
extern "C" fn syscall_entry() {
    naked_asm!(
        // 1. Guardar user RSP en el área per-CPU (gs:[offset]) y cambiar a kernel stack
        "mov gs:[{usr_rsp_off}], rsp",
        "mov rsp, gs:[{kstack_off}]",

        // 2. Push saved regs onto KERNEL stack (not user stack)
        "push rcx",
        "push r11",
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // 3. Shuffle arguments for C ABI: (num, arg1..arg5)
        "mov r15, rdi",
        "mov rdi, rax",
        "mov rax, rsi",
        "mov rsi, r15",
        "mov r15, rdx",
        "mov rdx, rax",
        "mov rcx, r15",
        "mov r15, r8",
        "mov r8, r10",
        "mov r9, r15",

        "sti",
        "call {handler}",
        "cli",

        // 4. Restore regs from kernel stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "pop r11",
        "pop rcx",

        // 5. Restore user RSP
        "mov rsp, gs:[{usr_rsp_off}]",

        "sysretq",
        kstack_off = const crate::smp::SYSCALL_KSTACK_OFFSET,
        usr_rsp_off = const crate::smp::SYSCALL_USR_RSP_OFFSET,
        handler = sym syscall_dispatcher,
    );
}

#[no_mangle]
extern "C" fn syscall_dispatcher(
    num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> i64 {
    let is_linux = crate::scheduler::with_current_task(|t| t.is_linux).unwrap_or(false);
    if is_linux {
        return crate::linux_compat::syscalls::dispatch(num, arg1, arg2, arg3, arg4, arg5);
    }
    match num {
        numbers::SYS_WRITE => sys_write(arg1 as i32, arg2, arg3),
        numbers::SYS_READ => sys_read(arg1 as i32, arg2, arg3),
        numbers::SYS_OPEN => sys_open(arg1, arg2),
        numbers::SYS_CLOSE => sys_close(arg1 as i32),
        numbers::SYS_STAT => sys_stat(arg1, arg2),
        numbers::SYS_EXIT => sys_exit(arg1 as i32),
        numbers::SYS_YIELD => sys_yield(),
        numbers::SYS_GETPID => sys_getpid(),
        numbers::SYS_GETUID => sys_getuid(),
        numbers::SYS_SLEEP => sys_sleep(arg1),
        numbers::SYS_PIPE => sys_pipe(arg1, arg2),
        numbers::SYS_BIOS_ANALYZE => sys_bios_analyze(),
        _ => {
            if !is_linux {
                security::audit_log(
                    security::AuditSeverity::Warning,
                    &alloc::format!("Unknown syscall: {}", num),
                );
            }
            -crate::linux_compat::errno::ENOSYS
        }
    }
}

/// Valida que un buffer esté en espacio de usuario y sea accesible
fn validate_user_buffer_safe(ptr: u64, count: u64) -> bool {
    if !security::validate_user_buffer(ptr, count as usize) {
        security::audit_log(
            security::AuditSeverity::Warning,
            &alloc::format!(
                "Syscall: invalid user buffer ptr={:#x} count={}",
                ptr,
                count
            ),
        );
        return false;
    }
    true
}

fn sys_write(fd: i32, buf: u64, count: u64) -> i64 {
    if buf == 0 || count == 0 || count > 4096 {
        return 0;
    }

    // SMAP check: validar que el buffer está en espacio de usuario
    if !validate_user_buffer_safe(buf, count) {
        return -1;
    }

    let slice = unsafe { core::slice::from_raw_parts(buf as *const u8, count as usize) };

    // Pipes
    if crate::pipe::is_pipe_fd(fd) {
        match crate::pipe::pipe_write(fd, slice) {
            Ok(n) => return n as i64,
            Err(e) => return e as i64,
        }
    }

    // Consola (stdout/stderr)
    if fd == 1 || fd == 2 {
        for &byte in slice {
            let c = byte as char;
            crate::serial_print!("{}", c);
            crate::drivers::framebuffer::console::_print(format_args!("{}", c));
        }
        return count as i64;
    }

    // Archivos
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        if let Some(handle) = table.get_mut(&fd) {
            if handle.node_type == crate::fs::NodeType::File {
                if crate::fs::write(&handle.path, slice).is_ok() {
                    handle.pos += slice.len();
                    return slice.len() as i64;
                }
            }
        }
        -1
    })
    .unwrap_or(-1)
}

fn sys_read(fd: i32, buf: u64, count: u64) -> i64 {
    if buf == 0 || count == 0 || count > 4096 {
        return 0;
    }

    // SMAP check: validar que el buffer está en espacio de usuario
    if !validate_user_buffer_safe(buf, count) {
        return -1;
    }

    // Pipes
    if crate::pipe::is_pipe_fd(fd) {
        let mut slice = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, count as usize) };
        match crate::pipe::pipe_read(fd, &mut slice) {
            Ok(n) => return n as i64,
            Err(e) => return e as i64,
        }
    }

    // Archivos
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        if let Some(handle) = table.get_mut(&fd) {
            if handle.node_type == crate::fs::NodeType::File {
                match crate::fs::read(&handle.path) {
                    Ok(data) => {
                        let start = handle.pos;
                        if start >= data.len() {
                            return 0;
                        }
                        let end = (start + count as usize).min(data.len());
                        let len = end - start;

                        let slice = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, len) };
                        slice.copy_from_slice(&data[start..end]);
                        handle.pos = end;
                        return len as i64;
                    }
                    Err(_) => return -1,
                }
            }
        }
        -1
    })
    .unwrap_or(-1)
}

fn sys_exit(_status: i32) -> i64 {
    security::audit_log(
        security::AuditSeverity::Info,
        &alloc::format!(
            "Process {} exited",
            crate::scheduler::current_task_id().unwrap_or(0)
        ),
    );
    crate::scheduler::exit_current();
    // exit_current() es -> ! y nunca retorna; esta línea es inalcanzable.
    #[allow(unreachable_code)]
    0
}

fn sys_yield() -> i64 {
    crate::scheduler::yield_now();
    0
}

fn sys_getpid() -> i64 {
    crate::scheduler::current_task_id().unwrap_or(0) as i64
}

fn sys_getuid() -> i64 {
    crate::users::current_uid() as i64
}

fn sys_sleep(ms: u64) -> i64 {
    let ticks = (ms / 55).max(1);
    let start = crate::curr_arch::get_ticks();
    while crate::curr_arch::get_ticks() - start < ticks {
        crate::scheduler::yield_now();
    }
    0
}

fn sys_pipe(_arg1: u64, _arg2: u64) -> i64 {
    match crate::pipe::create_pipe() {
        Ok((r, w)) => ((r as i64) & 0xFFFFFFFF) | ((w as i64) << 32),
        Err(_) => -1,
    }
}

fn sys_bios_analyze() -> i64 {
    #[cfg(target_arch = "x86_64")]
    crate::drivers::bios_analyzer::bios_analyze_cmd(&[]);
    0
}

fn sys_open(path_ptr: u64, _flags: u64) -> i64 {
    let path = match read_user_string_safe(path_ptr) {
        Some(s) => s,
        None => return -1,
    };

    // Auditoría de apertura de archivos
    security::audit_log(
        security::AuditSeverity::Info,
        &alloc::format!(
            "open(\"{}\") by PID {}",
            path,
            crate::scheduler::current_task_id().unwrap_or(0)
        ),
    );

    match crate::fs::stat(&path) {
        Ok(meta) => crate::scheduler::with_current_task(|task| {
            let mut table = task.fd_table.lock();
            let next_fd = table.keys().max().map(|k| k + 1).unwrap_or(3).max(3);
            table.insert(
                next_fd,
                crate::fs::FileHandle {
                    path: path.clone(),
                    pos: 0,
                    node_type: meta.node_type,
                },
            );
            next_fd as i64
        })
        .unwrap_or(-1),
        Err(_) => -2, // ENOENT
    }
}

fn sys_close(fd: i32) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        if table.remove(&fd).is_some() {
            0
        } else {
            -1
        }
    })
    .unwrap_or(-1)
}

fn sys_stat(path_ptr: u64, stat_ptr: u64) -> i64 {
    let path = match read_user_string_safe(path_ptr) {
        Some(s) => s,
        None => return -1,
    };

    match crate::fs::stat(&path) {
        Ok(meta) => {
            if stat_ptr != 0 {
                // Validar que stat_ptr está en espacio de usuario
                if !security::validate_user_ptr(stat_ptr) {
                    security::audit_log(
                        security::AuditSeverity::Warning,
                        "sys_stat: invalid stat buffer pointer",
                    );
                    return -1;
                }
                let user_meta = unsafe { &mut *(stat_ptr as *mut crate::fs::Metadata) };
                *user_meta = meta;
            }
            0
        }
        Err(_) => -1,
    }
}

/// Lee un string de espacio de usuario con validación SMAP
fn read_user_string_safe(ptr: u64) -> Option<alloc::string::String> {
    // Validación SMAP: el puntero debe estar en espacio de usuario
    if !security::validate_user_ptr(ptr) {
        security::audit_log(
            security::AuditSeverity::Warning,
            &alloc::format!("read_user_string: invalid ptr {:#x}", ptr),
        );
        return None;
    }

    let mut s = alloc::string::String::new();
    let mut curr = ptr;

    // Usamos el AddressSpace actual para validar mapeos
    let current_as = crate::memory::AddressSpace::kernel();

    loop {
        if s.len() >= 256 {
            // Límite reducido para seguridad
            security::audit_log(
                security::AuditSeverity::Warning,
                "read_user_string: string too long (>256 chars)",
            );
            return None;
        }

        // Verificar que el byte actual esté mapeado
        if current_as.translate(curr).is_none() {
            return None;
        }

        let val = unsafe { *(curr as *const u8) };
        if val == 0 {
            break;
        }

        // Solo permitir caracteres ASCII imprimibles y algunos whitespace
        if val < 0x20 || val > 0x7E {
            if val != b'\n' && val != b'\r' && val != b'\t' {
                return None; // Rechazar caracteres de control no whitespace
            }
        }

        s.push(val as char);
        curr += 1;
    }
    Some(s)
}
