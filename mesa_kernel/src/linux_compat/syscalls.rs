use super::abi::syscall_numbers::*;
use super::errno;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

pub fn dispatch(num: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i64 {
    match num {
        LINUX_READ => linux_sys_read(arg1 as i32, arg2, arg3),
        LINUX_WRITE => linux_sys_write(arg1 as i32, arg2, arg3),
        LINUX_OPEN => linux_sys_open(arg1, arg2, arg3),
        LINUX_CLOSE => {
            let fd = arg1 as i32;
            if fd >= 10000 && fd < 30000 {
                crate::linux_compat::socket::socket_close(fd)
            } else if crate::linux_compat::socketpair::is_socketpair_fd(fd) {
                crate::linux_compat::socketpair::socketpair_close(fd)
            } else if crate::linux_compat::eventfd::is_eventfd_fd(fd) {
                crate::linux_compat::eventfd::eventfd_close(fd)
            } else if crate::linux_compat::signalfd::is_signalfd_fd(fd) {
                crate::linux_compat::signalfd::signalfd_close(fd)
            } else if crate::linux_compat::timerfd::is_timerfd_fd(fd) {
                crate::linux_compat::timerfd::timerfd_close(fd)
            } else if crate::linux_compat::epoll::is_epoll_fd(fd) {
                crate::linux_compat::epoll::epoll_close(fd)
            } else {
                linux_sys_close(fd)
            }
        }
        LINUX_STAT => linux_sys_stat(arg1, arg2),
        LINUX_FSTAT => linux_sys_fstat(arg1 as i32, arg2),
        LINUX_LSTAT => linux_sys_stat(arg1, arg2),
        LINUX_POLL => linux_sys_poll(arg1, arg2, arg3),
        LINUX_LSEEK => linux_sys_lseek(arg1 as i32, arg2, arg3 as i32),
        LINUX_MMAP => linux_sys_mmap(arg1, arg2, arg3 as i32, arg4 as i32, arg5 as i32, arg1), // addr=arg1
        LINUX_MPROTECT => linux_sys_mprotect(arg1, arg2, arg3 as i32),
        LINUX_MUNMAP => linux_sys_munmap(arg1, arg2),
        LINUX_BRK => linux_sys_brk(arg1),
        LINUX_RT_SIGACTION => super::signal::do_sigaction(arg1 as i32, arg2, arg3),
        LINUX_RT_SIGPROCMASK => super::signal::do_sigprocmask(arg1 as i32, arg2, arg3),
        LINUX_RT_SIGRETURN => linux_sys_rt_sigreturn(),
        LINUX_IOCTL => linux_sys_ioctl(arg1 as i32, arg2, arg3),
        LINUX_PREAD64 => linux_sys_pread64(arg1 as i32, arg2, arg3, arg4),
        LINUX_PWRITE64 => linux_sys_pwrite64(arg1 as i32, arg2, arg3, arg4),
        LINUX_READV => linux_sys_readv(arg1 as i32, arg2, arg3),
        LINUX_WRITEV => linux_sys_writev(arg1 as i32, arg2, arg3),
        LINUX_ACCESS => linux_sys_access(arg1, arg2 as i32),
        LINUX_PIPE => linux_sys_pipe(arg1),
        LINUX_SELECT => linux_sys_select(arg1 as i32, arg2, arg3, arg4, arg5),
        LINUX_SCHED_YIELD => {
            crate::scheduler::yield_now();
            0
        }
        LINUX_MREMAP => linux_sys_mremap(arg1, arg2, arg3, arg4 as i32, arg5),
        LINUX_MSYNC => -errno::EOPNOTSUPP,
        LINUX_MINCORE => -errno::EOPNOTSUPP,
        LINUX_MADVISE => 0,
        LINUX_DUP => linux_sys_dup(arg1 as i32),
        LINUX_DUP2 => linux_sys_dup2(arg1 as i32, arg2 as i32),
        LINUX_PAUSE => linux_sys_pause(),
        LINUX_NANOSLEEP => linux_sys_nanosleep(arg1, arg2),
        LINUX_GETITIMER => -errno::EINVAL,
        LINUX_ALARM => 0,
        LINUX_SETITIMER => -errno::EINVAL,
        LINUX_GETPID => crate::scheduler::current_task_id().unwrap_or(0) as i64,
        LINUX_SENDFILE => linux_sys_sendfile(arg1 as i32, arg2 as i32, arg3, arg4),
        LINUX_SOCKET => {
            crate::linux_compat::socket::socket_create(arg1 as i32, arg2 as i32, arg3 as i32)
        }
        LINUX_CONNECT => linux_sys_connect(arg1 as i32, arg2, arg3),
        LINUX_ACCEPT => linux_sys_accept(arg1 as i32, arg2, arg3),
        LINUX_SENDTO => linux_sys_sendto(arg1 as i32, arg2, arg3, arg4 as i32, arg5),
        LINUX_RECVFROM => linux_sys_recvfrom(arg1 as i32, arg2, arg3, arg4 as i32, arg5),
        LINUX_SENDMSG => linux_sys_sendmsg(arg1 as i32, arg2, arg3, arg4),
        LINUX_RECVMSG => linux_sys_recvmsg(arg1 as i32, arg2, arg3),
        LINUX_SHUTDOWN => linux_sys_shutdown(arg1 as i32, arg2 as i32),
        LINUX_BIND => linux_sys_bind(arg1 as i32, arg2, arg3),
        LINUX_LISTEN => linux_sys_listen(arg1 as i32, arg2 as i32),
        LINUX_GETSOCKNAME => linux_sys_getsockname(arg1 as i32, arg2, arg3),
        LINUX_GETPEERNAME => linux_sys_getpeername(arg1 as i32, arg2, arg3),
        LINUX_SOCKETPAIR => {
            crate::linux_compat::socketpair::socketpair(arg1 as i32, arg2 as i32, arg3 as i32, arg4)
        }
        LINUX_SETSOCKOPT => linux_sys_setsockopt(arg1 as i32, arg2 as i32, arg3 as i32, arg4, arg5),
        LINUX_GETSOCKOPT => linux_sys_getsockopt(arg1 as i32, arg2 as i32, arg3 as i32, arg4, arg5),
        LINUX_CLONE => linux_sys_clone(arg1, arg2, arg3, arg4, arg5),
        LINUX_FORK => linux_sys_fork(),
        LINUX_VFORK => linux_sys_vfork(),
        LINUX_EXECVE => linux_sys_execve(arg1, arg2, arg3),
        LINUX_EXIT => linux_sys_exit(arg1 as i32),
        LINUX_WAIT4 => linux_sys_wait4(arg1 as i64, arg2, arg3 as i32, arg4),
        LINUX_KILL => super::signal::do_kill(arg1 as i32, arg2 as i32),
        LINUX_UNAME => linux_sys_uname(arg1),
        LINUX_FCNTL => linux_sys_fcntl(arg1 as i32, arg2, arg3),
        LINUX_FLOCK => linux_sys_fsync(arg1 as i32),
        LINUX_FSYNC => linux_sys_fsync(arg1 as i32),
        LINUX_FDATASYNC => linux_sys_fsync(arg1 as i32),
        LINUX_TRUNCATE => linux_sys_truncate(arg1, arg2),
        LINUX_FTRUNCATE => linux_sys_ftruncate(arg1 as i32, arg2),
        LINUX_GETDENTS => linux_sys_getdents(arg1 as i32, arg2, arg3),
        LINUX_GETCWD => linux_sys_getcwd(arg1, arg2),
        LINUX_CHDIR => linux_sys_chdir(arg1),
        LINUX_FCHDIR => linux_sys_fchdir(arg1 as i32),
        LINUX_RENAME => linux_sys_rename(arg1, arg2),
        LINUX_MKDIR => linux_sys_mkdir(arg1, arg2 as i32),
        LINUX_RMDIR => linux_sys_rmdir(arg1),
        LINUX_CREAT => linux_sys_open(
            arg1,
            (super::O_CREAT | super::O_WRONLY | super::O_TRUNC) as u64,
            arg2,
        ),
        LINUX_LINK => linux_sys_link(arg1, arg2),
        LINUX_UNLINK => linux_sys_unlink(arg1),
        LINUX_SYMLINK => linux_sys_symlink(arg1, arg2),
        LINUX_READLINK => linux_sys_readlink(arg1, arg2, arg3),
        LINUX_CHMOD => linux_sys_chmod(arg1, arg2 as i32),
        LINUX_FCHMOD => linux_sys_fchmod(arg1 as i32, arg2),
        LINUX_CHOWN => linux_sys_chown(arg1, arg2 as u32, arg3 as u32),
        LINUX_FCHOWN => linux_sys_fchown(arg1 as i32, arg2 as u32, arg3 as u32),
        LINUX_LCHOWN => linux_sys_lchown(arg1, arg2 as u32, arg3 as u32),
        LINUX_UMASK => linux_sys_umask(arg1 as i32),
        LINUX_GETTIMEOFDAY => linux_sys_gettimeofday(arg1, arg2),
        LINUX_GETRLIMIT => linux_sys_getrlimit(arg1 as i32, arg2),
        LINUX_GETRUSAGE => linux_sys_getrusage(arg1 as i32, arg2),
        LINUX_SYSINFO => linux_sys_sysinfo(arg1),
        LINUX_TIMES => linux_sys_times(arg1),
        LINUX_PTRACE => -errno::EOPNOTSUPP,
        LINUX_GETUID => crate::users::current_uid() as i64,
        LINUX_SYSLOG => -errno::EOPNOTSUPP,
        LINUX_GETGID => crate::users::current_gid() as i64,
        LINUX_SETUID => {
            crate::users::set_uid(arg1 as u32);
            0
        }
        LINUX_SETGID => {
            crate::users::set_gid(arg1 as u32);
            0
        }
        LINUX_GETEUID => crate::users::current_uid() as i64,
        LINUX_GETEGID => crate::users::current_gid() as i64,
        LINUX_SETPGID => linux_sys_setpgid(arg1 as i32, arg2 as i32),
        LINUX_GETPPID => linux_sys_getppid(),
        LINUX_GETPGRP => linux_sys_getpgrp(),
        LINUX_SETSID => linux_sys_setsid(),
        LINUX_SETREUID => linux_sys_setreuid(arg1 as u32, arg2 as u32),
        LINUX_SETREGID => linux_sys_setregid(arg1 as u32, arg2 as u32),
        LINUX_GETGROUPS => linux_sys_getgroups(arg1 as i32, arg2),
        LINUX_SETGROUPS => linux_sys_setgroups(arg1 as u32, arg2),
        LINUX_SETRESUID => linux_sys_setresuid(arg1 as u32, arg2 as u32, arg3 as u32),
        LINUX_GETRESUID => linux_sys_getresuid(arg1),
        LINUX_SETRESGID => linux_sys_setresgid(arg1 as u32, arg2 as u32, arg3 as u32),
        LINUX_GETRESGID => linux_sys_getresgid(arg1),
        LINUX_GETPGID => linux_sys_getpgid(arg1 as i32),
        LINUX_SETFSUID => linux_sys_setfsuid(arg1 as u32),
        LINUX_SETFSGID => linux_sys_setfsgid(arg1 as u32),
        LINUX_GETSID => linux_sys_getsid(arg1 as i32),
        LINUX_CAPGET => -errno::EOPNOTSUPP,
        LINUX_CAPSET => -errno::EOPNOTSUPP,
        LINUX_RT_SIGTIMEDWAIT => -errno::EOPNOTSUPP,
        LINUX_RT_SIGQUEUEINFO => -errno::EOPNOTSUPP,
        LINUX_RT_SIGSUSPEND => -errno::EOPNOTSUPP,
        LINUX_SIGALTSTACK => super::signal::do_sigaltstack(arg1, arg2),
        LINUX_UTIME => linux_sys_utime(arg1, arg2),
        LINUX_MKNOD => -errno::EOPNOTSUPP,
        LINUX_SETPRIORITY => linux_sys_setpriority(arg1 as i32, arg2 as i32, arg3 as i32),
        LINUX_GETPRIORITY => linux_sys_getpriority(arg1 as i32, arg2 as i32),
        LINUX_STATFS => linux_sys_statfs(arg1, arg2),
        LINUX_FSTATFS => linux_sys_fstatfs(arg1 as i32, arg2),
        LINUX_SCHED_SETPARAM => linux_sys_sched_setparam(arg1 as i32, arg2),
        LINUX_SCHED_GETPARAM => linux_sys_sched_getparam(arg1 as i32, arg2),
        LINUX_SCHED_SETSCHEDULER => linux_sys_sched_setscheduler(arg1 as i32, arg2 as i32, arg3),
        LINUX_SCHED_GETSCHEDULER => linux_sys_sched_getscheduler(arg1 as i32),
        LINUX_SCHED_GET_PRIORITY_MAX => linux_sys_sched_get_priority_max(arg1 as i32),
        LINUX_SCHED_GET_PRIORITY_MIN => linux_sys_sched_get_priority_min(arg1 as i32),
        LINUX_SCHED_RR_GET_INTERVAL => linux_sys_sched_rr_get_interval(arg1 as i32, arg2),
        LINUX_MLOCK => linux_sys_mlock(arg1, arg2),
        LINUX_MUNLOCK => linux_sys_munlock(arg1, arg2),
        LINUX_MLOCKALL => linux_sys_mlockall(arg1 as i32),
        LINUX_MUNLOCKALL => linux_sys_munlockall(),
        LINUX_PRCTL => linux_sys_prctl(arg1 as i32, arg2, arg3, arg4, arg5),
        LINUX_ARCH_PRCTL => linux_sys_arch_prctl(arg1 as i32, arg2, arg3),
        LINUX_ADJTIMEX => -errno::EOPNOTSUPP,
        LINUX_SETRLIMIT => linux_sys_setrlimit(arg1 as i32, arg2),
        LINUX_CHROOT => -errno::EPERM,
        LINUX_SYNC => linux_sys_sync(),
        LINUX_SETTIMEOFDAY => -errno::EPERM,
        LINUX_MOUNT => -errno::EPERM,
        LINUX_UMOUNT2 => -errno::EPERM,
        LINUX_SWAPON => -errno::EPERM,
        LINUX_SWAPOFF => -errno::EPERM,
        LINUX_REBOOT => -errno::EPERM,
        LINUX_SETHOSTNAME => -errno::EPERM,
        LINUX_SETDOMAINNAME => -errno::EPERM,
        LINUX_IOPL => -errno::EPERM,
        LINUX_IOPERM => -errno::EPERM,
        LINUX_QUOTACTL => -errno::EOPNOTSUPP,
        LINUX_GETTID => crate::scheduler::current_task_id().unwrap_or(0) as i64,
        LINUX_READAHEAD => 0, // OK - readahead hint
        LINUX_TKILL => super::signal::do_tkill(arg1 as i32, arg2 as i32),
        LINUX_TIME => linux_sys_time(arg1),
        LINUX_FUTEX => linux_sys_futex(arg1, arg2 as i32, arg3 as i32, arg4, arg5),
        LINUX_SCHED_SETAFFINITY => linux_sys_sched_setaffinity(arg1 as i32, arg2, arg3),
        LINUX_SCHED_GETAFFINITY => linux_sys_sched_getaffinity(arg1 as i32, arg2, arg3),
        LINUX_SET_THREAD_AREA => linux_sys_set_thread_area(arg1),
        LINUX_GET_THREAD_AREA => linux_sys_get_thread_area(arg1),
        LINUX_GETDENTS64 => linux_sys_getdents64(arg1 as i32, arg2, arg3),
        LINUX_SET_TID_ADDRESS => linux_sys_set_tid_address(arg1 as i32),
        LINUX_RESTART_SYSCALL => -errno::EINTR,
        LINUX_FADVISE64 => 0, // OK - fadvise hint
        LINUX_TIMER_CREATE => {
            crate::linux_compat::posix_timer::posix_timer_create(arg1 as i32, arg2, arg3)
        }
        LINUX_TIMER_SETTIME => crate::linux_compat::posix_timer::posix_timer_settime(
            arg1 as i32,
            arg2 as i32,
            arg3,
            arg4,
        ),
        LINUX_TIMER_GETTIME => {
            crate::linux_compat::posix_timer::posix_timer_gettime(arg1 as i32, arg2)
        }
        LINUX_TIMER_GETOVERRUN => linux_sys_timer_getoverrun(arg1 as i32),
        LINUX_TIMER_DELETE => crate::linux_compat::posix_timer::posix_timer_delete(arg1 as i32),
        LINUX_CLOCK_SETTIME => -errno::EPERM,
        LINUX_CLOCK_GETTIME => linux_sys_clock_gettime(arg1 as i32, arg2),
        LINUX_CLOCK_GETRES => linux_sys_clock_getres(arg1 as i32, arg2),
        LINUX_CLOCK_NANOSLEEP => linux_sys_clock_nanosleep(arg1 as i32, arg2 as i32, arg3, arg4),
        LINUX_EXIT_GROUP => linux_sys_exit(arg1 as i32),
        LINUX_EPOLL_CREATE => crate::linux_compat::epoll::epoll_create1(0),
        LINUX_EPOLL_CTL => {
            crate::linux_compat::epoll::epoll_ctl(arg1 as i32, arg2 as i32, arg3 as i32, arg4)
        }
        LINUX_EPOLL_WAIT => {
            crate::linux_compat::epoll::epoll_wait(arg1 as i32, arg2, arg3 as i32, arg4 as i32)
        }
        LINUX_TGKILL => super::signal::do_tgkill(arg1 as i32, arg2 as i32, arg3 as i32),
        LINUX_UTIMES => linux_sys_utimes(arg1, arg2),
        LINUX_MQ_OPEN => -errno::EOPNOTSUPP,
        LINUX_MQ_UNLINK => -errno::EOPNOTSUPP,
        LINUX_KEXEC_LOAD => -errno::EPERM,
        LINUX_WAITID => -errno::EOPNOTSUPP,
        LINUX_ADD_KEY => -errno::EOPNOTSUPP,
        LINUX_REQUEST_KEY => -errno::EOPNOTSUPP,
        LINUX_KEYCTL => -errno::EOPNOTSUPP,
        LINUX_IOPRIO_SET => linux_sys_ioprio_set(arg1 as i32, arg2 as i32, arg3 as i32),
        LINUX_IOPRIO_GET => linux_sys_ioprio_get(arg1 as i32, arg2 as i32),
        LINUX_INOTIFY_INIT => -errno::EOPNOTSUPP,
        LINUX_INOTIFY_ADD_WATCH => -errno::EOPNOTSUPP,
        LINUX_INOTIFY_RM_WATCH => -errno::EOPNOTSUPP,
        LINUX_OPENAT => linux_sys_openat(arg1 as i32, arg2, arg3 as i32, arg4),
        LINUX_MKDIRAT => linux_sys_mkdirat(arg1 as i32, arg2, arg3 as i32),
        LINUX_UNLINKAT => linux_sys_unlinkat(arg1 as i32, arg2, arg3 as i32),
        LINUX_RENAMEAT => linux_sys_renameat(arg1 as i32, arg2, arg3 as i32, arg4),
        LINUX_NEWFSTATAT => linux_sys_newfstatat(arg1 as i32, arg2, arg3, arg4 as i32),
        LINUX_READLINKAT => linux_sys_readlinkat(arg1 as i32, arg2, arg3, arg4),
        LINUX_FCHMODAT => linux_sys_fchmodat(arg1 as i32, arg2, arg3 as i32, arg4 as i32),
        LINUX_FACCESSAT => linux_sys_faccessat(arg1 as i32, arg2, arg3 as i32, arg4 as i32),
        LINUX_PSELECT6 => linux_sys_pselect6(arg1 as i32, arg2, arg3, arg4, arg5),
        LINUX_PPOLL => linux_sys_ppoll(arg1, arg2, arg3, arg4),
        LINUX_UNSHARE => -errno::EOPNOTSUPP,
        LINUX_SPLICE => linux_sys_splice(arg1 as i32, arg2, arg3 as i32, arg4, arg5),
        LINUX_TEE => linux_sys_splice(arg1 as i32, arg2, arg3 as i32, arg4, arg5),
        LINUX_SYNC_FILE_RANGE => linux_sys_fsync(arg1 as i32),
        LINUX_UTIMENSAT => linux_sys_utimensat(arg1 as i32, arg2, arg3, arg4 as i32),
        LINUX_EPOLL_PWAIT => {
            let _ = arg1;
            let _ = arg5;
            crate::linux_compat::epoll::epoll_wait(arg1 as i32, arg2, arg3 as i32, arg4 as i32)
        }
        LINUX_SIGNALFD => crate::linux_compat::signalfd::signalfd(arg1 as i32, arg2, arg3, 0),
        LINUX_SIGNALFD4 => {
            crate::linux_compat::signalfd::signalfd(arg1 as i32, arg2, arg3, arg4 as u32)
        }
        LINUX_TIMERFD_CREATE => {
            crate::linux_compat::timerfd::timerfd_create(arg1 as i32, arg2 as u32)
        }
        LINUX_TIMERFD_SETTIME => {
            crate::linux_compat::timerfd::timerfd_settime(arg1 as i32, arg2 as u32, arg3, arg4)
        }
        LINUX_TIMERFD_GETTIME => crate::linux_compat::timerfd::timerfd_gettime(arg1 as i32, arg2),
        LINUX_EVENTFD => crate::linux_compat::eventfd::eventfd(arg1 as u32, 0),
        LINUX_EVENTFD2 => crate::linux_compat::eventfd::eventfd(arg1 as u32, arg2 as u32),
        LINUX_FALLOCATE => linux_sys_fallocate(arg1 as i32, arg2 as i32, arg3, arg4),
        LINUX_ACCEPT4 => linux_sys_accept4(arg1 as i32, arg2, arg3, arg4 as i32),
        LINUX_EPOLL_CREATE1 => crate::linux_compat::epoll::epoll_create1(arg1 as u32),
        LINUX_DUP3 => linux_sys_dup3(arg1 as i32, arg2 as i32, arg3 as i32),
        LINUX_PIPE2 => linux_sys_pipe2(arg1, arg2 as i32),
        LINUX_PREADV => -errno::EOPNOTSUPP,
        LINUX_PWRITEV => -errno::EOPNOTSUPP,
        LINUX_PERF_EVENT_OPEN => -errno::EOPNOTSUPP,
        LINUX_RECVMMSG => -errno::EOPNOTSUPP,
        LINUX_FANOTIFY_INIT => -errno::EOPNOTSUPP,
        LINUX_FANOTIFY_MARK => -errno::EOPNOTSUPP,
        LINUX_PRLIMIT64 => linux_sys_prlimit64(arg1 as i32, arg2, arg3, arg4),
        LINUX_NAME_TO_HANDLE_AT => -errno::EOPNOTSUPP,
        LINUX_OPEN_BY_HANDLE_AT => -errno::EOPNOTSUPP,
        LINUX_CLOCK_ADJTIME => -errno::EOPNOTSUPP,
        LINUX_SYNCFS => 0,
        LINUX_SENDMMSG => -errno::EOPNOTSUPP,
        LINUX_SETNS => -errno::EOPNOTSUPP,
        LINUX_GETCPU => linux_sys_getcpu(arg1, arg2, arg3),
        LINUX_PROCESS_VM_READV => -errno::EOPNOTSUPP,
        LINUX_PROCESS_VM_WRITEV => -errno::EOPNOTSUPP,
        LINUX_KCMP => -errno::EOPNOTSUPP,
        LINUX_CREATE_MODULE => -errno::EPERM,
        LINUX_INIT_MODULE => linux_sys_init_module(arg1, arg2),
        LINUX_FINIT_MODULE => linux_sys_finit_module(arg1 as i32, arg2, arg3 as i32),
        LINUX_DELETE_MODULE => linux_sys_delete_module(arg1, arg2 as i32),
        LINUX_SCHED_SETATTR => linux_sys_sched_setattr(arg1 as i32, arg2, arg3 as u32),
        LINUX_SCHED_GETATTR => linux_sys_sched_getattr(arg1 as i32, arg2, arg3 as u32),
        LINUX_RENAMEAT2 => linux_sys_renameat2(arg1 as i32, arg2, arg3 as i32, arg4, arg5 as i32),
        LINUX_SECCOMP => -errno::EOPNOTSUPP,
        LINUX_GETRANDOM => linux_sys_getrandom(arg1, arg2, arg3 as i32),
        LINUX_MEMFD_CREATE => linux_sys_memfd_create(arg1, arg2 as u32),
        LINUX_KEXEC_FILE_LOAD => -errno::EPERM,
        LINUX_BPF => -errno::EOPNOTSUPP,
        LINUX_EXECVEAT => -errno::EOPNOTSUPP,
        LINUX_USERFAULTFD => -errno::EOPNOTSUPP,
        LINUX_MEMBARRIER => linux_sys_membarrier(arg1 as i32, arg2 as i32),
        LINUX_MLOCK2 => linux_sys_mlock(arg1, arg2),
        LINUX_COPY_FILE_RANGE => -errno::EOPNOTSUPP,
        LINUX_PREADV2 => -errno::EOPNOTSUPP,
        LINUX_PWRITEV2 => -errno::EOPNOTSUPP,
        LINUX_PKEY_MPROTECT => -errno::EOPNOTSUPP,
        LINUX_PKEY_ALLOC => -errno::EOPNOTSUPP,
        LINUX_PKEY_FREE => -errno::EOPNOTSUPP,
        LINUX_STATX => linux_sys_statx(arg1 as i32, arg2, arg3 as i32, arg4 as i32, arg5),
        LINUX_IO_PGETEVENTS => -errno::EOPNOTSUPP,
        LINUX_RSEQ => -errno::EOPNOTSUPP,
        LINUX_PIDFD_SEND_SIGNAL => -errno::EOPNOTSUPP,
        LINUX_IO_URING_SETUP => -errno::EOPNOTSUPP,
        LINUX_IO_URING_ENTER => -errno::EOPNOTSUPP,
        LINUX_IO_URING_REGISTER => -errno::EOPNOTSUPP,
        LINUX_OPEN_TREE => -errno::EOPNOTSUPP,
        LINUX_MOVE_MOUNT => -errno::EPERM,
        LINUX_FSOPEN => -errno::EOPNOTSUPP,
        LINUX_FSCONFIG => -errno::EOPNOTSUPP,
        LINUX_FSMOUNT => -errno::EOPNOTSUPP,
        LINUX_FSPICK => -errno::EOPNOTSUPP,
        LINUX_PIDFD_OPEN => -errno::EOPNOTSUPP,
        LINUX_CLONE3 => linux_sys_clone3(arg1, arg2),
        LINUX_CLOSE_RANGE => linux_sys_close_range(arg1 as u32, arg2 as u32, arg3 as i32),
        LINUX_OPENAT2 => -errno::EOPNOTSUPP,
        LINUX_PIDFD_GETFD => -errno::EOPNOTSUPP,
        LINUX_FACCESSAT2 => -errno::EOPNOTSUPP,
        LINUX_PROCESS_MADVISE => 0, // OK - madvise hint
        LINUX_EPOLL_PWAIT2 => {
            let _ = arg5;
            crate::linux_compat::epoll::epoll_wait(arg1 as i32, arg2, arg3 as i32, arg4 as i32)
        }
        LINUX_QUOTACTL_FD => -errno::EOPNOTSUPP,
        LINUX_LANDLOCK_CREATE_RULESET => -errno::EOPNOTSUPP,
        LINUX_LANDLOCK_ADD_RULE => -errno::EOPNOTSUPP,
        LINUX_LANDLOCK_RESTRICT_SELF => -errno::EOPNOTSUPP,
        LINUX_PROCESS_MRELEASE => -errno::EOPNOTSUPP,
        LINUX_FUTEX_WAITV => -errno::EOPNOTSUPP,
        LINUX_SET_MEMPOLICY_HOME_NODE => -errno::EOPNOTSUPP,
        LINUX_CACHESTAT => -errno::EOPNOTSUPP,
        LINUX_FCHMODAT2 => linux_sys_fchmodat(arg1 as i32, arg2, arg3 as i32, arg4 as i32),
        LINUX_MSEAL => -errno::EOPNOTSUPP,
        LINUX_SETXATTR => -errno::EOPNOTSUPP,
        LINUX_LSETXATTR => -errno::EOPNOTSUPP,
        LINUX_FSETXATTR => -errno::EOPNOTSUPP,
        LINUX_GETXATTR => -errno::EOPNOTSUPP,
        LINUX_LGETXATTR => -errno::EOPNOTSUPP,
        LINUX_FGETXATTR => -errno::EOPNOTSUPP,
        LINUX_LISTXATTR => -errno::EOPNOTSUPP,
        LINUX_LLISTXATTR => -errno::EOPNOTSUPP,
        LINUX_FLISTXATTR => -errno::EOPNOTSUPP,
        LINUX_REMOVEXATTR => -errno::EOPNOTSUPP,
        LINUX_LREMOVEXATTR => -errno::EOPNOTSUPP,
        LINUX_FREMOVEXATTR => -errno::EOPNOTSUPP,
        _ => -errno::EOPNOTSUPP,
    }
}

fn linux_sys_read(fd: i32, buf: u64, count: u64) -> i64 {
    if buf == 0 || count == 0 || count > 4096 {
        return 0;
    }
    if !crate::security::validate_user_buffer(buf, count as usize) {
        return -errno::EFAULT;
    }
    let slice = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, count as usize) };

    if crate::pipe::is_pipe_fd(fd) {
        match crate::pipe::pipe_read(fd, slice) {
            Ok(n) => n as i64,
            Err(_) => -errno::EIO,
        }
    } else if fd >= 0 && fd <= 2 {
        return 0;
    } else if crate::linux_compat::socketpair::is_socketpair_fd(fd) {
        return crate::linux_compat::socketpair::socketpair_read(fd, slice);
    } else if crate::linux_compat::eventfd::is_eventfd_fd(fd) {
        return crate::linux_compat::eventfd::eventfd_read(fd, slice);
    } else if crate::linux_compat::signalfd::is_signalfd_fd(fd) {
        return crate::linux_compat::signalfd::signalfd_read(fd, slice);
    } else if crate::linux_compat::timerfd::is_timerfd_fd(fd) {
        return crate::linux_compat::timerfd::timerfd_read(fd, slice);
    } else {
        crate::scheduler::with_current_task(|task| {
            let mut table = task.fd_table.lock();
            if let Some(handle) = table.get_mut(&fd) {
                if handle.node_type != crate::fs::NodeType::File {
                    return -errno::EISDIR;
                }
                match crate::fs::read(&handle.path) {
                    Ok(data) => {
                        let start = handle.pos;
                        if start >= data.len() {
                            return 0;
                        }
                        let end = (start + count as usize).min(data.len());
                        let len = end - start;
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                data.as_ptr().add(start),
                                slice.as_mut_ptr(),
                                len,
                            );
                        }
                        handle.pos = end;
                        len as i64
                    }
                    Err(e) => -errno::from_fs_error(&e),
                }
            } else {
                -errno::EBADF
            }
        })
        .unwrap_or(-errno::EBADF)
    }
}

