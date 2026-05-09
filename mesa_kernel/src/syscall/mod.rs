// mesa_kernel/src/syscall/mod.rs
#![cfg(target_arch = "x86_64")]

//! Sistema de llamadas al sistema (syscalls) usando SYSCALL/SYSRET

extern crate alloc;

use x86_64::registers::model_specific::{Efer, EferFlags, LStar, Star, SFMask};
use x86_64::registers::rflags::RFlags;
use x86_64::VirtAddr;
use core::arch::naked_asm;

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

/// Inicializa el mecanismo de syscalls
pub fn init() {
    crate::serial_println!("[SYSCALL] Inicializando syscalls...");
    
    unsafe {
        // Habilitar SYSCALL/SYSRET
        let efer = Efer::read();
        Efer::write(efer | EferFlags::SYSTEM_CALL_EXTENSIONS);
        
        let sysret_base: u16 = 0x10;  // Base para user
        let syscall_base: u16 = 0x08; // Base para kernel
        Star::write_raw(sysret_base, syscall_base);
        
        // LSTAR: dirección del handler
        LStar::write(VirtAddr::new(syscall_entry as u64));
        
        // SFMASK: flags a limpiar (deshabilitar interrupts durante syscall)
        SFMask::write(RFlags::INTERRUPT_FLAG);
    }
    
    crate::klog_info!("Syscalls initialized (SYSCALL/SYSRET)");
    crate::serial_println!("[SYSCALL] Syscalls listos");
}

/// Entry point de syscall (ensamblador)
#[unsafe(naked)]
extern "C" fn syscall_entry() {
    naked_asm!(
        // Al entrar:
        //   RCX = return RIP
        //   R11 = return RFLAGS
        //   RAX = syscall number
        //   RDI, RSI, RDX, R10, R8, R9 = argumentos
        
        // Guardar registros del usuario
        "push rcx",          // RIP de retorno
        "push r11",          // RFLAGS
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        
        // Mover argumentos
        "mov r15, rdi",      // Guardar arg1 temporalmente
        "mov rdi, rax",      // syscall number -> rdi
        "mov rax, rsi",      // arg2 -> temp
        "mov rsi, r15",      // arg1 -> rsi
        "mov r15, rdx",      // arg3 -> temp
        "mov rdx, rax",      // arg2 -> rdx
        "mov rcx, r15",      // arg3 -> rcx
        "mov r15, r8",       // arg5 -> temp
        "mov r8, r10",       // arg4 -> r8
        "mov r9, r15",       // arg5 -> r9
        
        "sti",
        "call {handler}",
        "cli",
        
        // Restaurar registros
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "pop r11",           // RFLAGS
        "pop rcx",           // RIP
        
        "sysretq",
        handler = sym syscall_dispatcher,
    );
}

#[no_mangle]
extern "C" fn syscall_dispatcher(
    num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    _arg4: u64,
    _arg5: u64,
) -> i64 {
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
            crate::serial_println!("[SYSCALL] Unknown syscall: {}", num);
            -1
        }
    }
}

fn sys_write(fd: i32, buf: u64, count: u64) -> i64 {
    if buf == 0 || count == 0 {
        return 0;
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
                // Por ahora solo soportamos sobreescritura total o append simple si implementamos offset
                // Para este demo, simplemente escribimos el buffer al archivo (ojo: esto sobrescribe todo el archivo en el VFS actual)
                if crate::fs::write(&handle.path, slice).is_ok() {
                    handle.pos += slice.len();
                    return slice.len() as i64;
                }
            }
        }
        -1
    }).unwrap_or(-1)
}

fn sys_read(fd: i32, buf: u64, count: u64) -> i64 {
    if buf == 0 || count == 0 { return 0; }
    
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
                        if start >= data.len() { return 0; }
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
    }).unwrap_or(-1)
}

fn sys_exit(status: i32) -> i64 {
    crate::scheduler::exit_current();
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
    let path = match read_user_string(path_ptr) {
        Some(s) => s,
        None => return -1,
    };
    
    match crate::fs::stat(&path) {
        Ok(meta) => {
            crate::scheduler::with_current_task(|task| {
                let mut table = task.fd_table.lock();
                let next_fd = table.keys().max().map(|k| k + 1).unwrap_or(3).max(3);
                table.insert(next_fd, crate::fs::FileHandle {
                    path: path.clone(),
                    pos: 0,
                    node_type: meta.node_type,
                });
                next_fd as i64
            }).unwrap_or(-1)
        }
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
    }).unwrap_or(-1)
}

fn sys_stat(path_ptr: u64, stat_ptr: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Some(s) => s,
        None => return -1,
    };
    
    match crate::fs::stat(&path) {
        Ok(meta) => {
            if stat_ptr != 0 {
                let user_meta = unsafe { &mut *(stat_ptr as *mut crate::fs::Metadata) };
                *user_meta = meta;
            }
            0
        }
        Err(_) => -1,
    }
}

fn read_user_string(ptr: u64) -> Option<alloc::string::String> {
    if ptr == 0 || ptr >= 0x0000_8000_0000_0000 { return None; }
    
    let mut s = alloc::string::String::new();
    let mut curr = ptr;
    
    // Usamos el AddressSpace actual para validar mapeos
    let current_as = crate::memory::AddressSpace::kernel();
    
    loop {
        if s.len() >= 1024 { return None; }
        
        // Verificar que el byte actual esté mapeado
        if current_as.translate(curr).is_none() {
            return None;
        }
        
        let val = unsafe { *(curr as *const u8) };
        if val == 0 { break; }
        s.push(val as char);
        curr += 1;
    }
    Some(s)
}
