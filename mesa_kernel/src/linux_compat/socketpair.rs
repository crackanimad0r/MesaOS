use alloc::vec::Vec;
use spin::Mutex;

pub const SP_BASE: i32 = 40000;
pub const SP_MAX: i32 = 41000;
const MAX_PAIRS: usize = 128;

const BUF_SIZE: usize = 4096;

struct SocketPair {
    buf_a_to_b: Vec<u8>,
    buf_b_to_a: Vec<u8>,
}

pub static SOCKETPAIR_TABLE: Mutex<Vec<Option<SocketPair>>> = Mutex::new(Vec::new());

fn pair_idx(fd: i32) -> Option<(usize, bool)> {
    if fd < SP_BASE || fd >= SP_MAX {
        return None;
    }
    let raw = (fd - SP_BASE) as usize;
    let idx = raw / 2;
    let is_b = (raw & 1) != 0;
    Some((idx, is_b))
}

pub fn is_socketpair_fd(fd: i32) -> bool {
    fd >= SP_BASE && fd < SP_MAX
}

pub fn socketpair(domain: i32, type_: i32, _protocol: i32, sv_ptr: u64) -> i64 {
    if domain != 1 {
        return -crate::linux_compat::errno::EAFNOSUPPORT;
    }
    if type_ != 1 {
        return -crate::linux_compat::errno::EPROTONOSUPPORT;
    }
    if sv_ptr == 0 || !crate::security::validate_user_buffer(sv_ptr, 8) {
        return -crate::linux_compat::errno::EFAULT;
    }
    let mut table = SOCKETPAIR_TABLE.lock();
    let idx = table.len();
    if idx >= MAX_PAIRS {
        return -crate::linux_compat::errno::EMFILE;
    }
    table.push(Some(SocketPair {
        buf_a_to_b: Vec::new(),
        buf_b_to_a: Vec::new(),
    }));
    let fd_a = SP_BASE + (idx as i32) * 2;
    let fd_b = fd_a + 1;
    unsafe {
        core::ptr::write_volatile(sv_ptr as *mut i32, fd_a);
        core::ptr::write_volatile((sv_ptr + 4) as *mut i32, fd_b);
    }
    0
}

pub fn socketpair_read(fd: i32, buf: &mut [u8]) -> i64 {
    let (idx, is_b) = match pair_idx(fd) {
        Some(v) => v,
        None => return -crate::linux_compat::errno::EBADF,
    };
    let mut table = SOCKETPAIR_TABLE.lock();
    match &mut table[idx] {
        Some(sp) => {
            let src = if is_b {
                &mut sp.buf_a_to_b
            } else {
                &mut sp.buf_b_to_a
            };
            if src.is_empty() {
                return 0;
            }
            let len = buf.len().min(src.len());
            buf[..len].copy_from_slice(&src[..len]);
            src.drain(..len);
            len as i64
        }
        None => -crate::linux_compat::errno::EBADF,
    }
}

pub fn socketpair_write(fd: i32, buf: &[u8]) -> i64 {
    let (idx, is_b) = match pair_idx(fd) {
        Some(v) => v,
        None => return -crate::linux_compat::errno::EBADF,
    };
    let mut table = SOCKETPAIR_TABLE.lock();
    match &mut table[idx] {
        Some(sp) => {
            let dst = if is_b {
                &mut sp.buf_b_to_a
            } else {
                &mut sp.buf_a_to_b
            };
            let len = buf.len().min(BUF_SIZE - dst.len());
            if len == 0 {
                return -crate::linux_compat::errno::EAGAIN;
            }
            dst.extend_from_slice(&buf[..len]);
            len as i64
        }
        None => -crate::linux_compat::errno::EBADF,
    }
}

pub fn socketpair_close(fd: i32) -> i64 {
    let (idx, _is_b) = match pair_idx(fd) {
        Some(v) => v,
        None => return -crate::linux_compat::errno::EBADF,
    };
    let mut table = SOCKETPAIR_TABLE.lock();
    if idx < table.len() {
        table[idx] = None;
        0
    } else {
        -crate::linux_compat::errno::EBADF
    }
}

pub fn socketpair_ready(fd: i32) -> i32 {
    let (idx, is_b) = match pair_idx(fd) {
        Some(v) => v,
        None => return 0,
    };
    let table = SOCKETPAIR_TABLE.lock();
    match &table[idx] {
        Some(sp) => {
            let src = if is_b { &sp.buf_a_to_b } else { &sp.buf_b_to_a };
            let dst = if is_b { &sp.buf_b_to_a } else { &sp.buf_a_to_b };
            let mut mask = 0;
            if !src.is_empty() {
                mask |= 1;
            }
            if dst.len() < BUF_SIZE {
                mask |= 2;
            }
            mask
        }
        None => 0,
    }
}
