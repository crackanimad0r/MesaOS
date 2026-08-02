use alloc::vec::Vec;
use spin::Mutex;

pub const EP_BASE: i32 = 33000;
pub const EP_MAX: i32 = 34000;
const MAX_EPOLL_FDS: usize = 64;

pub const EPOLL_CTL_ADD: i32 = 1;
pub const EPOLL_CTL_DEL: i32 = 2;
pub const EPOLL_CTL_MOD: i32 = 3;

pub const EPOLLIN: u32 = 0x001;
pub const EPOLLOUT: u32 = 0x004;
pub const EPOLLERR: u32 = 0x008;
pub const EPOLLHUP: u32 = 0x010;
pub const EPOLLET: u32 = 0x8000_0000;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct epoll_event {
    pub events: u32,
    pub data: u64,
}

struct EpollItem {
    fd: i32,
    events: u32,
    data: u64,
}

struct EpollInstance {
    fds: Vec<EpollItem>,
}

pub static EPOLL_TABLE: Mutex<Vec<Option<EpollInstance>>> = Mutex::new(Vec::new());

pub fn is_epoll_fd(fd: i32) -> bool {
    fd >= EP_BASE && fd < EP_MAX
}

pub fn epoll_create1(flags: u32) -> i64 {
    let mut table = EPOLL_TABLE.lock();
    let idx = table.len();
    if idx >= MAX_EPOLL_FDS {
        return -crate::linux_compat::errno::EMFILE;
    }
    let _ = flags;
    table.push(Some(EpollInstance { fds: Vec::new() }));
    (EP_BASE + idx as i32) as i64
}

fn check_fd_ready_general(fd: i32) -> i32 {
    if fd < 0 {
        return 0;
    }
    if fd >= 0 && fd <= 2 {
        return 3;
    }
    if crate::pipe::is_pipe_fd(fd) {
        return 3;
    }
    if fd >= 10000 && fd < 30000 {
        let table = crate::linux_compat::socket::SOCKET_TABLE.lock();
        let idx = (fd - 10000) as usize;
        if let Some(Some(sock)) = table.get(idx) {
            let mut mask = 2;
            match sock {
                crate::linux_compat::socket::Socket::Udp(udp) => {
                    if !udp.rx_buffer.is_empty() {
                        mask |= 1;
                    }
                }
                crate::linux_compat::socket::Socket::Tcp(tcp) => {
                    if !tcp.rx_buffer.is_empty() {
                        mask |= 1;
                    }
                }
            }
            return mask;
        }
        return 0;
    }
    if fd >= 30000 && fd < 31000 {
        return crate::linux_compat::eventfd::eventfd_ready(fd);
    }
    if fd >= 31000 && fd < 32000 {
        return crate::linux_compat::signalfd::signalfd_ready(fd);
    }
    if fd >= 32000 && fd < 33000 {
        return crate::linux_compat::timerfd::timerfd_ready(fd);
    }
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if table.contains_key(&fd) {
            3
        } else {
            0
        }
    })
    .unwrap_or(0)
}

pub fn epoll_ctl(epfd: i32, op: i32, fd: i32, event_ptr: u64) -> i64 {
    let mut table = EPOLL_TABLE.lock();
    let eidx = (epfd - EP_BASE) as usize;
    if eidx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    match &mut table[eidx] {
        Some(inst) => match op {
            EPOLL_CTL_ADD => {
                if inst.fds.iter().any(|item| item.fd == fd) {
                    return -crate::linux_compat::errno::EEXIST;
                }
                let (events, data) =
                    if event_ptr != 0 && crate::security::validate_user_ptr(event_ptr) {
                        let ev: epoll_event =
                            unsafe { core::ptr::read_volatile(event_ptr as *const epoll_event) };
                        (ev.events, ev.data)
                    } else {
                        (0, 0)
                    };
                inst.fds.push(EpollItem { fd, events, data });
                0
            }
            EPOLL_CTL_DEL => {
                let pos = inst.fds.iter().position(|item| item.fd == fd);
                match pos {
                    Some(p) => {
                        inst.fds.remove(p);
                        0
                    }
                    None => -crate::linux_compat::errno::ENOENT,
                }
            }
            EPOLL_CTL_MOD => {
                let (events, data) =
                    if event_ptr != 0 && crate::security::validate_user_ptr(event_ptr) {
                        let ev: epoll_event =
                            unsafe { core::ptr::read_volatile(event_ptr as *const epoll_event) };
                        (ev.events, ev.data)
                    } else {
                        (0, 0)
                    };
                match inst.fds.iter_mut().find(|item| item.fd == fd) {
                    Some(item) => {
                        item.events = events;
                        item.data = data;
                        0
                    }
                    None => -crate::linux_compat::errno::ENOENT,
                }
            }
            _ => -crate::linux_compat::errno::EINVAL,
        },
        None => -crate::linux_compat::errno::EBADF,
    }
}

pub fn epoll_wait(epfd: i32, events_ptr: u64, maxevents: i32, timeout: i32) -> i64 {
    if events_ptr == 0 || maxevents <= 0 {
        return -crate::linux_compat::errno::EINVAL;
    }
    let maxevents = maxevents.min(1024) as usize;
    if !crate::security::validate_user_buffer(events_ptr, maxevents * 12) {
        return -crate::linux_compat::errno::EFAULT;
    }
    let deadline = if timeout > 0 {
        Some(current_tick() + (timeout as u64 * 18 / 1000).max(1))
    } else {
        None
    };
    loop {
        let table = EPOLL_TABLE.lock();
        let eidx = (epfd - EP_BASE) as usize;
        let ready: Vec<epoll_event> = match table.get(eidx) {
            Some(Some(inst)) => inst
                .fds
                .iter()
                .filter_map(|item| {
                    let ready = check_fd_ready_general(item.fd);
                    let mut revents: u32 = 0;
                    if (item.events & EPOLLIN) != 0 && (ready & 1) != 0 {
                        revents |= EPOLLIN;
                    }
                    if (item.events & EPOLLOUT) != 0 && (ready & 2) != 0 {
                        revents |= EPOLLOUT;
                    }
                    if (ready & 4) != 0 {
                        revents |= EPOLLERR;
                    }
                    if revents == 0 {
                        if (item.events & EPOLLET) != 0 {
                            return None;
                        }
                        return None;
                    }
                    Some(epoll_event {
                        events: revents,
                        data: item.data,
                    })
                })
                .take(maxevents)
                .collect(),
            _ => return -crate::linux_compat::errno::EBADF,
        };
        let n = ready.len();
        if n > 0 {
            drop(table);
            for (i, ev) in ready.iter().enumerate() {
                if i >= maxevents {
                    break;
                }
                unsafe {
                    core::ptr::write_volatile(
                        (events_ptr + (i as u64 * 12)) as *mut epoll_event,
                        *ev,
                    );
                }
            }
            return n as i64;
        }
        drop(table);
        if timeout == 0 {
            return 0;
        }
        if let Some(dl) = deadline {
            if current_tick() >= dl {
                return 0;
            }
        }
        crate::scheduler::yield_now();
    }
}

fn current_tick() -> u64 {
    crate::curr_arch::get_ticks()
}

pub fn epoll_close(fd: i32) -> i64 {
    let mut table = EPOLL_TABLE.lock();
    let idx = (fd - EP_BASE) as usize;
    if idx < table.len() {
        table[idx] = None;
        0
    } else {
        -crate::linux_compat::errno::EBADF
    }
}
