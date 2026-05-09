//! Context switch en ensamblador para x86_64 con soporte CR3

use core::arch::naked_asm;

/// Contexto de CPU que se guarda/restaura en context switch
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub cr3: u64,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: 0,
            rbp: 0,
            rsp: 0,
            cr3: 0,
        }
    }
    
    /// Crea un contexto con el CR3 actual
    pub fn with_current_cr3() -> Self {
        let cr3: u64;
        unsafe {
            core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
        }
        Self {
            cr3,
            ..Self::new()
        }
    }

    pub fn set_sp(&mut self, sp: u64) { self.rsp = sp; }
    pub fn set_entry(&mut self, entry: u64) { self.rbx = entry; }
    pub fn set_page_table(&mut self, cr3: u64) { self.cr3 = cr3; }
}

/// Cambia de una tarea a otra (con cambio de CR3)
#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(old: *mut Context, new: *const Context) {
    naked_asm!(
        // Guardar callee-saved en stack
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        
        // Guardar RSP actual en old->rsp (offset 48 = 6*8)
        "mov [rdi + 48], rsp",
        
        // Guardar CR3 actual en old->cr3 (offset 56 = 7*8)
        "mov rax, cr3",
        "mov [rdi + 56], rax",
        
        // Cargar nuevo CR3 si es diferente (offset 56)
        "mov rax, [rsi + 56]",
        "test rax, rax",
        "jz 2f",
        "mov rcx, cr3",
        "cmp rax, rcx",
        "je 2f",
        "mov cr3, rax",
        "2:",
        
        // Cargar nuevo RSP (offset 48)
        "mov rsp, [rsi + 48]",
        
        // Restaurar callee-saved desde nuevo stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        
        // Retornar a la nueva tarea
        "ret",
    );
}

/// Cambia solo el CR3 (sin cambiar el stack)
#[inline]
pub unsafe fn switch_cr3(new_cr3: u64) {
    if new_cr3 != 0 {
        let current: u64;
        core::arch::asm!("mov {}, cr3", out(reg) current, options(nomem, nostack));
        if current != new_cr3 {
            core::arch::asm!("mov cr3, {}", in(reg) new_cr3, options(nostack));
        }
    }
}