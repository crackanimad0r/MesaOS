#![allow(dead_code)]
#![allow(non_camel_case_types)]

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

pub const SIGHUP: i32 = 1;
pub const SIGINT: i32 = 2;
pub const SIGQUIT: i32 = 3;
pub const SIGILL: i32 = 4;
pub const SIGTRAP: i32 = 5;
pub const SIGABRT: i32 = 6;
pub const SIGBUS: i32 = 7;
pub const SIGFPE: i32 = 8;
pub const SIGKILL: i32 = 9;
pub const SIGUSR1: i32 = 10;
pub const SIGSEGV: i32 = 11;
pub const SIGUSR2: i32 = 12;
pub const SIGPIPE: i32 = 13;
pub const SIGALRM: i32 = 14;
pub const SIGTERM: i32 = 15;
pub const SIGSTKFLT: i32 = 16;
pub const SIGCHLD: i32 = 17;
pub const SIGCONT: i32 = 18;
pub const SIGSTOP: i32 = 19;
pub const SIGTSTP: i32 = 20;
pub const SIGTTIN: i32 = 21;
pub const SIGTTOU: i32 = 22;
pub const SIGURG: i32 = 23;
pub const SIGXCPU: i32 = 24;
pub const SIGXFSZ: i32 = 25;
pub const SIGVTALRM: i32 = 26;
pub const SIGPROF: i32 = 27;
pub const SIGWINCH: i32 = 28;
pub const SIGIO: i32 = 29;
pub const SIGPWR: i32 = 30;
pub const SIGSYS: i32 = 31;

pub const SIG_DFL: u64 = 0;
pub const SIG_IGN: u64 = 1;

pub const SA_NOCLDSTOP: u64 = 0x00000001;
pub const SA_NOCLDWAIT: u64 = 0x00000002;
pub const SA_SIGINFO: u64 = 0x00000004;
pub const SA_ONSTACK: u64 = 0x08000000;
pub const SA_RESTART: u64 = 0x10000000;
pub const SA_NODEFER: u64 = 0x40000000;
pub const SA_RESETHAND: u64 = 0x80000000;

pub const SIG_BLOCK: i32 = 0;
pub const SIG_UNBLOCK: i32 = 1;
pub const SIG_SETMASK: i32 = 2;

pub const SI_QUEUE: i32 = -1;
pub const SI_USER: i32 = 0;
pub const SI_KERNEL: i32 = 128;

pub const SS_ONSTACK: i32 = 1;
pub const SS_DISABLE: i32 = 2;

const _NSIG: usize = 64;
const _NSIG_BPW: usize = 64;
const _NSIG_WORDS: usize = _NSIG / _NSIG_BPW;

pub type sigset_t = [u64; _NSIG_WORDS];

pub fn sigemptyset() -> sigset_t {
    [0u64; _NSIG_WORDS]
}

pub fn sigfillset() -> sigset_t {
    [!0u64; _NSIG_WORDS]
}

pub fn sigaddset(set: &mut sigset_t, sig: i32) {
    if sig > 0 && sig <= _NSIG as i32 {
        let bit = (sig - 1) as usize;
        set[bit / 64] |= 1u64 << (bit % 64);
    }
}

pub fn sigdelset(set: &mut sigset_t, sig: i32) {
    if sig > 0 && sig <= _NSIG as i32 {
        let bit = (sig - 1) as usize;
        set[bit / 64] &= !(1u64 << (bit % 64));
    }
}

pub fn sigismember(set: &sigset_t, sig: i32) -> bool {
    if sig > 0 && sig <= _NSIG as i32 {
        let bit = (sig - 1) as usize;
        (set[bit / 64] & (1u64 << (bit % 64))) != 0
    } else {
        false
    }
}

#[repr(C)]
pub struct sigaction {
    pub sa_handler: u64,
    pub sa_flags: u64,
    pub sa_restorer: u64,
    pub sa_mask: sigset_t,
}

pub struct SignalAction {
    pub handler: u64,
    pub flags: u64,
    pub restorer: u64,
    pub mask: sigset_t,
}

impl SignalAction {
    pub const fn default() -> Self {
        Self {
            handler: 0,
            flags: 0,
            restorer: 0,
            mask: [0u64; _NSIG_WORDS],
        }
    }
}

pub struct SignalInfo {
    pub si_signo: i32,
    pub si_errno: i32,
    pub si_code: i32,
    pub si_pid: i32,
    pub si_uid: i32,
    pub si_status: i32,
    pub si_addr: u64,
}

impl SignalInfo {
    pub fn new(signo: i32, code: i32) -> Self {
        Self {
            si_signo: signo,
            si_errno: 0,
            si_code: code,
            si_pid: 0,
            si_uid: 0,
            si_status: 0,
            si_addr: 0,
        }
    }
}

