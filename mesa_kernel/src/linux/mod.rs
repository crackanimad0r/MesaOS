// src/linux/mod.rs
pub mod slab;
pub mod list;
pub mod spinlock;

pub use slab::*;
pub use list::*;
pub use spinlock::*;

pub fn msleep(ms: u64) {
    // Basic busy wait for now, or call a scheduler sleep if available
    for _ in 0..ms * 1_000_000 {
        core::hint::spin_loop();
    }
}

#[allow(non_camel_case_types)]
pub type size_t = usize;

pub type u8 = core::primitive::u8;
pub type u16 = core::primitive::u16;
pub type u32 = core::primitive::u32;
pub type u64 = core::primitive::u64;

#[macro_export]
macro_rules! dev_info {
    ($($arg:tt)*) => {
        $crate::mesa_println!("[LCS] info: {}", format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! dev_err {
    ($($arg:tt)*) => {
        $crate::mesa_println!("[LCS] error: {}", format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => {
        $crate::mesa_println!("[LCS] {}", format_args!($($arg)*));
    };
}



