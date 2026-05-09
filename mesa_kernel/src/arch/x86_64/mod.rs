pub mod gdt;
pub mod interrupts;
pub mod limine_req;
pub mod context;

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
    unsafe { x86_64::instructions::interrupts::enable(); }
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
    unsafe { *(sp as *mut u64) = task_bootstrap as u64; }
    sp -= 8;
    unsafe { *(sp as *mut u64) = 0; } // rbp
    sp -= 8;
    unsafe { *(sp as *mut u64) = entry; } // rbx = entry
    sp -= 8;
    unsafe { *(sp as *mut u64) = 0; } // r12
    sp -= 8;
    unsafe { *(sp as *mut u64) = 0; } // r13
    sp -= 8;
    unsafe { *(sp as *mut u64) = 0; } // r14
    sp -= 8;
    unsafe { *(sp as *mut u64) = 0; } // r15
    
    sp
}

pub unsafe fn init_user_stack(stack_top: u64, entry: u64, user_stack: u64) -> u64 {
    let mut sp = stack_top;
    sp -= 8;
    unsafe { *(sp as *mut u64) = user_task_bootstrap as u64; }
    sp -= 8;
    unsafe { *(sp as *mut u64) = 0; } // rbp
    sp -= 8;
    unsafe { *(sp as *mut u64) = entry; } // rbx = user entry
    sp -= 8;
    unsafe { *(sp as *mut u64) = user_stack; } // r12 = user stack
    sp -= 8;
    unsafe { *(sp as *mut u64) = 0; } // r13
    sp -= 8;
    unsafe { *(sp as *mut u64) = 0; } // r14
    sp -= 8;
    unsafe { *(sp as *mut u64) = 0; } // r15
    sp
}

/// Bootstrap para tareas de kernel
extern "C" fn task_bootstrap() {
    let entry: fn();
    unsafe {
        core::arch::asm!(
            "mov {}, rbx",
            out(reg) entry,
            options(nomem, nostack)
        );
    }
    
    unsafe { x86_64::instructions::interrupts::enable(); }
    entry();
    crate::scheduler::exit_current();
}

/// Bootstrap para tareas de usuario (salta a Ring 3)
extern "C" fn user_task_bootstrap() {
    let user_entry: u64;
    let user_stack: u64;
    
    unsafe {
        core::arch::asm!(
            "mov {}, rbx",
            "mov {}, r12",
            out(reg) user_entry,
            out(reg) user_stack,
            options(nomem, nostack)
        );
    }
    
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
    
    crate::serial_println!("[JUMP] Ring 3: RIP={:#x}, RSP={:#x}, CS={:#x}, DS={:#x}",
        entry, stack, user_cs, user_ds);
    
    core::arch::asm!(
        "mov ax, {user_ds:x}",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
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