#[repr(C)]
pub struct sigaltstack {
    pub ss_sp: u64,
    pub ss_flags: i32,
    pub ss_size: u64,
}

impl sigaltstack {
    pub fn disabled() -> Self {
        Self {
            ss_sp: 0,
            ss_flags: SS_DISABLE as i32,
            ss_size: 0,
        }
    }
}

static SIGNAL_ACTIONS: Mutex<[[SignalAction; _NSIG]; 1]> = Mutex::new([{
    const DFL: SignalAction = SignalAction::default();
    [DFL; _NSIG]
}]);

pub fn get_signal_action(sig: usize) -> &'static SignalAction {
    unsafe {
        let ptr = SIGNAL_ACTIONS.lock().as_mut_ptr();
        &(*ptr)[sig]
    }
}

pub fn get_signal_action_mut(sig: usize) -> &'static mut SignalAction {
    unsafe {
        let ptr = SIGNAL_ACTIONS.lock().as_mut_ptr();
        &mut (*ptr)[sig]
    }
}

pub fn do_sigaction(sig: i32, act: u64, oldact: u64) -> i64 {
    if sig < 1 || sig >= _NSIG as i32 || sig == SIGKILL || sig == SIGSTOP {
        return -super::errno::EINVAL;
    }

    if oldact != 0 {
        let actions = SIGNAL_ACTIONS.lock();
        let old = &actions[0][sig as usize];
        let sigact = sigaction {
            sa_handler: old.handler,
            sa_flags: old.flags,
            sa_restorer: old.restorer,
            sa_mask: old.mask,
        };
        unsafe {
            core::ptr::write_volatile(oldact as *mut sigaction, sigact);
        }
    }

    if act != 0 {
        let sigact: sigaction = unsafe { core::ptr::read_volatile(act as *const sigaction) };
        let mut actions = SIGNAL_ACTIONS.lock();
        actions[0][sig as usize] = SignalAction {
            handler: sigact.sa_handler,
            flags: sigact.sa_flags,
            restorer: sigact.sa_restorer,
            mask: sigact.sa_mask,
        };
        if sigact.sa_handler == SIG_IGN {
            actions[0][sig as usize].handler = SIG_IGN;
        } else if sigact.sa_handler == SIG_DFL {
            actions[0][sig as usize].handler = SIG_DFL;
        }
    }

    0
}

fn arr_to_sigset(arr: [u64; 1]) -> sigset_t {
    let mut s = sigemptyset();
    s[0] = arr[0];
    s
}

pub fn do_sigprocmask(how: i32, set: u64, oldset: u64) -> i64 {
    if oldset != 0 {
        let block =
            crate::scheduler::with_current_task(|task| task.linux_sigblock).unwrap_or([0u64; 1]);

        let mask = arr_to_sigset(block);
        unsafe {
            core::ptr::write_volatile(oldset as *mut sigset_t, mask);
        }
    }

    if set != 0 {
        let newmask: sigset_t = unsafe { core::ptr::read_volatile(set as *const sigset_t) };
        let old_block =
            crate::scheduler::with_current_task(|task| task.linux_sigblock[0]).unwrap_or(0);

        let mut new_block = [0u64; 1];
        match how {
            SIG_BLOCK => {
                new_block[0] = old_block | newmask[0];
            }
            SIG_UNBLOCK => {
                new_block[0] = old_block & !newmask[0];
            }
            SIG_SETMASK => {
                new_block[0] = newmask[0];
            }
            _ => {
                new_block[0] = old_block;
            }
        }

        crate::scheduler::with_current_task(|task| {
            task.linux_sigblock = new_block;
        });
    }

    0
}

pub fn do_sigpending(set_ptr: u64) -> i64 {
    let pending = [0u64; _NSIG_WORDS];
    if set_ptr != 0 {
        unsafe {
            core::ptr::write_volatile(set_ptr as *mut sigset_t, pending);
        }
    }
    0
}

pub fn do_kill(pid: i32, sig: i32) -> i64 {
    if sig == 0 {
        return 0;
    }
    if pid <= 0 {
        return -super::errno::EINVAL;
    }
    if sig == SIGKILL {
        let _ = crate::scheduler::kill(pid as u64);
        return 0;
    }
    0
}

pub fn do_tkill(tid: i32, sig: i32) -> i64 {
    do_kill(tid, sig)
}

pub fn do_tgkill(_tgid: i32, tid: i32, sig: i32) -> i64 {
    do_kill(tid, sig)
}

pub fn do_sigaltstack(uss: u64, uoss: u64) -> i64 {
    if uoss != 0 {
        let altstack = sigaltstack::disabled();
        unsafe {
            core::ptr::write_volatile(uoss as *mut sigaltstack, altstack);
        }
    }
    0
}
