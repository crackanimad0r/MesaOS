use alloc::vec::Vec;
use spin::Mutex;

pub const EF_BASE: i32 = 30000;
pub const EF_MAX: i32 = 31000;
const MAX_EVENTFDS: usize = 128;
const EFD_SEMAPHORE: u32 = 1;
const EFD_NONBLOCK: u32 = 0x800;

struct Eventfd {
    counter: u64,
    flags: u32,
}

pub static EVENTFD_TABLE: Mutex<Vec<Option<Eventfd>>> = Mutex::new(Vec::new());

pub fn eventfd(initval: u32, flags: u32) -> i64 {
    let mut table = EVENTFD_TABLE.lock();
    let idx = table.len();
    if idx >= MAX_EVENTFDS {
        return -crate::linux_compat::errno::EMFILE;
    }
    table.push(Some(Eventfd {
        counter: initval as u64,
        flags,
    }));
    (EF_BASE + idx as i32) as i64
}

pub fn is_eventfd_fd(fd: i32) -> bool {
    fd >= EF_BASE && fd < EF_MAX
}

pub fn eventfd_read(fd: i32, buf: &mut [u8]) -> i64 {
    if buf.len() < 8 {
        return -crate::linux_compat::errno::EINVAL;
    }
    let mut table = EVENTFD_TABLE.lock();
    let idx = (fd - EF_BASE) as usize;
    if idx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    match &mut table[idx] {
        Some(ev) => {
            if ev.counter == 0 {
                if (ev.flags & EFD_NONBLOCK) != 0 {
                    return -crate::linux_compat::errno::EAGAIN;
                }
                drop(table);
                loop {
                    crate::scheduler::yield_now();
                    let mut t = EVENTFD_TABLE.lock();
                    if idx >= t.len() {
                        return -crate::linux_compat::errno::EBADF;
                    }
                    if let Some(e) = &mut t[idx] {
                        if e.counter > 0 {
                            break;
                        }
                    } else {
                        return -crate::linux_compat::errno::EBADF;
                    }
                }
                let mut t = EVENTFD_TABLE.lock();
                let e = t[idx].as_mut().unwrap();
                let val = if (e.flags & EFD_SEMAPHORE) != 0 {
                    e.counter -= 1;
                    1u64
                } else {
                    let v = e.counter;
                    e.counter = 0;
                    v
                };
                buf[..8].copy_from_slice(&val.to_ne_bytes());
                return 8;
            }
            let val = if (ev.flags & EFD_SEMAPHORE) != 0 {
                ev.counter -= 1;
                1u64
            } else {
                let v = ev.counter;
                ev.counter = 0;
                v
            };
            buf[..8].copy_from_slice(&val.to_ne_bytes());
            8
        }
        None => -crate::linux_compat::errno::EBADF,
    }
}

pub fn eventfd_write(fd: i32, buf: &[u8]) -> i64 {
    if buf.len() < 8 {
        return -crate::linux_compat::errno::EINVAL;
    }
    let mut val = [0u8; 8];
    val.copy_from_slice(&buf[..8]);
    let add = u64::from_ne_bytes(val);
    if add == 0 {
        return -crate::linux_compat::errno::EINVAL;
    }
    const MAX_VAL: u64 = 0xfffffffffffffffe;
    let mut table = EVENTFD_TABLE.lock();
    let idx = (fd - EF_BASE) as usize;
    if idx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    match &mut table[idx] {
        Some(ev) => {
            if ev.counter > MAX_VAL - add {
                if (ev.flags & EFD_NONBLOCK) != 0 {
                    return -crate::linux_compat::errno::EAGAIN;
                }
                drop(table);
                loop {
                    crate::scheduler::yield_now();
                    let mut t = EVENTFD_TABLE.lock();
                    if idx >= t.len() {
                        return -crate::linux_compat::errno::EBADF;
                    }
                    if let Some(e) = &mut t[idx] {
                        if e.counter <= MAX_VAL - add {
                            e.counter += add;
                            return 8;
                        }
                    } else {
                        return -crate::linux_compat::errno::EBADF;
                    }
                }
            }
            ev.counter += add;
            8
        }
        None => -crate::linux_compat::errno::EBADF,
    }
}

pub fn eventfd_close(fd: i32) -> i64 {
    let mut table = EVENTFD_TABLE.lock();
    let idx = (fd - EF_BASE) as usize;
    if idx < table.len() {
        table[idx] = None;
        0
    } else {
        -crate::linux_compat::errno::EBADF
    }
}

pub fn eventfd_ready(fd: i32) -> i32 {
    let table = EVENTFD_TABLE.lock();
    let idx = (fd - EF_BASE) as usize;
    match table.get(idx) {
        Some(Some(ev)) => {
            let mut mask = 2;
            if ev.counter > 0 {
                mask |= 1;
            }
            mask
        }
        _ => 0,
    }
}
