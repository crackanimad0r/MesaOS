use alloc::vec::Vec;
use spin::Mutex;

pub const SF_BASE: i32 = 31000;
pub const SF_MAX: i32 = 32000;
const MAX_SIGNALFDS: usize = 32;

struct Signalfd {
    sigmask: [u64; 2],
    flags: u32,
}

pub static SIGNALFD_TABLE: Mutex<Vec<Option<Signalfd>>> = Mutex::new(Vec::new());

pub fn is_signalfd_fd(fd: i32) -> bool {
    fd >= SF_BASE && fd < SF_MAX
}

pub fn signalfd(fd: i32, mask_ptr: u64, _sizemask: u64, flags: u32) -> i64 {
    if fd != -1 && is_signalfd_fd(fd) {
        let mut table = SIGNALFD_TABLE.lock();
        let idx = (fd - SF_BASE) as usize;
        if idx < table.len() {
            if let Some(ref mut sf) = table[idx] {
                if mask_ptr != 0 && crate::security::validate_user_ptr(mask_ptr) {
                    let mask: [u64; 2] =
                        unsafe { core::ptr::read_volatile(mask_ptr as *const [u64; 2]) };
                    sf.sigmask = mask;
                }
                sf.flags = flags;
                return fd as i64;
            }
        }
        return -crate::linux_compat::errno::EBADF;
    }
    let mut table = SIGNALFD_TABLE.lock();
    let idx = table.len();
    if idx >= MAX_SIGNALFDS {
        return -crate::linux_compat::errno::EMFILE;
    }
    let sigmask = if mask_ptr != 0 && crate::security::validate_user_ptr(mask_ptr) {
        unsafe { core::ptr::read_volatile(mask_ptr as *const [u64; 2]) }
    } else {
        [0u64; 2]
    };
    table.push(Some(Signalfd { sigmask, flags }));
    (SF_BASE + idx as i32) as i64
}

pub fn signalfd_read(fd: i32, _buf: &mut [u8]) -> i64 {
    let table = SIGNALFD_TABLE.lock();
    let idx = (fd - SF_BASE) as usize;
    if idx >= table.len() || table[idx].is_none() {
        return -crate::linux_compat::errno::EBADF;
    }
    -crate::linux_compat::errno::EAGAIN
}

pub fn signalfd_close(fd: i32) -> i64 {
    let mut table = SIGNALFD_TABLE.lock();
    let idx = (fd - SF_BASE) as usize;
    if idx < table.len() {
        table[idx] = None;
        0
    } else {
        -crate::linux_compat::errno::EBADF
    }
}

pub fn signalfd_ready(_fd: i32) -> i32 {
    0
}
