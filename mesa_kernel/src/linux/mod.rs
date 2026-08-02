pub mod completion;
pub mod dma;
pub mod io;
pub mod list;
pub mod mutex;
pub mod slab;
pub mod spinlock;
pub mod timer;
pub mod wait;
pub mod workqueue;

pub use completion::*;
pub use dma::*;
pub use io::*;
pub use list::*;
pub use mutex::*;
pub use slab::*;
pub use spinlock::*;
pub use timer::*;
pub use wait::*;
pub use workqueue::*;

pub fn msleep(ms: u64) {
    let ticks = crate::curr_arch::get_ticks();
    let wait_ticks = (ms / 55).max(1);
    while crate::curr_arch::get_ticks() - ticks < wait_ticks {
        crate::scheduler::yield_now();
    }
}

pub fn mdelay(ms: u64) {
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
pub type pid_t = i32;
pub type bool_t = u32;

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
macro_rules! dev_warn {
    ($($arg:tt)*) => {
        $crate::mesa_println!("[LCS] warn: {}", format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! dev_dbg {
    ($($arg:tt)*) => {
        $crate::mesa_println!("[LCS] dbg: {}", format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => {
        $crate::mesa_println!("[LCS] {}", format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! pr_info {
    ($($arg:tt)*) => {
        $crate::mesa_println!("[LCS] {}", format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! pr_err {
    ($($arg:tt)*) => {
        $crate::mesa_println!("[LCS] ERROR: {}", format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! pr_warn {
    ($($arg:tt)*) => {
        $crate::mesa_println!("[LCS] WARN: {}", format_args!($($arg)*));
    };
}
