// src/linux/spinlock.rs


#[allow(non_camel_case_types)]
pub struct spinlock_t {
    // In a real shim, this would be a raw spinlock.
    // For now, we'll use a simple indicator since we are mostly single-core or 
    // using the big kernel lock pattern.
    locked: bool,
}

impl spinlock_t {
    pub const fn new() -> Self {
        Self { locked: false }
    }
}

pub unsafe fn spin_lock_irqsave(_lock: &mut spinlock_t, flags: &mut u64) {
    *flags = if x86_64::instructions::interrupts::are_enabled() { 1 } else { 0 };
    x86_64::instructions::interrupts::disable();
    _lock.locked = true;
}

pub unsafe fn spin_unlock_irqrestore(_lock: &mut spinlock_t, flags: u64) {
    _lock.locked = false;
    if flags != 0 {
        x86_64::instructions::interrupts::enable();
    }
}

