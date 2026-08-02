pub mod context;
pub mod gdt;
pub mod interrupts;
pub mod limine_req;
pub mod uefi_nvram;
pub mod uefi_nvram_cmd;

pub fn get_ticks() -> u64 {
    interrupts::timer::get_ticks()
}

pub fn init() {
    gdt::init();
    interrupts::init_idt();
}

pub fn halt() {
    x86_64::instructions::hlt();
}

pub fn enable_interrupts() {
    unsafe {
        x86_64::instructions::interrupts::enable();
    }
}

pub fn disable_interrupts() {
    x86_64::instructions::interrupts::disable();
}

pub fn are_interrupts_enabled() -> bool {
    x86_64::instructions::interrupts::are_enabled()
}

pub unsafe fn init_task_stack(stack_top: u64, entry: u64) -> u64 {
    let mut sp = stack_top;

    // Setup inicial del stack para context switch (x86_64 naked_asm style)
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = task_bootstrap as u64;
    }
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = 0;
    } // rbp
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = entry;
    } // rbx = entry
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = 0;
    } // r12
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = 0;
    } // r13
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = 0;
    } // r14
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = 0;
    } // r15

    sp
}

pub unsafe fn init_user_stack(stack_top: u64, entry: u64, user_stack: u64) -> u64 {
    let mut sp = stack_top;
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = user_task_bootstrap as u64;
    }
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = 0;
    } // rbp
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = entry;
    } // rbx = user entry
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = user_stack;
    } // r12 = user stack
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = 0;
    } // r13
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = 0;
    } // r14
    sp -= 8;
    unsafe {
        *(sp as *mut u64) = 0;
    } // r15
    sp
}

/// Bootstrap para tareas de kernel
extern "C" fn task_bootstrap() {
    // Clear scheduler re-entry guard (set during context switch)
    crate::scheduler::clear_in_schedule();

    // El entry point NO se pasa por registro rbx (el compilador puede
    // corromper los registros callee-saved en el prólogo). Se lee del
    // campo context.rbx de la tarea actual.
    let entry_addr = crate::scheduler::with_current_task(|t| t.context.rbx).unwrap_or(0);
    let entry: fn() = unsafe { core::mem::transmute(entry_addr) };

    unsafe {
        x86_64::instructions::interrupts::enable();
    }
    entry();
    crate::scheduler::exit_current();
}

/// Bootstrap para tareas de usuario (salta a Ring 3)
extern "C" fn user_task_bootstrap() {
    // Clear scheduler re-entry guard (set during context switch)
    crate::scheduler::clear_in_schedule();

    // El entry y el stack de usuario NO se pasan por registros (rbx/r12),
    // porque el compilador puede corromperlos en el prólogo. Se leen de los
    // campos user_entry / user_stack de la tarea actual.
    let (user_entry, user_stack) =
        crate::scheduler::with_current_task(|t| (t.user_entry, t.user_stack)).unwrap_or((0, 0));

    // Actualizar TSS RSP0 para que las interrupciones vuelvan aquí
    if let Some(stack_top) = crate::scheduler::current_kernel_stack_top() {
        crate::curr_arch::gdt::set_kernel_stack(stack_top);
    }

    // Saltar a Ring 3
    unsafe {
        jump_to_user(user_entry, user_stack);
    }

    crate::scheduler::exit_current();
}

pub unsafe fn jump_to_user(entry: u64, stack: u64) {
    let user_ds: u64 = gdt::user_data_selector().0 as u64;
    let user_cs: u64 = gdt::user_code_selector().0 as u64;

    core::arch::asm!(
        "mov ds, {user_ds:x}",
        "mov es, {user_ds:x}",
        "mov fs, {user_ds:x}",
        // NOTA: NO se toca GS. Su base (IA32_GS_BASE) apunta al área per-CPU
        // (ver smp::setup_cpu) y el entry de syscall la usa con gs:[offset]
        // para guardar/recuperar el RSP de usuario y el stack de kernel.
        "push {user_ss}",      // SS
        "push {user_rsp}",     // RSP
        "push 0x202",          // RFLAGS (IF=1)
        "push {user_cs}",      // CS
        "push {entry}",        // RIP
        "iretq",
        user_ds = in(reg) user_ds,
        user_ss = in(reg) user_ds,
        user_cs = in(reg) user_cs,
        user_rsp = in(reg) stack,
        entry = in(reg) entry,
        options(noreturn)
    );
}
