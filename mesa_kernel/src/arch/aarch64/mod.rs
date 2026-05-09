// Raspberry Pi Zero 2W (BCM2710 / Cortex-A53) initialization stub
pub mod context;
pub mod limine_req;

pub fn get_ticks() -> u64 {
    0 // TODO: ARM generic timer
}

pub fn init() {
    // UART init
    // MMU init
    // Exception Levels setup
}

pub fn halt() {
    // wfi on ARM
    unsafe { core::arch::asm!("wfi"); }
}

pub fn enable_interrupts() {
    // msr daifclr, #2 on ARM
}

pub fn disable_interrupts() {
    // msr daifset, #2 on ARM
}

pub fn are_interrupts_enabled() -> bool {
    false // TODO
}

pub unsafe fn init_task_stack(stack_top: u64, _entry: u64) -> u64 {
    stack_top // TODO
}

pub unsafe fn init_user_stack(stack_top: u64, _entry: u64, _user_stack: u64) -> u64 {
    stack_top // TODO
}

pub unsafe fn jump_to_user(_entry: u64, _stack: u64) {
    // eret on ARM
}
