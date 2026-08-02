use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub type timer_func_t = unsafe extern "C" fn(*mut timer_list);

#[repr(C)]
pub struct timer_list {
    pub expires: u64,
    pub function: Option<timer_func_t>,
    pub data: u64,
    pub fired: AtomicBool,
    pub _slack: i32,
}

impl timer_list {
    pub const fn new() -> Self {
        Self {
            expires: 0,
            function: None,
            data: 0,
            fired: AtomicBool::new(false),
            _slack: -1,
        }
    }
}

pub unsafe fn init_timer(timer: *mut timer_list) {
    (*timer).function = None;
    (*timer).expires = 0;
    (*timer).fired.store(false, Ordering::Release);
}

pub unsafe fn setup_timer(timer: *mut timer_list, func: timer_func_t, data: u64) {
    (*timer).function = Some(func);
    (*timer).data = data;
    (*timer).fired.store(false, Ordering::Release);
}

pub unsafe fn mod_timer(timer: *mut timer_list, expires: u64) -> i32 {
    (*timer).expires = expires;
    (*timer).fired.store(false, Ordering::Release);
    0
}

pub unsafe fn del_timer(timer: *mut timer_list) -> i32 {
    (*timer).fired.store(true, Ordering::Release);
    0
}

pub unsafe fn timer_pending(timer: *const timer_list) -> bool {
    !(*timer).fired.load(Ordering::Acquire)
}

pub fn process_timers(timers: &[&timer_list]) {
    let ticks = crate::curr_arch::get_ticks();
    for t in timers {
        if t.expires > 0 && !t.fired.load(Ordering::Acquire) && ticks >= t.expires {
            if let Some(func) = t.function {
                unsafe {
                    func(*t as *const timer_list as *mut timer_list);
                }
            }
            t.fired.store(true, Ordering::Release);
        }
    }
}