fn linux_sys_write(fd: i32, buf: u64, count: u64) -> i64 {
    if buf == 0 || count == 0 || count > 4096 {
        return 0;
    }
    if !crate::security::validate_user_buffer(buf, count as usize) {
        return -errno::EFAULT;
    }
    let slice = unsafe { core::slice::from_raw_parts(buf as *const u8, count as usize) };

    if crate::pipe::is_pipe_fd(fd) {
        match crate::pipe::pipe_write(fd, slice) {
            Ok(n) => n as i64,
            Err(_) => -errno::EIO,
        }
    } else if crate::linux_compat::eventfd::is_eventfd_fd(fd) {
        return crate::linux_compat::eventfd::eventfd_write(fd, slice);
    } else if crate::linux_compat::socketpair::is_socketpair_fd(fd) {
        return crate::linux_compat::socketpair::socketpair_write(fd, slice);
    } else if fd == 1 || fd == 2 {
        for &byte in slice {
            let c = byte as char;
            crate::serial_print!("{}", c);
            crate::drivers::framebuffer::console::_print(format_args!("{}", c));
        }
        count as i64
    } else {
        crate::scheduler::with_current_task(|task| {
            let mut table = task.fd_table.lock();
            if let Some(handle) = table.get_mut(&fd) {
                if handle.node_type == crate::fs::NodeType::Directory {
                    return -errno::EISDIR;
                }
                if crate::fs::write(&handle.path, slice).is_ok() {
                    handle.pos += slice.len();
                    slice.len() as i64
                } else {
                    -errno::EIO
                }
            } else {
                -errno::EBADF
            }
        })
        .unwrap_or(-errno::EBADF)
    }
}

