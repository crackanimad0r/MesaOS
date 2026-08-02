use core::sync::atomic::{AtomicBool, Ordering};

pub type work_func_t = unsafe extern "C" fn(*mut work_struct);

#[repr(C)]
pub struct work_struct {
    pub func: Option<work_func_t>,
    pub pending: AtomicBool,
}

impl work_struct {
    pub const fn new() -> Self {
        Self {
            func: None,
            pending: AtomicBool::new(false),
        }
    }
}

pub unsafe fn INIT_WORK(work: *mut work_struct, func: work_func_t) {
    (*work).func = Some(func);
    (*work).pending.store(false, Ordering::Release);
}

pub unsafe fn schedule_work(work: *mut work_struct) -> bool {
    if (*work).pending.swap(true, Ordering::Acquire) {
        return false;
    }
    if let Some(func) = (*work).func {
        func(work);
    }
    (*work).pending.store(false, Ordering::Release);
    true
}

pub unsafe fn flush_work(work: *mut work_struct) {
    while (*work).pending.load(Ordering::Acquire) {
        crate::scheduler::yield_now();
    }
}
