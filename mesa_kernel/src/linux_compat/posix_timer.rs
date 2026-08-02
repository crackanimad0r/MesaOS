use alloc::vec::Vec;
use spin::Mutex;

const MAX_TIMERS: usize = 64;

struct PosixTimer {
    clockid: i32,
    value_ns: u64,
    interval_ns: u64,
    expiry_tick: u64,
    expired: bool,
    sev_sigev_signo: i32,
    in_use: bool,
}

static POSIX_TIMERS: Mutex<Vec<PosixTimer>> = Mutex::new(Vec::new());

fn ns_to_ticks(ns: u64) -> u64 {
    if ns == 0 {
        return 0;
    }
    (ns / 55_000_000).max(1)
}

fn ticks_to_ns(ticks: u64) -> u64 {
    ticks * 55_000_000
}

fn current_tick() -> u64 {
    crate::curr_arch::get_ticks()
}

pub fn posix_timer_create(clockid: i32, sevp: u64, timerid_ptr: u64) -> i64 {
    if timerid_ptr == 0 || !crate::security::validate_user_ptr(timerid_ptr) {
        return -crate::linux_compat::errno::EFAULT;
    }
    let sigev_signo = if sevp != 0 && crate::security::validate_user_ptr(sevp) {
        unsafe { *(sevp as *const i32) } // sigev_notify at offset 0 (simplified)
    } else {
        0 // SIGEV_SIGNAL with SIGALRM (signal 14)
    };
    let mut timers = POSIX_TIMERS.lock();
    let idx = timers
        .iter()
        .position(|t| !t.in_use)
        .unwrap_or(timers.len());
    if idx >= MAX_TIMERS || idx >= timers.len() && timers.len() >= MAX_TIMERS {
        return -crate::linux_compat::errno::EAGAIN;
    }
    if idx == timers.len() {
        timers.push(PosixTimer {
            clockid,
            value_ns: 0,
            interval_ns: 0,
            expiry_tick: 0,
            expired: false,
            sev_sigev_signo: sigev_signo,
            in_use: true,
        });
    } else {
        timers[idx] = PosixTimer {
            clockid,
            value_ns: 0,
            interval_ns: 0,
            expiry_tick: 0,
            expired: false,
            sev_sigev_signo: sigev_signo,
            in_use: true,
        };
    }
    let timer_id = (idx + 1) as i32; // 1-based IDs, 0 is invalid
    unsafe {
        *(timerid_ptr as *mut i32) = timer_id;
    }
    0
}

fn find_timer(timers: &mut Vec<PosixTimer>, id: i32) -> Option<&mut PosixTimer> {
    let idx = (id - 1) as usize;
    if idx >= timers.len() {
        return None;
    }
    if !timers[idx].in_use {
        return None;
    }
    Some(&mut timers[idx])
}

#[repr(C)]
struct itimerspec {
    it_interval_sec: u64,
    it_interval_nsec: u64,
    it_value_sec: u64,
    it_value_nsec: u64,
}

pub fn posix_timer_settime(
    timer_id: i32,
    _flags: i32,
    new_value_ptr: u64,
    old_value_ptr: u64,
) -> i64 {
    if new_value_ptr == 0 || !crate::security::validate_user_ptr(new_value_ptr) {
        return -crate::linux_compat::errno::EFAULT;
    }
    let new_val: itimerspec =
        unsafe { core::ptr::read_volatile(new_value_ptr as *const itimerspec) };
    let mut timers = POSIX_TIMERS.lock();
    let tmr = match find_timer(&mut timers, timer_id) {
        Some(t) => t,
        None => return -crate::linux_compat::errno::EINVAL,
    };
    if old_value_ptr != 0 && crate::security::validate_user_ptr(old_value_ptr) {
        let old = itimerspec {
            it_interval_sec: tmr.interval_ns / 1_000_000_000,
            it_interval_nsec: tmr.interval_ns % 1_000_000_000,
            it_value_sec: tmr.value_ns / 1_000_000_000,
            it_value_nsec: tmr.value_ns % 1_000_000_000,
        };
        unsafe {
            core::ptr::write_volatile(old_value_ptr as *mut itimerspec, old);
        }
    }
    let value_ns = new_val
        .it_value_sec
        .saturating_mul(1_000_000_000)
        .saturating_add(new_val.it_value_nsec);
    let interval_ns = new_val
        .it_interval_sec
        .saturating_mul(1_000_000_000)
        .saturating_add(new_val.it_interval_nsec);
    if value_ns == 0 {
        tmr.value_ns = 0;
        tmr.interval_ns = 0;
        tmr.expiry_tick = 0;
        tmr.expired = false;
    } else {
        tmr.value_ns = value_ns;
        tmr.interval_ns = interval_ns;
        tmr.expiry_tick = current_tick() + ns_to_ticks(value_ns);
        tmr.expired = false;
    }
    0
}

pub fn posix_timer_gettime(timer_id: i32, curr_value_ptr: u64) -> i64 {
    if curr_value_ptr == 0 || !crate::security::validate_user_ptr(curr_value_ptr) {
        return -crate::linux_compat::errno::EFAULT;
    }
    let timers = POSIX_TIMERS.lock();
    let idx = (timer_id - 1) as usize;
    if idx >= timers.len() || !timers[idx].in_use {
        return -crate::linux_compat::errno::EINVAL;
    }
    let tmr = &timers[idx];
    let remains = if tmr.expired || tmr.value_ns == 0 {
        0
    } else if tmr.expiry_tick > current_tick() {
        ticks_to_ns(tmr.expiry_tick - current_tick())
    } else {
        0
    };
    let curr = itimerspec {
        it_interval_sec: tmr.interval_ns / 1_000_000_000,
        it_interval_nsec: tmr.interval_ns % 1_000_000_000,
        it_value_sec: remains / 1_000_000_000,
        it_value_nsec: remains % 1_000_000_000,
    };
    unsafe {
        core::ptr::write_volatile(curr_value_ptr as *mut itimerspec, curr);
    }
    0
}

pub fn posix_timer_delete(timer_id: i32) -> i64 {
    let mut timers = POSIX_TIMERS.lock();
    let idx = (timer_id - 1) as usize;
    if idx >= timers.len() || !timers[idx].in_use {
        return -crate::linux_compat::errno::EINVAL;
    }
    timers[idx].in_use = false;
    0
}
