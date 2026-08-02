use alloc::vec::Vec;
use spin::Mutex;

pub const TF_BASE: i32 = 32000;
pub const TF_MAX: i32 = 33000;
const MAX_TIMERFDS: usize = 64;
const TFD_NONBLOCK: u32 = 0x800;

struct Timerfd {
    interval_ns: u64,
    value_ns: u64,
    expiry_tick: u64,
    expired: bool,
    clockid: i32,
    flags: u32,
}

pub static TIMERFD_TABLE: Mutex<Vec<Option<Timerfd>>> = Mutex::new(Vec::new());

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

pub fn is_timerfd_fd(fd: i32) -> bool {
    fd >= TF_BASE && fd < TF_MAX
}

pub fn timerfd_create(clockid: i32, flags: u32) -> i64 {
    let mut table = TIMERFD_TABLE.lock();
    let idx = table.len();
    if idx >= MAX_TIMERFDS {
        return -crate::linux_compat::errno::EMFILE;
    }
    table.push(Some(Timerfd {
        interval_ns: 0,
        value_ns: 0,
        expiry_tick: 0,
        expired: false,
        clockid,
        flags,
    }));
    (TF_BASE + idx as i32) as i64
}

#[repr(C)]
struct itimerspec {
    it_interval_sec: u64,
    it_interval_nsec: u64,
    it_value_sec: u64,
    it_value_nsec: u64,
}

pub fn timerfd_settime(fd: i32, _flags: u32, new_val_ptr: u64, old_val_ptr: u64) -> i64 {
    if new_val_ptr == 0 || !crate::security::validate_user_ptr(new_val_ptr) {
        return -crate::linux_compat::errno::EFAULT;
    }
    let new_val: itimerspec = unsafe { core::ptr::read_volatile(new_val_ptr as *const itimerspec) };
    let mut table = TIMERFD_TABLE.lock();
    let idx = (fd - TF_BASE) as usize;
    if idx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    match &mut table[idx] {
        Some(tf) => {
            if old_val_ptr != 0 && crate::security::validate_user_ptr(old_val_ptr) {
                let old = itimerspec {
                    it_interval_sec: tf.interval_ns / 1_000_000_000,
                    it_interval_nsec: tf.interval_ns % 1_000_000_000,
                    it_value_sec: tf.value_ns / 1_000_000_000,
                    it_value_nsec: tf.value_ns % 1_000_000_000,
                };
                unsafe {
                    core::ptr::write_volatile(old_val_ptr as *mut itimerspec, old);
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
                tf.value_ns = 0;
                tf.interval_ns = 0;
                tf.expiry_tick = 0;
                tf.expired = false;
            } else {
                tf.value_ns = value_ns;
                tf.interval_ns = interval_ns;
                tf.expiry_tick = current_tick() + ns_to_ticks(value_ns);
                tf.expired = false;
            }
            0
        }
        None => -crate::linux_compat::errno::EBADF,
    }
}

pub fn timerfd_gettime(fd: i32, curr_val_ptr: u64) -> i64 {
    if curr_val_ptr == 0 || !crate::security::validate_user_ptr(curr_val_ptr) {
        return -crate::linux_compat::errno::EFAULT;
    }
    let table = TIMERFD_TABLE.lock();
    let idx = (fd - TF_BASE) as usize;
    if idx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    match &table[idx] {
        Some(tf) => {
            let remains = if tf.expired {
                0
            } else if tf.expiry_tick > current_tick() {
                ticks_to_ns(tf.expiry_tick - current_tick())
            } else {
                0
            };
            let curr = itimerspec {
                it_interval_sec: tf.interval_ns / 1_000_000_000,
                it_interval_nsec: tf.interval_ns % 1_000_000_000,
                it_value_sec: remains / 1_000_000_000,
                it_value_nsec: remains % 1_000_000_000,
            };
            unsafe {
                core::ptr::write_volatile(curr_val_ptr as *mut itimerspec, curr);
            }
            0
        }
        None => -crate::linux_compat::errno::EBADF,
    }
}

pub fn timerfd_read(fd: i32, buf: &mut [u8]) -> i64 {
    if buf.len() < 8 {
        return -crate::linux_compat::errno::EINVAL;
    }
    let mut table = TIMERFD_TABLE.lock();
    let idx = (fd - TF_BASE) as usize;
    if idx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    match &mut table[idx] {
        Some(tf) => {
            if current_tick() >= tf.expiry_tick && !tf.expired {
                tf.expired = true;
                if tf.interval_ns > 0 {
                    let next_tick = current_tick() + ns_to_ticks(tf.interval_ns);
                    tf.expiry_tick = next_tick;
                    tf.expired = false;
                }
                let expirations = 1u64;
                buf[..8].copy_from_slice(&expirations.to_ne_bytes());
                return 8;
            }
            if (tf.flags & TFD_NONBLOCK) != 0 {
                return -crate::linux_compat::errno::EAGAIN;
            }
            drop(table);
            loop {
                crate::scheduler::yield_now();
                let mut t = TIMERFD_TABLE.lock();
                if idx >= t.len() {
                    return -crate::linux_compat::errno::EBADF;
                }
                if let Some(tf) = &mut t[idx] {
                    if current_tick() >= tf.expiry_tick && !tf.expired {
                        tf.expired = true;
                        if tf.interval_ns > 0 {
                            tf.expiry_tick = current_tick() + ns_to_ticks(tf.interval_ns);
                            tf.expired = false;
                        }
                        let expirations = 1u64;
                        buf[..8].copy_from_slice(&expirations.to_ne_bytes());
                        return 8;
                    }
                } else {
                    return -crate::linux_compat::errno::EBADF;
                }
            }
        }
        None => -crate::linux_compat::errno::EBADF,
    }
}

pub fn timerfd_close(fd: i32) -> i64 {
    let mut table = TIMERFD_TABLE.lock();
    let idx = (fd - TF_BASE) as usize;
    if idx < table.len() {
        table[idx] = None;
        0
    } else {
        -crate::linux_compat::errno::EBADF
    }
}

pub fn timerfd_ready(fd: i32) -> i32 {
    let table = TIMERFD_TABLE.lock();
    let idx = (fd - TF_BASE) as usize;
    match table.get(idx) {
        Some(Some(tf)) => {
            let mut mask = 2;
            if current_tick() >= tf.expiry_tick {
                mask |= 1;
            }
            mask
        }
        _ => 0,
    }
}