fn linux_sys_open(path_ptr: u64, flags: u64, _mode: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match crate::fs::path::resolve(&path).ok() {
        Some(p) => p,
        None => return -errno::ENOENT,
    };

    let is_creat = (flags & 0x40) != 0;
    let is_trunc = (flags & 0x200) != 0;
    let is_rdonly = (flags & 3) == 0;
    let is_wronly = (flags & 3) == 1;
    let is_rdwr = (flags & 3) == 2;

    if is_creat {
        let _ = crate::fs::touch(&abs_path);
    }

    match crate::fs::stat(&abs_path) {
        Ok(_) => {
            if is_trunc && (is_wronly || is_rdwr) {
                let _ = crate::fs::write(&abs_path, b"");
            }
        }
        Err(_) if !is_creat => return -errno::ENOENT,
        Err(_) => {}
    }

    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        let next_fd = (3..).find(|fd| !table.contains_key(fd)).unwrap_or(3);
        table.insert(
            next_fd,
            crate::fs::FileHandle {
                path: abs_path.clone(),
                pos: 0,
                node_type: crate::fs::NodeType::File,
            },
        );
        next_fd as i64
    })
    .unwrap_or(-errno::EMFILE)
}

fn linux_sys_close(fd: i32) -> i64 {
    if fd < 3 {
        return -errno::EINVAL;
    }
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        if table.remove(&fd).is_some() {
            0
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_stat(path_ptr: u64, stat_ptr: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    fill_linux_stat(&path, stat_ptr)
}

fn linux_sys_fstat(fd: i32, stat_ptr: u64) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if let Some(handle) = table.get(&fd) {
            fill_linux_stat(&handle.path, stat_ptr)
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_lseek(fd: i32, offset: u64, whence: i32) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        if let Some(handle) = table.get_mut(&fd) {
            match whence {
                0 => {
                    handle.pos = offset as usize;
                    handle.pos as i64
                }
                1 => {
                    handle.pos = handle.pos.wrapping_add(offset as usize);
                    handle.pos as i64
                }
                2 => {
                    if let Ok(data) = crate::fs::read(&handle.path) {
                        handle.pos = data.len().saturating_sub(offset as usize);
                        handle.pos as i64
                    } else {
                        -errno::EIO
                    }
                }
                _ => -errno::EINVAL,
            }
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_ioctl(fd: i32, request: u64, arg: u64) -> i64 {
    match request {
        0x5401 => {
            if fd <= 2 {
                if crate::security::validate_user_ptr(arg) {
                    unsafe {
                        core::ptr::write_bytes(arg as *mut u8, 0, 60);
                    }
                }
                0
            } else {
                -errno::ENOTTY
            }
        }
        0x5402 | 0x5403 | 0x5404 => 0,
        0x5413 => {
            if crate::security::validate_user_ptr(arg) {
                unsafe {
                    let ws = arg as *mut u16;
                    *ws = 80;
                    *ws.add(1) = 24;
                    *ws.add(2) = 0;
                    *ws.add(3) = 0;
                }
            }
            0
        }
        0x540F => {
            if crate::security::validate_user_ptr(arg) {
                unsafe {
                    *(arg as *mut i32) = crate::scheduler::current_task_id().unwrap_or(0) as i32;
                }
            }
            0
        }
        0x5410 => 0,
        0x5421 => 0,
        0x540E => 0,
        0x80045430 => 0,
        0x40045431 => 0,
        _ => -errno::ENOTTY,
    }
}

fn linux_sys_pread64(fd: i32, buf: u64, count: u64, offset: u64) -> i64 {
    if buf == 0 || count == 0 {
        return 0;
    }
    if !crate::security::validate_user_buffer(buf, count as usize) {
        return -errno::EFAULT;
    }
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if let Some(handle) = table.get(&fd) {
            match crate::fs::read(&handle.path) {
                Ok(data) => {
                    let start = offset as usize;
                    if start >= data.len() {
                        return 0;
                    }
                    let end = (start + count as usize).min(data.len());
                    let len = end - start;
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            data.as_ptr().add(start),
                            buf as *mut u8,
                            len,
                        );
                    }
                    len as i64
                }
                Err(e) => -errno::from_fs_error(&e),
            }
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_pwrite64(fd: i32, buf: u64, count: u64, offset: u64) -> i64 {
    if buf == 0 || count == 0 {
        return 0;
    }
    if !crate::security::validate_user_buffer(buf, count as usize) {
        return -errno::EFAULT;
    }
    let slice = unsafe { core::slice::from_raw_parts(buf as *const u8, count as usize) };
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        if let Some(handle) = table.get_mut(&fd) {
            if crate::fs::write(&handle.path, slice).is_ok() {
                count as i64
            } else {
                -errno::EIO
            }
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_readv(fd: i32, iov: u64, iovcnt: u64) -> i64 {
    if iovcnt > 1024 {
        return -errno::EINVAL;
    }
    let mut total = 0i64;
    for i in 0..iovcnt as isize {
        let iov_entry =
            unsafe { core::ptr::read_volatile((iov as isize + i * 16) as *const [u64; 2]) };
        let base = iov_entry[0];
        let len = iov_entry[1];
        if len == 0 {
            continue;
        }
        let ret = linux_sys_read(fd, base, len);
        if ret < 0 {
            return if total > 0 { total } else { ret };
        }
        total += ret;
        if (ret as u64) < len {
            break;
        }
    }
    total
}

fn linux_sys_writev(fd: i32, iov: u64, iovcnt: u64) -> i64 {
    if iovcnt > 1024 {
        return -errno::EINVAL;
    }
    let mut total = 0i64;
    for i in 0..iovcnt as isize {
        let iov_entry =
            unsafe { core::ptr::read_volatile((iov as isize + i * 16) as *const [u64; 2]) };
        let base = iov_entry[0];
        let len = iov_entry[1];
        if len == 0 {
            continue;
        }
        let ret = linux_sys_write(fd, base, len);
        if ret < 0 {
            return if total > 0 { total } else { ret };
        }
        total += ret;
    }
    total
}

fn linux_sys_pipe(pipe_ptr: u64) -> i64 {
    linux_sys_pipe2(pipe_ptr, 0)
}

fn linux_sys_pipe2(pipe_ptr: u64, _flags: i32) -> i64 {
    if pipe_ptr == 0 || !crate::security::validate_user_ptr(pipe_ptr) {
        return -errno::EFAULT;
    }
    match crate::pipe::create_pipe() {
        Ok((r, w)) => {
            unsafe {
                core::ptr::write_volatile(pipe_ptr as *mut i32, r);
                core::ptr::write_volatile((pipe_ptr + 4) as *mut i32, w);
            }
            0
        }
        Err(_) => -errno::EMFILE,
    }
}

fn linux_sys_dup(old_fd: i32) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        if !table.contains_key(&old_fd) {
            return -errno::EBADF;
        }
        let new_fd = (3..).find(|fd| !table.contains_key(fd)).unwrap_or(3);
        if let Some(handle) = table.get(&old_fd) {
            let handle = handle.clone();
            table.insert(new_fd, handle);
            new_fd as i64
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_dup2(old_fd: i32, new_fd: i32) -> i64 {
    if old_fd == new_fd {
        return old_fd as i64;
    }
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        if !table.contains_key(&old_fd) {
            return -errno::EBADF;
        }
        table.remove(&new_fd);
        if let Some(handle) = table.get(&old_fd) {
            let handle = handle.clone();
            table.insert(new_fd, handle);
            new_fd as i64
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_dup3(old_fd: i32, new_fd: i32, _flags: i32) -> i64 {
    linux_sys_dup2(old_fd, new_fd)
}

fn linux_sys_access(path_ptr: u64, _mode: i32) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match crate::fs::path::resolve(&path).ok() {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    if crate::fs::exists(&abs_path) {
        0
    } else {
        -errno::ENOENT
    }
}

static MMAP_REGIONS: spin::Mutex<alloc::vec::Vec<(u64, u64, u64)>> =
    spin::Mutex::new(alloc::vec::Vec::new());

fn mmap_get_unused_addr(length: u64) -> u64 {
    let regions = MMAP_REGIONS.lock();
    let mut candidate = crate::memory::user_layout::USER_HEAP_BASE + 0x1000000;
    let end =
        crate::memory::user_layout::USER_STACK_TOP - crate::memory::user_layout::USER_STACK_SIZE;
    'search: while candidate + length <= end {
        for &(start, size, _) in regions.iter() {
            if candidate < start + size && candidate + length > start {
                candidate = start + size + 0x1000;
                continue 'search;
            }
        }
        return candidate;
    }
    0
}

fn linux_sys_mmap(addr: u64, length: u64, prot: i32, flags: i32, fd: i32, _offset: u64) -> i64 {
    if length == 0 {
        return -errno::EINVAL;
    }
    let map_anonymous = (flags as u32 & 0x20) != 0;
    let map_private = (flags as u32 & 0x02) != 0;
    let map_fixed = (flags as u32 & 0x10) != 0;
    let map_shared = (flags as u32 & 0x01) != 0;
    let prot_write = (prot as u32 & 0x02) != 0;
    let prot_exec = (prot as u32 & 0x04) != 0;
    let prot_read = (prot as u32 & 0x01) != 0;

    let page_aligned_len = (length + 4095) & !4095;

    if map_fixed && addr != 0 {
        return addr as i64;
    }

    if map_anonymous || fd < 0 {
        let virt = if addr != 0 && !map_fixed {
            addr
        } else if addr != 0 && map_fixed {
            addr
        } else {
            let mut candidate = mmap_get_unused_addr(page_aligned_len);
            if candidate == 0 {
                return -errno::ENOMEM;
            }
            candidate
        };

        let page_flags = if prot_write {
            crate::memory::page_flags::USER_RW
        } else if prot_exec && prot_read {
            crate::memory::page_flags::USER_RX
        } else if prot_exec {
            crate::memory::page_flags::USER_RX
        } else if prot_read {
            crate::memory::page_flags::USER_RW & !crate::memory::page_flags::WRITABLE
        } else {
            crate::memory::page_flags::USER_RW & !crate::memory::page_flags::WRITABLE
        };

        crate::scheduler::with_current_task(|task| {
            if let Some(ref mut aspace) = task.address_space {
                let _ = aspace.map_range(virt, page_aligned_len, page_flags);
            }
        });

        MMAP_REGIONS.lock().push((
            virt,
            page_aligned_len,
            crate::scheduler::current_task_id().unwrap_or(0),
        ));
        return virt as i64;
    }

    if fd >= 0 && map_shared {
        crate::scheduler::with_current_task(|task| {
            let table = task.fd_table.lock();
            if let Some(handle) = table.get(&fd) {
                if let Ok(data) = crate::fs::read(&handle.path) {
                    let virt = addr;
                    if let Some(ref mut aspace) = task.address_space {
                        let _ = aspace.map_range(
                            virt,
                            page_aligned_len,
                            crate::memory::page_flags::USER_RW,
                        );
                        let copy_len = data.len().min(page_aligned_len as usize);
                        let _ = aspace.write_to(virt, &data[..copy_len]);
                    }
                }
            }
        });
        return addr as i64;
    }

    -errno::ENOMEM
}

fn linux_sys_munmap(addr: u64, length: u64) -> i64 {
    let pid = crate::scheduler::current_task_id().unwrap_or(0);
    let mut regions = MMAP_REGIONS.lock();
    regions.retain(|&(start, size, p)| {
        if p == pid && addr >= start && addr < start + size {
            let end = addr + length;
            if end <= start + size {
                return false;
            }
        }
        true
    });
    0
}

fn linux_sys_mprotect(_addr: u64, _len: u64, _prot: i32) -> i64 {
    0
}

fn linux_sys_brk(brk: u64) -> i64 {
    static mut PROGRAM_BREAK: u64 = 0x600000;
    unsafe {
        if brk == 0 {
            PROGRAM_BREAK as i64
        } else if brk > PROGRAM_BREAK {
            PROGRAM_BREAK = brk;
            brk as i64
        } else {
            PROGRAM_BREAK as i64
        }
    }
}

fn linux_sys_mremap(
    _old_addr: u64,
    _old_size: u64,
    _new_size: u64,
    _flags: i32,
    _new_addr: u64,
) -> i64 {
    -errno::ENOMEM
}

fn linux_sys_select(nfds: i32, readfds: u64, writefds: u64, exceptfds: u64, timeout: u64) -> i64 {
    if nfds <= 0 || nfds > 1024 {
        return 0;
    }
    let nfds = nfds as usize;

    let read_set = if readfds != 0 && crate::security::validate_user_buffer(readfds, 16) {
        unsafe { core::ptr::read_volatile(readfds as *const [u64; 16]) }
    } else {
        [0u64; 16]
    };
    let write_set = if writefds != 0 && crate::security::validate_user_buffer(writefds, 16) {
        unsafe { core::ptr::read_volatile(writefds as *const [u64; 16]) }
    } else {
        [0u64; 16]
    };
    let except_set = if exceptfds != 0 && crate::security::validate_user_buffer(exceptfds, 16) {
        unsafe { core::ptr::read_volatile(exceptfds as *const [u64; 16]) }
    } else {
        [0u64; 16]
    };

    loop {
        let mut result_read = [0u64; 16];
        let mut result_write = [0u64; 16];
        let mut result_except = [0u64; 16];
        let mut total = 0i64;

        for fd in 0..nfds {
            let bit = 1u64 << (fd % 64);
            let idx = fd / 64;

            let want_read = (readfds != 0) && (read_set[idx] & bit) != 0;
            let want_write = (writefds != 0) && (write_set[idx] & bit) != 0;
            let want_except = (exceptfds != 0) && (except_set[idx] & bit) != 0;

            if !want_read && !want_write && !want_except {
                continue;
            }

            let fd_i32 = fd as i32;
            let ready = check_fd_ready(fd_i32);

            if want_read && (ready & 1) != 0 {
                result_read[idx] |= bit;
                total += 1;
            }
            if want_write && (ready & 2) != 0 {
                result_write[idx] |= bit;
                total += 1;
            }
            if want_except && (ready & 4) != 0 {
                result_except[idx] |= bit;
                total += 1;
            }
        }

        if total > 0 {
            if readfds != 0 && crate::security::validate_user_buffer(readfds, 16) {
                unsafe {
                    core::ptr::write_volatile(readfds as *mut [u64; 16], result_read);
                }
            }
            if writefds != 0 && crate::security::validate_user_buffer(writefds, 16) {
                unsafe {
                    core::ptr::write_volatile(writefds as *mut [u64; 16], result_write);
                }
            }
            if exceptfds != 0 && crate::security::validate_user_buffer(exceptfds, 16) {
                unsafe {
                    core::ptr::write_volatile(exceptfds as *mut [u64; 16], result_except);
                }
            }
            return total;
        }

        // Parse timeout: tv_sec(8) + tv_usec(8) = 16 bytes
        if timeout != 0 && crate::security::validate_user_ptr(timeout) {
            let tv: [u64; 2] = unsafe { core::ptr::read_volatile(timeout as *const [u64; 2]) };
            if tv[0] == 0 && tv[1] == 0 {
                return 0; // timeout=0 → return immediately
            }
        } else if timeout == 0 {
            return 0; // no timeout → return immediately
        }
        // timeout > 0 or NULL (block forever): yield and retry
        crate::scheduler::yield_now();
    }
}

fn check_fd_ready(fd: i32) -> i32 {
    // Returns bitmask: bit0=readable, bit1=writable, bit2=error
    if fd < 0 {
        return 0;
    }
    // stdin: always readable (but returns 0 bytes)
    if fd >= 0 && fd <= 2 {
        return 3; // readable + writable
    }
    // Pipe fds
    if crate::pipe::is_pipe_fd(fd) {
        return 3; // assume always writable, readable check simplified
    }
    // Socket fds (>= 10000, < 30000)
    if fd >= 10000 && fd < 30000 {
        return crate::linux_compat::socket::socket_check_ready(fd);
    }
    // socketpair fds (40000+)
    if crate::linux_compat::socketpair::is_socketpair_fd(fd) {
        return crate::linux_compat::socketpair::socketpair_ready(fd);
    }
    // eventfd fds (30000+)
    if crate::linux_compat::eventfd::is_eventfd_fd(fd) {
        return crate::linux_compat::eventfd::eventfd_ready(fd);
    }
    // signalfd fds (31000+)
    if crate::linux_compat::signalfd::is_signalfd_fd(fd) {
        return crate::linux_compat::signalfd::signalfd_ready(fd);
    }
    // timerfd fds (32000+)
    if crate::linux_compat::timerfd::is_timerfd_fd(fd) {
        return crate::linux_compat::timerfd::timerfd_ready(fd);
    }
    // File fds from task fd_table
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if table.contains_key(&fd) {
            3 // readable + writable
        } else {
            0
        }
    })
    .unwrap_or(0)
}

fn linux_sys_poll(fds: u64, nfds: u64, timeout_msecs: u64) -> i64 {
    if fds == 0 || nfds == 0 {
        return 0;
    }
    let nfds = nfds.min(1024) as usize;
    if !crate::security::validate_user_buffer(fds, nfds * 8) {
        return -errno::EFAULT;
    }

    const POLLIN: i16 = 0x001;
    const POLLOUT: i16 = 0x004;
    const POLLERR: i16 = 0x008;
    const POLLHUP: i16 = 0x010;
    const POLLNVAL: i16 = 0x020;

    #[repr(C)]
    struct pollfd {
        fd: i32,
        events: i16,
        revents: i16,
    }

    loop {
        let mut ready_count = 0i64;
        for i in 0..nfds {
            let entry: pollfd =
                unsafe { core::ptr::read_volatile((fds + (i as u64 * 8)) as *const pollfd) };
            if entry.fd < 0 {
                continue;
            }
            let mut revents: i16 = 0;
            let fd_ready = check_fd_ready(entry.fd);
            if (entry.events & POLLIN) != 0 && (fd_ready & 1) != 0 {
                revents |= POLLIN;
            }
            if (entry.events & POLLOUT) != 0 && (fd_ready & 2) != 0 {
                revents |= POLLOUT;
            }
            if entry.fd >= 10000 && entry.fd < 30000 {
                let table = crate::linux_compat::socket::SOCKET_TABLE.lock();
                let idx = (entry.fd - 10000) as usize;
                if table.get(idx).and_then(|s| s.as_ref()).is_none() {
                    revents |= POLLNVAL;
                }
            } else if crate::linux_compat::eventfd::is_eventfd_fd(entry.fd)
                || crate::linux_compat::signalfd::is_signalfd_fd(entry.fd)
                || crate::linux_compat::timerfd::is_timerfd_fd(entry.fd)
                || crate::linux_compat::epoll::is_epoll_fd(entry.fd)
                || crate::linux_compat::socketpair::is_socketpair_fd(entry.fd)
            {
                // new fd type, always valid if present; check_fd_ready handles it
            } else {
                let valid = crate::scheduler::with_current_task(|task| {
                    task.fd_table.lock().contains_key(&entry.fd)
                })
                .unwrap_or(false);
                if !valid && !(entry.fd >= 0 && entry.fd <= 2) && !crate::pipe::is_pipe_fd(entry.fd)
                {
                    revents |= POLLNVAL;
                }
            }
            if revents != 0 {
                ready_count += 1;
            }
            let updated = pollfd {
                fd: entry.fd,
                events: entry.events,
                revents,
            };
            unsafe {
                core::ptr::write_volatile((fds + (i as u64 * 8)) as *mut pollfd, updated);
            }
        }
        if ready_count > 0 {
            return ready_count;
        }
        if timeout_msecs == 0 {
            return 0;
        }
        crate::scheduler::yield_now();
    }
}

fn linux_sys_fcntl(fd: i32, cmd: u64, arg: u64) -> i64 {
    match cmd as i32 {
        0 => linux_sys_dup(fd),
        1 => linux_sys_dup2(fd, arg as i32),
        2 => 0,
        3 => 0,
        4 => 0,
        5 => 0,
        6 => linux_sys_dup2(fd, arg as i32),
        _ => -errno::EINVAL,
    }
}

fn linux_sys_fsync(fd: i32) -> i64 {
    if fd < 0 {
        return -errno::EBADF;
    }
    0
}

fn linux_sys_truncate(path_ptr: u64, _length: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match crate::fs::path::resolve(&path).ok() {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    if crate::fs::exists(&abs_path) {
        let _ = crate::fs::write(&abs_path, b"");
        0
    } else {
        -errno::ENOENT
    }
}

fn linux_sys_ftruncate(fd: i32, _length: u64) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if table.contains_key(&fd) {
            0
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_getdents64(fd: i32, buf: u64, count: u64) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if let Some(handle) = table.get(&fd) {
            let dir_path = if crate::fs::is_dir(&handle.path) {
                handle.path.clone()
            } else {
                let parent = crate::fs::path::parent(&handle.path).unwrap_or_else(|| "/".into());
                parent
            };
            match crate::fs::readdir(&dir_path) {
                Ok(entries) => {
                    let mut written = 0u64;
                    let base = buf;
                    let max = count;

                    for entry in &entries {
                        let name_bytes = entry.name.as_bytes();
                        let reclen = ((24 + name_bytes.len() + 1 + 7) & !7) as u64;
                        if written + reclen > max {
                            break;
                        }
                        unsafe {
                            let pos = (base + written) as *mut u8;
                            core::ptr::write_volatile(pos as *mut u64, entry.size);
                            core::ptr::write_volatile((pos.add(8)) as *mut u64, reclen);
                            core::ptr::write_volatile(
                                (pos.add(16)) as *mut u8,
                                entry.node_type as u8 & 0xF,
                            );
                            core::ptr::write_volatile((pos.add(17)) as *mut u8, 0);
                            core::ptr::copy_nonoverlapping(
                                name_bytes.as_ptr(),
                                pos.add(19),
                                name_bytes.len(),
                            );
                            core::ptr::write_volatile(pos.add(19 + name_bytes.len()), 0u8);
                        }
                        written += reclen;
                    }
                    written as i64
                }
                Err(e) => -errno::from_fs_error(&e),
            }
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_getcwd(buf: u64, size: u64) -> i64 {
    let cwd = crate::fs::cwd();
    let cwd_bytes = cwd.as_bytes();
    let len = cwd_bytes.len() + 1;
    if len > size as usize {
        return -errno::ERANGE;
    }
    if !crate::security::validate_user_buffer(buf, len) {
        return -errno::EFAULT;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(cwd_bytes.as_ptr(), buf as *mut u8, cwd_bytes.len());
        core::ptr::write_volatile((buf + cwd_bytes.len() as u64) as *mut u8, 0u8);
    }
    len as i64
}

fn linux_sys_chdir(path_ptr: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    match crate::fs::chdir(&path) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_mkdir(path_ptr: u64, _mode: i32) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match crate::fs::path::resolve(&path).ok() {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    match crate::fs::mkdir(&abs_path) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_rmdir(path_ptr: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match crate::fs::path::resolve(&path).ok() {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    match crate::fs::rmdir(&abs_path) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_unlink(path_ptr: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match crate::fs::path::resolve(&path).ok() {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    if crate::fs::is_dir(&abs_path) {
        return -errno::EISDIR;
    }
    match crate::fs::rm(&abs_path) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_link(old_ptr: u64, new_ptr: u64) -> i64 {
    let old_path = match read_user_string(old_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let new_path = match read_user_string(new_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    match crate::fs::link(&old_path, &new_path) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_symlink(target_ptr: u64, link_ptr: u64) -> i64 {
    let target = match read_user_string(target_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let link_path = match read_user_string(link_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    match crate::fs::symlink(&target, &link_path) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_readlink(path_ptr: u64, buf: u64, size: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match crate::fs::path::resolve(&path).ok() {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    if !crate::fs::exists(&abs_path) {
        return -errno::ENOENT;
    }
    if size == 0 || !crate::security::validate_user_buffer(buf, size as usize) {
        return -errno::EFAULT;
    }
    // Read the symlink target from the filesystem node's content
    match crate::fs::read(&abs_path) {
        Ok(data) => {
            let len = size.min(data.len() as u64) as usize;
            unsafe {
                core::ptr::copy_nonoverlapping(data.as_ptr(), buf as *mut u8, len);
            }
            len as i64
        }
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_chmod(path_ptr: u64, _mode: i32) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if crate::fs::exists(&path) {
        0
    } else {
        -errno::ENOENT
    }
}

fn linux_sys_rename(old_ptr: u64, new_ptr: u64) -> i64 {
    let old_path = match read_user_string(old_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let new_path = match read_user_string(new_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    match crate::fs::mv(&old_path, &new_path) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_umask(mask: i32) -> i64 {
    let old = 0o022i64;
    let _ = mask;
    old
}

fn linux_sys_exit(status: i32) -> i64 {
    crate::security::audit_log(
        crate::security::AuditSeverity::Info,
        &alloc::format!(
            "Linux process {} exited with status {}",
            crate::scheduler::current_task_id().unwrap_or(0),
            status
        ),
    );
    // Store exit code before exiting
    crate::scheduler::with_current_task(|task| {
        task.exit_code = status;
    });
    crate::scheduler::exit_current();
    0
}

fn linux_sys_getppid() -> i64 {
    crate::scheduler::with_current_task(|task| task.parent_id.unwrap_or(0) as i64).unwrap_or(0)
}

fn linux_sys_wait4(pid: i64, wstatus: u64, options: i32, _rusage: u64) -> i64 {
    let current_pid = crate::scheduler::current_task_id().unwrap_or(0);
    let wnohang = (options & 1) != 0; // WNOHANG = 1

    loop {
        let found = crate::scheduler::collect_zombie(
            if pid == -1 {
                None
            } else if pid > 0 {
                Some(pid as u64)
            } else {
                return -errno::EINVAL;
            },
            current_pid,
        );

        if let Some((child_pid, exit_code)) = found {
            if wstatus != 0 {
                if crate::security::validate_user_ptr(wstatus) {
                    let status = ((exit_code & 0xff) << 8) as i32;
                    unsafe { *(wstatus as *mut i32) = status };
                }
            }
            return child_pid as i64;
        }

        if wnohang {
            return 0;
        }

        // No zombie yet, yield and retry
        crate::scheduler::yield_now();
        crate::curr_arch::halt();
    }
}

fn linux_sys_getpgrp() -> i64 {
    crate::scheduler::with_current_task(|t| t.pgid as i64).unwrap_or(0)
}

fn linux_sys_setsid() -> i64 {
    crate::scheduler::with_current_task(|t| {
        // Check if we are already a session leader (simple check: sid == pgid == own pid)
        if t.sid == t.id as u64 && t.pgid == t.id as u64 {
            return -errno::EPERM;
        }
        // Cannot setsid if we are already in a session group (pgid differs from pid)
        if t.pgid != t.id as u64 {
            return -errno::EPERM;
        }
        t.sid = t.id as u64;
        t.pgid = t.id as u64;
        t.id as i64
    })
    .unwrap_or(-errno::EPERM)
}

fn linux_sys_setpgid(pid: i32, pgid: i32) -> i64 {
    let target_pid = if pid == 0 {
        crate::scheduler::current_task_id().unwrap_or(0)
    } else {
        pid as u64
    };
    let new_pgid = if pgid == 0 { target_pid } else { pgid as u64 };
    // For now, simple implementation: set pgid on target task
    // In a full implementation, we'd need to find the task by pid and verify it's in the same session
    let current = crate::scheduler::current_task_id().unwrap_or(0);
    if target_pid != current {
        // We could search for the target task, but for now only allow setting own pgid
        return -errno::ESRCH;
    }
    crate::scheduler::with_current_task(|t| {
        t.pgid = new_pgid;
        0i64
    })
    .unwrap_or(-errno::ESRCH)
}

fn linux_sys_nanosleep(req_ptr: u64, rem_ptr: u64) -> i64 {
    if req_ptr == 0 {
        return -errno::EFAULT;
    }
    if !crate::security::validate_user_ptr(req_ptr) {
        return -errno::EFAULT;
    }
    let ts: [u64; 2] = unsafe { core::ptr::read_volatile(req_ptr as *const [u64; 2]) };
    let sec = ts[0];
    let nsec = ts[1];
    let ms = sec.saturating_mul(1000).saturating_add(nsec / 1_000_000);
    if ms > 0 {
        let ticks = (ms / 55).max(1);
        let start = crate::curr_arch::get_ticks();
        while crate::curr_arch::get_ticks() - start < ticks {
            crate::scheduler::yield_now();
        }
    }
    if rem_ptr != 0 {
        unsafe {
            core::ptr::write_volatile(rem_ptr as *mut [u64; 2], [0, 0]);
        }
    }
    0
}

fn linux_sys_clock_gettime(clock_id: i32, tp_ptr: u64) -> i64 {
    if tp_ptr == 0 || !crate::security::validate_user_ptr(tp_ptr) {
        return -errno::EFAULT;
    }
    let ticks = crate::curr_arch::get_ticks();
    let seconds = (ticks / 18) as u64;
    let nanos = 0u64;
    unsafe {
        core::ptr::write_volatile(tp_ptr as *mut [u64; 2], [seconds, nanos]);
    }
    0
}

fn linux_sys_gettimeofday(tv_ptr: u64, _tz_ptr: u64) -> i64 {
    if tv_ptr == 0 {
        return 0;
    }
    if !crate::security::validate_user_ptr(tv_ptr) {
        return -errno::EFAULT;
    }
    let ticks = crate::curr_arch::get_ticks();
    let seconds = (ticks / 18) as u64;
    let usecs = 0u64;
    unsafe {
        core::ptr::write_volatile(tv_ptr as *mut [u64; 2], [seconds, usecs]);
    }
    0
}

fn linux_sys_time(t_ptr: u64) -> i64 {
    let ticks = crate::curr_arch::get_ticks();
    let seconds = (ticks / 18) as i64;
    if t_ptr != 0 {
        if crate::security::validate_user_ptr(t_ptr) {
            unsafe {
                core::ptr::write_volatile(t_ptr as *mut i64, seconds);
            }
        }
    }
    seconds
}

fn linux_sys_uname(buf: u64) -> i64 {
    if buf == 0 {
        return -errno::EFAULT;
    }
    if !crate::security::validate_user_ptr(buf) {
        return -errno::EFAULT;
    }
    let hostname = crate::config::get_hostname();
    let hn_bytes = hostname.as_bytes();
    let len = hn_bytes.len().min(64);
    let mut utsname = [0u8; 390];
    let sysname = b"Linux\0";
    let release = b"0.1.0-mesaos\0";
    let version = b"#1 SMP Tue Jun 9 00:00:00 UTC 2026\0";
    let machine = b"x86_64\0";
    let nodename = hn_bytes;

    let mut off = 0usize;
    utsname[off..off + sysname.len()].copy_from_slice(sysname);
    off += 65;
    utsname[off..off + len.min(64)].copy_from_slice(&nodename[..len.min(64)]);
    utsname[off + len.min(64)] = 0;
    off += 65;
    utsname[off..off + release.len()].copy_from_slice(release);
    off += 65;
    utsname[off..off + version.len()].copy_from_slice(version);
    off += 65;
    utsname[off..off + machine.len()].copy_from_slice(machine);
    off += 65;
    utsname[off] = 0;

    unsafe {
        core::ptr::copy_nonoverlapping(utsname.as_ptr(), buf as *mut u8, 390);
    }
    0
}

fn linux_sys_sysinfo(info_ptr: u64) -> i64 {
    if info_ptr == 0 || !crate::security::validate_user_ptr(info_ptr) {
        return -errno::EFAULT;
    }
    let ticks = crate::curr_arch::get_ticks();
    let uptime_sec = (ticks / 18) as i64;
    let (free, total) = crate::memory::pmm::stats();
    let page_size = crate::memory::PAGE_SIZE as u64;
    let totalram = (total * page_size) as u64;
    let freeram = (free * page_size) as u64;
    let procs = crate::scheduler::task_count() as u16;

    #[repr(C)]
    struct linux_sysinfo {
        uptime: i64,
        loads: [u64; 3],
        totalram: u64,
        freeram: u64,
        sharedram: u64,
        bufferram: u64,
        totalswap: u64,
        freeswap: u64,
        procs: u16,
        pad: u16,
        totalhigh: u64,
        freehigh: u64,
        mem_unit: u32,
        _f: [u8; 20],
    }

    let info = linux_sysinfo {
        uptime: uptime_sec,
        loads: [0, 0, 0],
        totalram,
        freeram,
        sharedram: 0,
        bufferram: 0,
        totalswap: 0,
        freeswap: 0,
        procs,
        pad: 0,
        totalhigh: 0,
        freehigh: 0,
        mem_unit: 1,
        _f: [0u8; 20],
    };

    unsafe {
        core::ptr::write_volatile(info_ptr as *mut linux_sysinfo, info);
    }
    0
}

fn linux_sys_getrlimit(resource: i32, rlim_ptr: u64) -> i64 {
    if rlim_ptr == 0 || !crate::security::validate_user_ptr(rlim_ptr) {
        return -errno::EFAULT;
    }
    let inf = u64::MAX;
    #[repr(C)]
    struct rlimit64 {
        rlim_cur: u64,
        rlim_max: u64,
    }
    let lim = rlimit64 {
        rlim_cur: inf,
        rlim_max: inf,
    };
    unsafe {
        core::ptr::write_volatile(rlim_ptr as *mut rlimit64, lim);
    }
    0
}

fn linux_sys_setrlimit(_resource: i32, _rlim_ptr: u64) -> i64 {
    0
}

fn linux_sys_getrusage(_who: i32, _usage_ptr: u64) -> i64 {
    0
}

fn linux_sys_prlimit64(_pid: i32, _resource: u64, _new_rlim: u64, _old_rlim: u64) -> i64 {
    0
}

fn linux_sys_prctl(_option: i32, _arg2: u64, _arg3: u64, _arg4: u64, _arg5: u64) -> i64 {
    0
}

fn linux_sys_getcpu(cpu_ptr: u64, node_ptr: u64, _tcache: u64) -> i64 {
    if cpu_ptr != 0 && crate::security::validate_user_ptr(cpu_ptr) {
        unsafe {
            core::ptr::write_volatile(cpu_ptr as *mut u32, 0u32);
        }
    }
    if node_ptr != 0 && crate::security::validate_user_ptr(node_ptr) {
        unsafe {
            core::ptr::write_volatile(node_ptr as *mut u32, 0u32);
        }
    }
    0
}

fn linux_sys_getrandom(buf: u64, count: u64, _flags: i32) -> i64 {
    if count == 0 {
        return 0;
    }
    if !crate::security::validate_user_buffer(buf, count as usize) {
        return -errno::EFAULT;
    }
    let count = count.min(256);
    unsafe {
        for i in 0..count {
            let byte = (crate::curr_arch::get_ticks() ^ (i.wrapping_mul(0x9e3779b97f4a7c15))) as u8;
            core::ptr::write_volatile((buf + i) as *mut u8, byte);
        }
    }
    count as i64
}

fn linux_sys_bind(fd: i32, addr_ptr: u64, _addrlen: u64) -> i64 {
    if !crate::security::validate_user_ptr(addr_ptr) {
        return -errno::EFAULT;
    }
    // sockaddr_in = family(u16) + port(u16) + addr(u32) + zero(u64) = 16 bytes
    let buf = unsafe { core::slice::from_raw_parts(addr_ptr as *const u8, 16) };
    let family = u16::from_ne_bytes([buf[0], buf[1]]);
    if family != 2 {
        // AF_INET
        return -errno::EAFNOSUPPORT;
    }
    let port = u16::from_be_bytes([buf[2], buf[3]]);
    crate::linux_compat::socket::socket_bind(fd, port)
}

fn linux_sys_sendto(fd: i32, buf: u64, len: u64, _flags: i32, dest_addr: u64) -> i64 {
    let len_u = len as usize;
    if !crate::security::validate_user_buffer(buf, len_u) {
        return -errno::EFAULT;
    }
    if len_u > 65535 {
        return -errno::EMSGSIZE;
    }
    let data = unsafe { core::slice::from_raw_parts(buf as *const u8, len_u) };

    // Parse sockaddr_in from dest_addr
    if !crate::security::validate_user_ptr(dest_addr) {
        return -errno::EFAULT;
    }
    let addr_buf = unsafe { core::slice::from_raw_parts(dest_addr as *const u8, 16) };
    let family = u16::from_ne_bytes([addr_buf[0], addr_buf[1]]);
    if family != 2 {
        return -errno::EAFNOSUPPORT;
    }
    let dest_port = u16::from_be_bytes([addr_buf[2], addr_buf[3]]);
    let dest_ip = u32::from_be_bytes([addr_buf[4], addr_buf[5], addr_buf[6], addr_buf[7]]);

    crate::linux_compat::socket::socket_sendto(fd, data, dest_ip, dest_port)
}

fn linux_sys_recvfrom(fd: i32, buf: u64, len: u64, _flags: i32, _src_addr: u64) -> i64 {
    let len_u = len as usize;
    if !crate::security::validate_user_buffer(buf, len_u) || len_u == 0 {
        return -errno::EFAULT;
    }
    let slice = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, len_u) };
    crate::linux_compat::socket::socket_recvfrom(fd, slice)
}

fn linux_sys_rt_sigreturn() -> i64 {
    -errno::EINTR
}

fn linux_sys_execve(path_ptr: u64, _argv: u64, _envp: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let code = match crate::fs::read(&path) {
        Ok(c) => c,
        Err(_) => return -errno::ENOENT,
    };
    if code.len() < 4 || code[0..4] != [0x7f, b'E', b'L', b'F'] {
        return -errno::ENOEXEC;
    }
    let name = alloc::format!("{}", path.split('/').last().unwrap_or(&path));

    // Create new address space and load ELF
    let mut new_as = match crate::memory::AddressSpace::new() {
        Ok(a) => a,
        Err(_) => return -errno::ENOMEM,
    };
    let (entry, user_stack_top) = match crate::elf::load_elf(&mut new_as, &code) {
        Ok(r) => r,
        Err(_) => return -errno::ENOEXEC,
    };

    // Replace current task's image
    let new_cr3 = new_as.cr3();
    crate::scheduler::with_current_task(|task| {
        let _ = task.address_space.replace(new_as);
        task.context.set_page_table(new_cr3);
        task.user_entry = entry;
        task.user_stack = user_stack_top;
        task.is_linux = true;
    });

    // Switch page tables immediately
    unsafe {
        crate::arch::x86_64::context::switch_cr3(new_cr3);
    }

    // Modify kernel stack to return to new entry point
    let kstack = crate::smp::syscall_kstack();
    if kstack != 0 {
        unsafe {
            // Saved registers on kernel stack (from bottom to top):
            // rsp+0 = r15, +8 = r14, +16 = r13, +24 = r12, +32 = rbx, +40 = rbp, +48 = r11, +56 = rcx
            let saved_rcx = (kstack.wrapping_sub(64)) as *mut u64;
            *saved_rcx = entry;
            // Set user RSP to new stack top
            crate::smp::set_usr_rsp(user_stack_top);
        }
    }

    0
}

fn linux_sys_futex(_uaddr: u64, _op: i32, _val: i32, _timeout: u64, _uaddr2: u64) -> i64 {
    match _op & 0xF {
        0 => {
            crate::scheduler::yield_now();
            0
        }
        1 => 0,
        _ => -errno::EOPNOTSUPP,
    }
}

fn linux_sys_openat(dirfd: i32, path_ptr: u64, flags: i32, _mode: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = if path.starts_with('/') {
        match crate::fs::path::resolve(&path).ok() {
            Some(p) => p,
            None => return -errno::ENOENT,
        }
    } else {
        let resolved = crate::scheduler::with_current_task(|task| {
            let table = task.fd_table.lock();
            if let Some(handle) = table.get(&dirfd) {
                let base = handle.path.trim_end_matches('/');
                crate::fs::path::resolve(&format!("{}/{}", base, path)).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| crate::fs::path::resolve(&path).ok());
        match resolved {
            Some(p) => p,
            None => return -errno::ENOENT,
        }
    };
    linux_sys_open_simple(&abs_path, flags)
}

fn linux_sys_mkdirat(dirfd: i32, path_ptr: u64, mode: i32) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match resolve_at(dirfd, &path) {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    match crate::fs::mkdir(&abs_path) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_unlinkat(dirfd: i32, path_ptr: u64, _flags: i32) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match resolve_at(dirfd, &path) {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    if crate::fs::is_dir(&abs_path) {
        match crate::fs::rmdir(&abs_path) {
            Ok(_) => 0,
            Err(e) => -errno::from_fs_error(&e),
        }
    } else {
        match crate::fs::rm(&abs_path) {
            Ok(_) => 0,
            Err(e) => -errno::from_fs_error(&e),
        }
    }
}

fn linux_sys_newfstatat(dirfd: i32, path_ptr: u64, stat_ptr: u64, _flags: i32) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match resolve_at(dirfd, &path) {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    fill_linux_stat(&abs_path, stat_ptr)
}

fn linux_sys_readlinkat(dirfd: i32, path_ptr: u64, buf: u64, size: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let abs_path = match resolve_at(dirfd, &path) {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    if !crate::fs::exists(&abs_path) {
        return -errno::ENOENT;
    }
    if size == 0 || !crate::security::validate_user_buffer(buf, size as usize) {
        return -errno::EFAULT;
    }
    let data = abs_path.as_bytes();
    let len = size.min(data.len() as u64) as usize;
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), buf as *mut u8, len);
    }
    len as i64
}

fn linux_sys_statx(_dirfd: i32, path_ptr: u64, _flags: i32, _mask: i32, statx_ptr: u64) -> i64 {
    if statx_ptr == 0 || path_ptr == 0 {
        return -errno::EFAULT;
    }
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    match crate::fs::stat(&path) {
        Ok(info) => {
            unsafe {
                let ptr = statx_ptr as *mut u8;
                *(ptr.add(0x00) as *mut u32) = 0x7FF;
                *(ptr.add(0x06) as *mut u16) = match info.node_type {
                    crate::fs::NodeType::File => 0o100_644u16,
                    crate::fs::NodeType::Directory => 0o40_755u16,
                    _ => 0,
                };
                *(ptr.add(0x08) as *mut u64) = info.size;
                *(ptr.add(0x10) as *mut u64) = 0;
                *(ptr.add(0x1C) as *mut u32) = info.owner_uid;
                *(ptr.add(0x20) as *mut u32) = info.owner_gid;
                *(ptr.add(0x24) as *mut u32) = 0;
            }
            0
        }
        Err(_) => -errno::ENOENT,
    }
}

fn linux_sys_statfs(path_ptr: u64, buf: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if buf == 0 || !crate::security::validate_user_ptr(buf) {
        return -errno::EFAULT;
    }
    if !crate::fs::exists(&path) {
        return -errno::ENOENT;
    }
    let (free_pages, total_pages) = crate::memory::pmm::stats();
    let page_size = crate::memory::PAGE_SIZE as u64;
    let block_size: u64 = 4096;
    let total_blocks = total_pages * page_size / block_size;
    let free_blocks = free_pages * page_size / block_size;

    #[repr(C)]
    struct linux_statfs64 {
        f_type: u64,
        f_bsize: u64,
        f_blocks: u64,
        f_bfree: u64,
        f_bavail: u64,
        f_files: u64,
        f_ffree: u64,
        f_fsid: u64,
        f_namelen: u64,
        f_frsize: u64,
        f_flags: u64,
        f_spare: [u64; 4],
    }
    let st = linux_statfs64 {
        f_type: 0x01021994, // TMPFS_MAGIC
        f_bsize: block_size,
        f_blocks: total_blocks,
        f_bfree: free_blocks,
        f_bavail: free_blocks,
        f_files: 0,
        f_ffree: 0,
        f_fsid: 0,
        f_namelen: 255,
        f_frsize: block_size,
        f_flags: 0,
        f_spare: [0; 4],
    };
    unsafe {
        core::ptr::write_volatile(buf as *mut linux_statfs64, st);
    }
    0
}

fn linux_sys_fstatfs(fd: i32, buf: u64) -> i64 {
    if buf == 0 || !crate::security::validate_user_ptr(buf) {
        return -errno::EFAULT;
    }
    let (free_pages, total_pages) = crate::memory::pmm::stats();
    let page_size = crate::memory::PAGE_SIZE as u64;
    let block_size: u64 = 4096;
    let total_blocks = total_pages * page_size / block_size;
    let free_blocks = free_pages * page_size / block_size;

    #[repr(C)]
    struct linux_statfs64 {
        f_type: u64,
        f_bsize: u64,
        f_blocks: u64,
        f_bfree: u64,
        f_bavail: u64,
        f_files: u64,
        f_ffree: u64,
        f_fsid: u64,
        f_namelen: u64,
        f_frsize: u64,
        f_flags: u64,
        f_spare: [u64; 4],
    }
    let st = linux_statfs64 {
        f_type: 0x01021994,
        f_bsize: block_size,
        f_blocks: total_blocks,
        f_bfree: free_blocks,
        f_bavail: free_blocks,
        f_files: 0,
        f_ffree: 0,
        f_fsid: 0,
        f_namelen: 255,
        f_frsize: block_size,
        f_flags: 0,
        f_spare: [0; 4],
    };
    unsafe {
        core::ptr::write_volatile(buf as *mut linux_statfs64, st);
    }
    0
}

fn linux_sys_sendfile(out_fd: i32, in_fd: i32, offset_ptr: u64, count: u64) -> i64 {
    if count == 0 {
        return 0;
    }
    let in_data = crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        table
            .get(&in_fd)
            .and_then(|handle| crate::fs::read(&handle.path).ok())
    })
    .flatten()
    .unwrap_or_default();

    if in_data.is_empty() {
        return 0;
    }

    let start = if offset_ptr != 0 && crate::security::validate_user_ptr(offset_ptr) {
        let off = unsafe { core::ptr::read_volatile(offset_ptr as *const i64) };
        off as usize
    } else {
        0
    };

    let actual_count = count.min((in_data.len().saturating_sub(start)) as u64) as usize;
    if actual_count == 0 {
        return 0;
    }

    let slice = &in_data[start..start + actual_count];
    // Write to out_fd directly (kernel-to-kernel, no user buffer check needed)
    let result = if crate::pipe::is_pipe_fd(out_fd) {
        crate::pipe::pipe_write(out_fd, slice)
            .map(|n| n as i64)
            .unwrap_or(-errno::EIO)
    } else if out_fd == 1 || out_fd == 2 {
        for &byte in slice {
            let c = byte as char;
            crate::serial_print!("{}", c);
            crate::drivers::framebuffer::console::_print(format_args!("{}", c));
        }
        actual_count as i64
    } else {
        crate::scheduler::with_current_task(|task| {
            let table = task.fd_table.lock();
            if let Some(handle) = table.get(&out_fd) {
                if crate::fs::write(&handle.path, slice).is_ok() {
                    actual_count as i64
                } else {
                    -errno::EIO
                }
            } else {
                -errno::EBADF
            }
        })
        .unwrap_or(-errno::EBADF)
    };

    if result > 0 {
        // Update offset if pointer was provided
        if offset_ptr != 0 && crate::security::validate_user_ptr(offset_ptr) {
            unsafe {
                core::ptr::write_volatile(offset_ptr as *mut i64, (start + actual_count) as i64);
            }
        }
    }
    result
}

fn linux_sys_chown(path_ptr: u64, uid: u32, gid: u32) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    match crate::fs::chown(&path, uid, gid) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_fchown(fd: i32, uid: u32, gid: u32) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if let Some(handle) = table.get(&fd) {
            match crate::fs::chown(&handle.path, uid, gid) {
                Ok(_) => 0,
                Err(e) => -errno::from_fs_error(&e),
            }
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_lchown(path_ptr: u64, uid: u32, gid: u32) -> i64 {
    // Same as chown since we don't follow symlinks in our simple FS
    linux_sys_chown(path_ptr, uid, gid)
}

fn linux_sys_renameat(old_dirfd: i32, old_path_ptr: u64, new_dirfd: i32, new_path_ptr: u64) -> i64 {
    let old_path = match read_user_string(old_path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let new_path = match read_user_string(new_path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    const AT_FDCWD: i32 = -100;
    let resolve_at_dir = |dirfd: i32, path: &str| -> Option<String> {
        if path.starts_with('/') {
            return crate::fs::path::resolve(path).ok();
        }
        if dirfd == AT_FDCWD {
            return crate::fs::path::resolve(path).ok();
        }
        crate::scheduler::with_current_task(|task| {
            let table = task.fd_table.lock();
            if let Some(handle) = table.get(&dirfd) {
                let base = handle.path.trim_end_matches('/');
                crate::fs::path::resolve(&format!("{}/{}", base, path)).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| crate::fs::path::resolve(path).ok())
    };

    let abs_old = match resolve_at_dir(old_dirfd, &old_path) {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    let abs_new = match resolve_at_dir(new_dirfd, &new_path) {
        Some(p) => p,
        None => return -errno::ENOENT,
    };
    match crate::fs::mv(&abs_old, &abs_new) {
        Ok(_) => 0,
        Err(e) => -errno::from_fs_error(&e),
    }
}

fn linux_sys_pause() -> i64 {
    // Yield forever (or until signal, which we don't have)
    loop {
        crate::scheduler::yield_now();
    }
}

fn linux_sys_times(buf: u64) -> i64 {
    if buf == 0 || !crate::security::validate_user_ptr(buf) {
        return -errno::EFAULT;
    }
    #[repr(C)]
    struct tms {
        tms_utime: i64,
        tms_stime: i64,
        tms_cutime: i64,
        tms_cstime: i64,
    }
    let tp = tms {
        tms_utime: 0,
        tms_stime: 0,
        tms_cutime: 0,
        tms_cstime: 0,
    };
    unsafe {
        core::ptr::write_volatile(buf as *mut tms, tp);
    }
    0
}

fn linux_sys_clock_getres(_clock_id: i32, tp_ptr: u64) -> i64 {
    if tp_ptr == 0 || !crate::security::validate_user_ptr(tp_ptr) {
        return -errno::EFAULT;
    }
    // 1ms resolution
    unsafe {
        core::ptr::write_volatile(tp_ptr as *mut [u64; 2], [0u64, 1_000_000u64]);
    }
    0
}

fn linux_sys_clock_nanosleep(_clock_id: i32, _flags: i32, req_ptr: u64, rem_ptr: u64) -> i64 {
    // Same as nanosleep
    if req_ptr == 0 || !crate::security::validate_user_ptr(req_ptr) {
        return -errno::EFAULT;
    }
    let ts: [u64; 2] = unsafe { core::ptr::read_volatile(req_ptr as *const [u64; 2]) };
    let ms = ts[0].saturating_mul(1000).saturating_add(ts[1] / 1_000_000);
    if ms > 0 {
        let ticks = (ms / 55).max(1);
        let start = crate::curr_arch::get_ticks();
        while crate::curr_arch::get_ticks() - start < ticks {
            crate::scheduler::yield_now();
        }
    }
    if rem_ptr != 0 && crate::security::validate_user_ptr(rem_ptr) {
        unsafe {
            core::ptr::write_volatile(rem_ptr as *mut [u64; 2], [0, 0]);
        }
    }
    0
}

fn linux_sys_getpeername(fd: i32, addr_ptr: u64, _addrlen: u64) -> i64 {
    let _ = fd;
    if addr_ptr == 0 || !crate::security::validate_user_ptr(addr_ptr) {
        return -errno::EFAULT;
    }
    // No connected sockets → return ENOTCONN
    -errno::ENOTCONN
}

fn linux_sys_getsockname(fd: i32, addr_ptr: u64, _addrlen: u64) -> i64 {
    let _ = fd;
    if addr_ptr == 0 || !crate::security::validate_user_ptr(addr_ptr) {
        return -errno::EFAULT;
    }
    // Return AF_UNSPEC
    unsafe {
        core::ptr::write_volatile(addr_ptr as *mut u16, 0u16);
    }
    0
}

fn linux_sys_fchdir(fd: i32) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if let Some(handle) = table.get(&fd) {
            let dir_path = if crate::fs::is_dir(&handle.path) {
                handle.path.clone()
            } else {
                crate::fs::path::parent(&handle.path).unwrap_or_else(|| "/".into())
            };
            match crate::fs::chdir(&dir_path) {
                Ok(_) => 0,
                Err(e) => -errno::from_fs_error(&e),
            }
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_fchmod(fd: i32, _mode: u64) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if table.contains_key(&fd) {
            0
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_fchmodat(dirfd: i32, path_ptr: u64, _mode: i32, _flags: i32) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    const AT_FDCWD: i32 = -100;
    let abs_path = if path.starts_with('/') {
        match crate::fs::path::resolve(&path).ok() {
            Some(p) => p,
            None => return -errno::ENOENT,
        }
    } else if dirfd == AT_FDCWD {
        match crate::fs::path::resolve(&path).ok() {
            Some(p) => p,
            None => return -errno::ENOENT,
        }
    } else {
        match resolve_at(dirfd, &path) {
            Some(p) => p,
            None => return -errno::ENOENT,
        }
    };
    if crate::fs::exists(&abs_path) {
        0
    } else {
        -errno::ENOENT
    }
}

fn linux_sys_faccessat(dirfd: i32, path_ptr: u64, _mode: i32, _flags: i32) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    const AT_FDCWD: i32 = -100;
    let abs_path = if path.starts_with('/') {
        match crate::fs::path::resolve(&path).ok() {
            Some(p) => p,
            None => return -errno::ENOENT,
        }
    } else if dirfd == AT_FDCWD {
        match crate::fs::path::resolve(&path).ok() {
            Some(p) => p,
            None => return -errno::ENOENT,
        }
    } else {
        match resolve_at(dirfd, &path) {
            Some(p) => p,
            None => return -errno::ENOENT,
        }
    };
    if crate::fs::exists(&abs_path) {
        0
    } else {
        -errno::ENOENT
    }
}

fn linux_sys_utime(path_ptr: u64, times_ptr: u64) -> i64 {
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if !crate::fs::exists(&path) {
        return -errno::ENOENT;
    }
    if times_ptr != 0 {
        // utimbuf = { actime: time_t, modtime: time_t } = 16 bytes
        if !crate::security::validate_user_ptr(times_ptr) {
            return -errno::EFAULT;
        }
    }
    0
}

fn linux_sys_setreuid(ruid: u32, euid: u32) -> i64 {
    if ruid != u32::MAX {
        crate::users::set_uid(ruid);
    }
    if euid != u32::MAX && euid != ruid {
        crate::users::set_uid(euid);
    }
    0
}

fn linux_sys_setregid(rgid: u32, egid: u32) -> i64 {
    if rgid != u32::MAX {
        crate::users::set_gid(rgid);
    }
    if egid != u32::MAX && egid != rgid {
        crate::users::set_gid(egid);
    }
    0
}

fn linux_sys_setresuid(ruid: u32, euid: u32, suid: u32) -> i64 {
    let _ = suid;
    if ruid != u32::MAX {
        crate::users::set_uid(ruid);
    }
    if euid != u32::MAX {
        crate::users::set_uid(euid);
    }
    0
}

fn linux_sys_setresgid(rgid: u32, egid: u32, sgid: u32) -> i64 {
    let _ = sgid;
    if rgid != u32::MAX {
        crate::users::set_gid(rgid);
    }
    if egid != u32::MAX {
        crate::users::set_gid(egid);
    }
    0
}

fn linux_sys_getresuid(buf: u64) -> i64 {
    if buf == 0 || !crate::security::validate_user_buffer(buf, 12) {
        return -errno::EFAULT;
    }
    let uid = crate::users::current_uid();
    unsafe {
        core::ptr::write_volatile(buf as *mut u32, uid);
        core::ptr::write_volatile((buf + 4) as *mut u32, uid);
        core::ptr::write_volatile((buf + 8) as *mut u32, uid);
    }
    0
}

fn linux_sys_getresgid(buf: u64) -> i64 {
    if buf == 0 || !crate::security::validate_user_buffer(buf, 12) {
        return -errno::EFAULT;
    }
    let gid = crate::users::current_gid();
    unsafe {
        core::ptr::write_volatile(buf as *mut u32, gid);
        core::ptr::write_volatile((buf + 4) as *mut u32, gid);
        core::ptr::write_volatile((buf + 8) as *mut u32, gid);
    }
    0
}

fn linux_sys_getpgid(pid: i32) -> i64 {
    if pid == 0 {
        crate::scheduler::with_current_task(|t| t.pgid as i64).unwrap_or(0)
    } else if pid > 0 {
        // In a full implementation, we'd find the task by pid
        // For now, we only check if it's the current task
        let current = crate::scheduler::current_task_id().unwrap_or(0);
        if pid as u64 == current {
            crate::scheduler::with_current_task(|t| t.pgid as i64).unwrap_or(0)
        } else {
            pid as i64 // approximate
        }
    } else {
        -errno::EINVAL
    }
}

fn linux_sys_getsid(pid: i32) -> i64 {
    if pid == 0 {
        crate::scheduler::with_current_task(|t| t.sid as i64).unwrap_or(0)
    } else if pid > 0 {
        let current = crate::scheduler::current_task_id().unwrap_or(0);
        if pid as u64 == current {
            crate::scheduler::with_current_task(|t| t.sid as i64).unwrap_or(0)
        } else {
            pid as i64
        }
    } else {
        -errno::EINVAL
    }
}

fn linux_sys_sched_rr_get_interval(_pid: i32, tp_ptr: u64) -> i64 {
    if tp_ptr == 0 || !crate::security::validate_user_ptr(tp_ptr) {
        return -errno::EFAULT;
    }
    unsafe {
        core::ptr::write_volatile(tp_ptr as *mut [u64; 2], [0u64, 0u64]);
    }
    0
}

fn fill_linux_stat(path: &str, stat_ptr: u64) -> i64 {
    if stat_ptr == 0 {
        return -errno::EFAULT;
    }
    if !crate::security::validate_user_ptr(stat_ptr) {
        return -errno::EFAULT;
    }
    match crate::fs::stat(path) {
        Ok(meta) => {
            #[repr(C)]
            struct linux_stat {
                st_dev: u64,
                st_ino: u64,
                st_mode: u32,
                st_nlink: u32,
                st_uid: u32,
                st_gid: u32,
                st_rdev: u64,
                __pad1: u64,
                st_size: i64,
                st_blksize: i32,
                __pad2: i32,
                st_blocks: i64,
                st_atime: i64,
                st_atime_nsec: u64,
                st_mtime: i64,
                st_mtime_nsec: u64,
                st_ctime: i64,
                st_ctime_nsec: u64,
                __unused4: u32,
                __unused5: u32,
            }

            let mode = match meta.node_type {
                crate::fs::NodeType::File => 0o100_644u32,
                crate::fs::NodeType::Directory => 0o40_755u32,
                crate::fs::NodeType::Symlink => 0o120_777u32,
                crate::fs::NodeType::Device => 0o20_600u32,
            };

            let now = (crate::curr_arch::get_ticks() / 18) as i64;

            let st = linux_stat {
                st_dev: 0,
                st_ino: 1,
                st_mode: mode,
                st_nlink: 1,
                st_uid: meta.owner_uid,
                st_gid: meta.owner_gid,
                st_rdev: 0,
                __pad1: 0,
                st_size: meta.size as i64,
                st_blksize: 4096,
                __pad2: 0,
                st_blocks: ((meta.size + 511) / 512) as i64,
                st_atime: now,
                st_atime_nsec: 0,
                st_mtime: now,
                st_mtime_nsec: 0,
                st_ctime: now,
                st_ctime_nsec: 0,
                __unused4: 0,
                __unused5: 0,
            };

            unsafe {
                core::ptr::write_volatile(stat_ptr as *mut linux_stat, st);
            }
            0
        }
        Err(_) => -errno::ENOENT,
    }
}

fn linux_sys_open_simple(abs_path: &str, flags: i32) -> i64 {
    let is_creat = (flags & 0x40) != 0;
    let is_trunc = (flags & 0x200) != 0;
    let is_wronly = (flags & 3) == 1;
    let is_rdwr = (flags & 3) == 2;

    if is_creat {
        let _ = crate::fs::touch(abs_path);
    }

    if crate::fs::is_dir(abs_path) {
        if is_wronly || is_rdwr {
            return -errno::EISDIR;
        }
    }

    match crate::fs::stat(abs_path) {
        Ok(_) => {
            if is_trunc && (is_wronly || is_rdwr) {
                let _ = crate::fs::write(abs_path, b"");
            }
        }
        Err(_) if !is_creat => return -errno::ENOENT,
        Err(_) => {}
    }

    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        let next_fd = (3..).find(|fd| !table.contains_key(fd)).unwrap_or(3);
        table.insert(
            next_fd,
            crate::fs::FileHandle {
                path: abs_path.to_string(),
                pos: 0,
                node_type: if crate::fs::is_dir(abs_path) {
                    crate::fs::NodeType::Directory
                } else {
                    crate::fs::NodeType::File
                },
            },
        );
        next_fd as i64
    })
    .unwrap_or(-errno::EMFILE)
}

fn linux_sys_getdents(fd: i32, _dirp: u64, _count: u64) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if let Some(handle) = table.get(&fd) {
            if crate::fs::is_dir(&handle.path) {
                -errno::EISDIR
            } else {
                -errno::ENOTDIR
            }
        } else {
            -errno::EBADF
        }
    })
    .unwrap_or(-errno::EBADF)
}

fn linux_sys_splice(fd_in: i32, _off_in: u64, fd_out: i32, _off_out: u64, len: u64) -> i64 {
    if len == 0 || len > 4096 {
        return 0;
    }
    let count = len as usize;
    let mut buf = alloc::vec![0u8; count];
    let nread = {
        if crate::pipe::is_pipe_fd(fd_in) {
            match crate::pipe::pipe_read(fd_in, &mut buf) {
                Ok(n) => n as i64,
                Err(_) => return -errno::EIO,
            }
        } else {
            crate::scheduler::with_current_task(|task| {
                let table = task.fd_table.lock();
                if let Some(handle) = table.get(&fd_in) {
                    if let Ok(data) = crate::fs::read(&handle.path) {
                        let len = count.min(data.len());
                        buf[..len].copy_from_slice(&data[..len]);
                        len as i64
                    } else {
                        -errno::EIO
                    }
                } else {
                    -errno::EBADF
                }
            })
            .unwrap_or(-errno::EBADF)
        }
    };
    if nread <= 0 {
        return nread;
    }
    let nread = nread as usize;
    if crate::pipe::is_pipe_fd(fd_out) {
        match crate::pipe::pipe_write(fd_out, &buf[..nread]) {
            Ok(n) => n as i64,
            Err(_) => -errno::EIO,
        }
    } else if fd_out == 1 || fd_out == 2 {
        for &byte in &buf[..nread] {
            let c = byte as char;
            crate::serial_print!("{}", c);
            crate::drivers::framebuffer::console::_print(format_args!("{}", c));
        }
        nread as i64
    } else {
        crate::scheduler::with_current_task(|task| {
            let table = task.fd_table.lock();
            if let Some(handle) = table.get(&fd_out) {
                if crate::fs::write(&handle.path, &buf[..nread]).is_ok() {
                    nread as i64
                } else {
                    -errno::EIO
                }
            } else {
                -errno::EBADF
            }
        })
        .unwrap_or(-errno::EBADF)
    }
}

fn linux_sys_memfd_create(name_ptr: u64, _flags: u32) -> i64 {
    let _name = match read_user_string(name_ptr) {
        Ok(n) => n,
        Err(e) => return e,
    };
    let path = alloc::format!("/memfd-{}", crate::curr_arch::get_ticks());
    let _ = crate::fs::touch(&path);
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        let next_fd = (3..).find(|fd| !table.contains_key(fd)).unwrap_or(3);
        table.insert(
            next_fd,
            crate::fs::FileHandle {
                path: path.clone(),
                pos: 0,
                node_type: crate::fs::NodeType::File,
            },
        );
        next_fd as i64
    })
    .unwrap_or(-errno::EMFILE)
}

fn linux_sys_close_range(first: u32, last: u32, _flags: i32) -> i64 {
    crate::scheduler::with_current_task(|task| {
        let mut table = task.fd_table.lock();
        for fd in first..=last {
            let fd = fd as i32;
            if fd >= 3 {
                table.remove(&fd);
            }
        }
    });
    0
}

fn linux_sys_pselect6(
    nfds: i32,
    readfds: u64,
    writefds: u64,
    exceptfds: u64,
    timeout_ptr: u64,
) -> i64 {
    let _ = timeout_ptr;
    linux_sys_select(nfds, readfds, writefds, exceptfds, 0)
}

fn linux_sys_ppoll(fds: u64, nfds: u64, timeout_ts_ptr: u64, _sigmask: u64) -> i64 {
    let timeout_ms = if timeout_ts_ptr != 0 && crate::security::validate_user_ptr(timeout_ts_ptr) {
        let ts: [u64; 2] = unsafe { core::ptr::read_volatile(timeout_ts_ptr as *const [u64; 2]) };
        (ts[0].saturating_mul(1000)).saturating_add(ts[1] / 1_000_000)
    } else if timeout_ts_ptr == 0 {
        u64::MAX
    } else {
        0
    };
    linux_sys_poll(fds, nfds, timeout_ms)
}

fn linux_sys_renameat2(
    old_dirfd: i32,
    old_path_ptr: u64,
    new_dirfd: i32,
    new_path_ptr: u64,
    _flags: i32,
) -> i64 {
    linux_sys_renameat(old_dirfd, old_path_ptr, new_dirfd, new_path_ptr)
}

fn resolve_at(dirfd: i32, path: &str) -> Option<String> {
    if path.starts_with('/') {
        crate::fs::path::resolve(path).ok()
    } else {
        crate::scheduler::with_current_task(|task| {
            let table = task.fd_table.lock();
            if let Some(handle) = table.get(&dirfd) {
                let base = handle.path.trim_end_matches('/');
                crate::fs::path::resolve(&format!("{}/{}", base, path)).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| crate::fs::path::resolve(path).ok())
    }
}

fn linux_sys_sendmsg(fd: i32, msg_ptr: u64, _flags: u64, _dest_addr: u64) -> i64 {
    // parse msghdr from user space
    if !crate::security::validate_user_ptr(msg_ptr) {
        return -errno::EFAULT;
    }
    // msghdr x86_64: msg_name(8) + msg_namelen(4) + pad(4) + msg_iov(8) + msg_iovlen(8) + msg_control(8) + msg_controllen(8) + msg_flags(4) + pad(4) = 56
    let hdr: [u64; 7] = unsafe { core::ptr::read_volatile(msg_ptr as *const [u64; 7]) };
    let iov_ptr = hdr[2]; // msg_iov
    let iovlen = hdr[3] as usize; // msg_iovlen
    if iovlen > 1024 || iov_ptr == 0 {
        return -errno::EINVAL;
    }
    let name_ptr = hdr[0];
    let _namelen = if name_ptr != 0 {
        unsafe { core::ptr::read_volatile(name_ptr as *const u32) }
    } else {
        0
    };
    // collect data from iovecs
    let mut data = Vec::new();
    for i in 0..iovlen {
        let iov: [u64; 2] =
            unsafe { core::ptr::read_volatile((iov_ptr + (i as u64 * 16)) as *const [u64; 2]) };
        let base = iov[0];
        let len = iov[1] as usize;
        if len == 0 {
            continue;
        }
        if !crate::security::validate_user_buffer(base, len) {
            return -errno::EFAULT;
        }
        let slice = unsafe { core::slice::from_raw_parts(base as *const u8, len) };
        data.extend_from_slice(slice);
    }
    if data.is_empty() {
        return 0;
    }
    // use existing sendto machinery
    if name_ptr != 0 && fd >= 10000 && fd < 30000 {
        let addr_buf = unsafe { core::slice::from_raw_parts(name_ptr as *const u8, 16) };
        let family = u16::from_ne_bytes([addr_buf[0], addr_buf[1]]);
        if family != 2 {
            return -errno::EAFNOSUPPORT;
        }
        let dport = u16::from_be_bytes([addr_buf[2], addr_buf[3]]);
        let dip = u32::from_be_bytes([addr_buf[4], addr_buf[5], addr_buf[6], addr_buf[7]]);
        crate::linux_compat::socket::socket_sendto(fd, &data, dip, dport)
    } else if fd >= 10000 && fd < 30000 {
        let table = crate::linux_compat::socket::SOCKET_TABLE.lock();
        let idx = (fd - 10000) as usize;
        if let Some(Some(sock)) = table.get(idx) {
            match sock {
                crate::linux_compat::socket::Socket::Udp(_) => {
                    drop(table);
                    -errno::EDESTADDRREQ
                }
                _ => -errno::EOPNOTSUPP,
            }
        } else {
            -errno::EBADF
        }
    } else {
        -errno::EBADF
    }
}

fn linux_sys_recvmsg(fd: i32, msg_ptr: u64, _flags: u64) -> i64 {
    if !crate::security::validate_user_ptr(msg_ptr) {
        return -errno::EFAULT;
    }
    let hdr: [u64; 7] = unsafe { core::ptr::read_volatile(msg_ptr as *const [u64; 7]) };
    let iov_ptr = hdr[2];
    let iovlen = hdr[3] as usize;
    if iovlen == 0 || iov_ptr == 0 {
        return -errno::EINVAL;
    }
    // read into first iov
    let iov0: [u64; 2] = unsafe { core::ptr::read_volatile(iov_ptr as *const [u64; 2]) };
    let buf = iov0[0];
    let len = iov0[1] as usize;
    if buf == 0 || len == 0 {
        return 0;
    }
    if !crate::security::validate_user_buffer(buf, len) {
        return -errno::EFAULT;
    }
    let slice = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, len) };
    let nread = if fd >= 10000 && fd < 30000 {
        crate::linux_compat::socket::socket_recvfrom(fd, slice)
    } else {
        -errno::EBADF
    };
    if nread > 0 {
        // set msg_namelen = 0
        let namelen_ptr = (msg_ptr + 8) as *mut u32;
        unsafe {
            core::ptr::write_volatile(namelen_ptr, 0u32);
        }
        // set msg_flags = 0
        let flags_ptr = (msg_ptr + 48) as *mut u32;
        unsafe {
            core::ptr::write_volatile(flags_ptr, 0u32);
        }
        // also fill remaining iovecs with 0-length for a correct result
        for i in 1..iovlen {
            let _ = i;
        }
    }
    nread
}

fn linux_sys_shutdown(_fd: i32, _how: i32) -> i64 {
    0
}

fn linux_sys_connect(fd: i32, addr_ptr: u64, addrlen: u64) -> i64 {
    if fd >= 10000 && fd < 30000 {
        if !crate::security::validate_user_ptr(addr_ptr) {
            return -errno::EFAULT;
        }
        let buf = unsafe { core::slice::from_raw_parts(addr_ptr as *const u8, 16) };
        let family = u16::from_ne_bytes([buf[0], buf[1]]);
        if family != 2 {
            return -errno::EAFNOSUPPORT;
        }
        let port = u16::from_be_bytes([buf[2], buf[3]]);
        let ip = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let _ = addrlen;
        crate::linux_compat::socket::socket_connect(fd, ip, port)
    } else {
        -errno::ENOTSOCK
    }
}

fn linux_sys_listen(fd: i32, backlog: i32) -> i64 {
    if fd >= 10000 && fd < 30000 {
        crate::linux_compat::socket::socket_listen(fd, backlog)
    } else {
        -errno::ENOTSOCK
    }
}

fn linux_sys_accept(fd: i32, addr_ptr: u64, addrlen_ptr: u64) -> i64 {
    if fd >= 10000 && fd < 30000 {
        crate::linux_compat::socket::socket_accept(fd, addr_ptr, addrlen_ptr)
    } else {
        -errno::ENOTSOCK
    }
}

fn linux_sys_accept4(fd: i32, addr_ptr: u64, addrlen_ptr: u64, _flags: i32) -> i64 {
    linux_sys_accept(fd, addr_ptr, addrlen_ptr)
}

fn linux_sys_fork() -> i64 {
    let kstack = crate::smp::syscall_kstack();
    if kstack == 0 {
        return -errno::ENOMEM;
    }
    let user_rsp;
    unsafe {
        user_rsp = crate::smp::usr_rsp();
    }
    let (user_rip, user_rflags, user_rbp, user_rbx, user_r12, user_r13, user_r14, user_r15) = unsafe {
        (
            core::ptr::read_volatile((kstack - 8) as *const u64),
            core::ptr::read_volatile((kstack - 16) as *const u64),
            core::ptr::read_volatile((kstack - 24) as *const u64),
            core::ptr::read_volatile((kstack - 32) as *const u64),
            core::ptr::read_volatile((kstack - 40) as *const u64),
            core::ptr::read_volatile((kstack - 48) as *const u64),
            core::ptr::read_volatile((kstack - 56) as *const u64),
            core::ptr::read_volatile((kstack - 64) as *const u64),
        )
    };
    let (parent_id, fd_table_clone) = crate::scheduler::with_current_task(|task| {
        let fd_table = task.fd_table.lock().clone();
        (task.id, fd_table)
    })
    .unwrap_or((0, alloc::collections::BTreeMap::new()));
    let addr_space = match crate::scheduler::with_current_task(|task| {
        task.address_space.as_ref().and_then(|as_| as_.clone().ok())
    })
    .flatten()
    {
        Some(as_) => as_,
        None => return -errno::ENOMEM,
    };
    let child_id = crate::scheduler::new_task_id();
    let kstack_size: usize = 16384;
    let kernel_stack = vec![0u8; kstack_size];
    let stack_bottom = kernel_stack.as_ptr() as u64;
    let stack_top = stack_bottom + kstack_size as u64;
    let stack_top_aligned = stack_top & !0xF;
    let base = stack_top_aligned - 128;
    let fork_return_addr = crate::syscall::fork_return as usize as u64;
    unsafe {
        core::ptr::write_volatile((base + 0) as *mut u64, user_r15);
        core::ptr::write_volatile((base + 8) as *mut u64, user_r14);
        core::ptr::write_volatile((base + 16) as *mut u64, user_r13);
        core::ptr::write_volatile((base + 24) as *mut u64, user_r12);
        core::ptr::write_volatile((base + 32) as *mut u64, user_rbx);
        core::ptr::write_volatile((base + 40) as *mut u64, user_rbp);
        core::ptr::write_volatile((base + 48) as *mut u64, fork_return_addr);
        core::ptr::write_volatile((base + 56) as *mut u64, user_r15);
        core::ptr::write_volatile((base + 64) as *mut u64, user_r14);
        core::ptr::write_volatile((base + 72) as *mut u64, user_r13);
        core::ptr::write_volatile((base + 80) as *mut u64, user_r12);
        core::ptr::write_volatile((base + 88) as *mut u64, user_rbx);
        core::ptr::write_volatile((base + 96) as *mut u64, user_rbp);
        core::ptr::write_volatile((base + 104) as *mut u64, user_rflags);
        core::ptr::write_volatile((base + 112) as *mut u64, user_rip);
        core::ptr::write_volatile((base + 120) as *mut u64, user_rsp);
    }
    let mut child_ctx = crate::scheduler::Context::new();
    child_ctx.set_sp(base);
    child_ctx.set_page_table(addr_space.cr3());
    let child = crate::scheduler::Task {
        id: child_id,
        name: alloc::format!("fork-{}", child_id),
        state: crate::scheduler::TaskState::Ready,
        context: child_ctx,
        kernel_stack,
        kernel_stack_top: stack_top_aligned,
        address_space: Some(addr_space),
        is_user: true,
        user_entry: user_rip,
        user_stack: user_rsp,
        priority: 1,
        quantum: 3,
        ticks_used: 0,
        total_ticks: 0,
        fd_table: spin::Mutex::new(fd_table_clone),
        is_linux: true,
        linux_sigblock: [0; 1],
        parent_id: Some(parent_id),
        exit_code: 0,
        pgid: parent_id,
        sid: parent_id,
    };
    crate::scheduler::add_ready_task(Box::new(child));
    child_id as i64
}

fn linux_sys_vfork() -> i64 {
    linux_sys_fork()
}

fn linux_sys_clone(
    flags: u64,
    child_stack: u64,
    _parent_tid: u64,
    _child_tls: u64,
    _child_tid: u64,
) -> i64 {
    let _ = child_stack;
    if (flags & 0x00000100) != 0 {
        linux_sys_fork()
    } else {
        linux_sys_fork()
    }
}

fn linux_sys_clone3(_args_ptr: u64, _size: u64) -> i64 {
    linux_sys_fork()
}

fn linux_sys_init_module(module_ptr: u64, len: u64) -> i64 {
    if !crate::security::validate_user_buffer(module_ptr, len as usize) {
        return -errno::EFAULT;
    }
    let elf_data = unsafe { core::slice::from_raw_parts(module_ptr as *const u8, len as usize) };
    match unsafe { crate::shim::manager::load_driver_module(elf_data, "user_module") } {
        Ok(ret) => ret as i64,
        Err(e) => {
            crate::printk!("[LINUX] init_module failed: {}", e);
            -errno::EINVAL
        }
    }
}

fn linux_sys_finit_module(fd: i32, param_ptr: u64, flags: i32) -> i64 {
    let _ = param_ptr;
    let _ = flags;
    let result = crate::scheduler::with_current_task(|task| {
        let table = task.fd_table.lock();
        if let Some(handle) = table.get(&fd) {
            let mod_name = {
                let path = &handle.path;
                let basename = path.rsplit('/').next().unwrap_or(path);
                if let Some(dot) = basename.rfind('.') {
                    basename[..dot].to_string()
                } else {
                    basename.to_string()
                }
            };
            let data = match crate::fs::read(&handle.path) {
                Ok(d) => Some(d),
                Err(_) => None,
            };
            data.map(|d| (d, mod_name))
        } else {
            None
        }
    })
    .flatten();
    match result {
        Some((elf_data, mod_name)) => {
            match unsafe { crate::shim::manager::load_driver_module(&elf_data, &mod_name) } {
                Ok(ret) => ret as i64,
                Err(e) => {
                    crate::printk!("[LINUX] finit_module '{}' failed: {}", mod_name, e);
                    -errno::EINVAL
                }
            }
        }
        None => -errno::EBADF,
    }
}

fn linux_sys_delete_module(name_ptr: u64, flags: i32) -> i64 {
    let name = match read_user_string(name_ptr) {
        Ok(n) => n,
        Err(e) => return e,
    };
    // Check if any module depends on this one
    if crate::shim::loader::any_module_depends_on(&name) {
        return -errno::EBUSY;
    }
    let refcount = crate::shim::loader::module_refcount(&name);
    if refcount > 0 && (flags & 0x8000) == 0 {
        // O_NONBLOCK not set, refuse to unload busy module
        return -errno::EBUSY;
    }
    match unsafe { crate::shim::loader::unload_module(&name) } {
        Ok(_) => 0,
        Err(e) => {
            crate::printk!("[LINUX] delete_module failed: {}", e);
            -errno::ENOENT
        }
    }
}

fn read_user_string(ptr: u64) -> Result<String, i64> {
    if !crate::security::validate_user_ptr(ptr) {
        return Err(-errno::EFAULT);
    }
    let mut s = String::new();
    let mut curr = ptr;
    loop {
        if s.len() >= 256 {
            return Err(-errno::ENAMETOOLONG);
        }
        let val = unsafe { core::ptr::read_volatile(curr as *const u8) };
        if val == 0 {
            break;
        }
        s.push(val as char);
        curr += 1;
    }
    Ok(s)
}

use spin::Mutex;

// Process priority (nice value)
static PROCESS_PRIORITY: Mutex<i32> = Mutex::new(0);

fn linux_sys_getgroups(size: i32, list: u64) -> i64 {
    if size == 0 {
        return 0;
    }
    if list == 0 || size < 0 {
        return -errno::EINVAL;
    }
    -errno::EOPNOTSUPP
}

fn linux_sys_setgroups(_size: u32, _list: u64) -> i64 {
    -errno::EOPNOTSUPP
}

fn linux_sys_setfsuid(_uid: u32) -> i64 {
    0
}

fn linux_sys_setfsgid(_gid: u32) -> i64 {
    0
}

fn linux_sys_setpriority(_which: i32, _who: i32, prio: i32) -> i64 {
    *PROCESS_PRIORITY.lock() = prio;
    0
}

fn linux_sys_getpriority(_which: i32, _who: i32) -> i64 {
    *PROCESS_PRIORITY.lock() as i64
}

fn linux_sys_sched_setparam(_pid: i32, _param: u64) -> i64 {
    0
}

fn linux_sys_sched_getparam(_pid: i32, param: u64) -> i64 {
    if param == 0 {
        return -errno::EINVAL;
    }
    unsafe {
        *(param as *mut i32) = 0;
    }
    0
}

fn linux_sys_sched_setscheduler(_pid: i32, _policy: i32, _param: u64) -> i64 {
    0
}

fn linux_sys_sched_getscheduler(_pid: i32) -> i64 {
    0
}

fn linux_sys_sched_get_priority_max(_policy: i32) -> i64 {
    0
}

fn linux_sys_sched_get_priority_min(_policy: i32) -> i64 {
    0
}

fn linux_sys_mlock(addr: u64, len: u64) -> i64 {
    if len == 0 {
        return 0;
    }
    if addr & 0xFFF != 0 {
        return -errno::EINVAL;
    }
    // MesOS has no swap, so memory is always "locked"
    0
}

fn linux_sys_munlock(addr: u64, len: u64) -> i64 {
    if len == 0 {
        return 0;
    }
    if addr & 0xFFF != 0 {
        return -errno::EINVAL;
    }
    0
}

fn linux_sys_mlockall(flags: i32) -> i64 {
    if flags & !3 != 0 {
        return -errno::EINVAL;
    }
    // MCL_CURRENT=1, MCL_FUTURE=2 - MesaOS has no swap, accept always
    0
}

fn linux_sys_munlockall() -> i64 {
    0
}

fn linux_sys_arch_prctl(code: i32, _addr: u64, _arg2: u64) -> i64 {
    match code {
        0x1001 => 0,
        0x1002 => 0,
        0x1003 => -errno::EINVAL,
        0x1004 => 0,
        0x1005 => -errno::EINVAL,
        _ => -errno::EINVAL,
    }
}

fn linux_sys_sync() -> i64 {
    crate::fs::sync().map(|_| 0).unwrap_or(-errno::EIO)
}

fn linux_sys_sched_setaffinity(_pid: i32, _len: u64, _mask: u64) -> i64 {
    0
}

fn linux_sys_sched_getaffinity(_pid: i32, len: u64, mask: u64) -> i64 {
    if mask == 0 {
        return -errno::EFAULT;
    }
    unsafe {
        core::ptr::write_bytes(mask as *mut u8, 0xFF, len.min(8) as usize);
    }
    len as i64
}

fn linux_sys_set_thread_area(_u_info: u64) -> i64 {
    0
}

fn linux_sys_get_thread_area(_u_info: u64) -> i64 {
    -errno::EOPNOTSUPP
}

fn linux_sys_set_tid_address(_tidptr: i32) -> i64 {
    crate::scheduler::current_task_id().unwrap_or(0) as i64
}

fn linux_sys_timer_getoverrun(_timer_id: i32) -> i64 {
    0
}

fn linux_sys_utimes(path_ptr: u64, times_ptr: u64) -> i64 {
    linux_sys_utimensat(-100, path_ptr, times_ptr, 0)
}

fn linux_sys_utimensat(dirfd: i32, path_ptr: u64, times_ptr: u64, flags: i32) -> i64 {
    if path_ptr == 0 {
        return -errno::EFAULT;
    }
    let path = match read_user_string(path_ptr) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if (flags & 0x100) != 0 {
        return -errno::EOPNOTSUPP;
    }
    if !crate::fs::exists(&path) {
        return -errno::ENOENT;
    }
    if times_ptr != 0 {
        // Could set timestamps here if fs supported it
        // For now accept the call but don't actually modify timestamps
    }
    let _ = dirfd;
    0
}

fn linux_sys_ioprio_set(_which: i32, _who: i32, _ioprio: i32) -> i64 {
    0
}

fn linux_sys_ioprio_get(_which: i32, _who: i32) -> i64 {
    0
}

fn linux_sys_fallocate(fd: i32, mode: i32, offset: u64, len: u64) -> i64 {
    if len == 0 {
        return 0;
    }
    if mode != 0 {
        return -errno::EOPNOTSUPP;
    }
    let new_size = offset.checked_add(len).unwrap_or(u64::MAX);
    linux_sys_ftruncate(fd, new_size)
}

fn linux_sys_sched_setattr(_pid: i32, _attr: u64, _flags: u32) -> i64 {
    0
}

fn linux_sys_sched_getattr(_pid: i32, attr: u64, _flags: u32) -> i64 {
    if attr == 0 {
        return -errno::EFAULT;
    }
    unsafe {
        *(attr as *mut u32) = 0;
    }
    0
}

fn linux_sys_membarrier(_cmd: i32, _flags: i32) -> i64 {
    0
}

static SOCK_OPTS: spin::Mutex<
    alloc::collections::BTreeMap<i32, alloc::collections::BTreeMap<i32, i32>>,
> = spin::Mutex::new(alloc::collections::BTreeMap::new());

fn linux_sys_setsockopt(fd: i32, level: i32, optname: i32, optval: u64, optlen: u64) -> i64 {
    if fd < 10000 || fd >= 30000 {
        return -errno::ENOTSOCK;
    }
    if level == 1 {
        return 0;
    }
    if optval == 0 || optlen < 4 {
        return -errno::EFAULT;
    }
    let val = unsafe { *(optval as *const i32) };
    SOCK_OPTS
        .lock()
        .entry(fd)
        .or_insert_with(alloc::collections::BTreeMap::new)
        .insert((level << 16) | optname, val);
    0
}

fn linux_sys_getsockopt(fd: i32, level: i32, optname: i32, optval: u64, optlen: u64) -> i64 {
    if fd < 10000 || fd >= 30000 {
        return -errno::ENOTSOCK;
    }
    if optval == 0 || optlen == 0 {
        return -errno::EFAULT;
    }
    let key = (level << 16) | optname;
    let val = SOCK_OPTS
        .lock()
        .get(&fd)
        .and_then(|m| m.get(&key))
        .copied()
        .unwrap_or(0);
    unsafe {
        *(optval as *mut i32) = val;
    }
    0
}
