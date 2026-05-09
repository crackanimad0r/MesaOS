//! Context switch for AArch64 (ARMv8-A)

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    // x19-x29 are callee-saved in AArch64
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64, // Frame pointer
    pub lr: u64,  // Link register (x30)
    pub sp: u64,
    pub ttbr: u64, // Translation Table Base Register (similar to CR3)
}

impl Context {
    pub const fn new() -> Self {
        Self {
            x19: 0, x20: 0, x21: 0, x22: 0, x23: 0, x24: 0, x25: 0, x26: 0, x27: 0, x28: 0, x29: 0,
            lr: 0, sp: 0, ttbr: 0,
        }
    }
    
    pub fn with_current_cr3() -> Self {
        // ttbr0_el1 on ARM
        let ttbr: u64 = 0;
        // unsafe { core::arch::asm!("mrs {}, ttbr0_el1", out(reg) ttbr); }
        Self {
            ttbr,
            ..Self::new()
        }
    }

    pub fn set_sp(&mut self, sp: u64) { self.sp = sp; }
    pub fn set_entry(&mut self, entry: u64) { self.lr = entry; }
    pub fn set_page_table(&mut self, ttbr: u64) { self.ttbr = ttbr; }
}

pub unsafe fn switch_context(old: *mut Context, new: *const Context) {
    // Assembly stub for ARM context switch
}
