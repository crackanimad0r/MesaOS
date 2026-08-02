pub mod abi;
pub mod epoll;
pub mod errno;
pub mod eventfd;
pub mod posix_timer;
pub mod procfs;
pub mod signal;
pub mod signalfd;
pub mod socket;
pub mod socketpair;
pub mod syscalls;
pub mod sysfs;
pub mod timerfd;

pub const O_RDONLY: u64 = 0;
pub const O_WRONLY: u64 = 1;
pub const O_RDWR: u64 = 2;
pub const O_CREAT: u64 = 0x40;
pub const O_EXCL: u64 = 0x80;
pub const O_NOCTTY: u64 = 0x100;
pub const O_TRUNC: u64 = 0x200;
pub const O_APPEND: u64 = 0x400;
pub const O_NONBLOCK: u64 = 0x800;
pub const O_DSYNC: u64 = 0x1000;
pub const O_DIRECT: u64 = 0x4000;
pub const O_LARGEFILE: u64 = 0x8000;
pub const O_DIRECTORY: u64 = 0x10000;
pub const O_NOFOLLOW: u64 = 0x20000;
pub const O_NOATIME: u64 = 0x40000;
pub const O_CLOEXEC: u64 = 0x80000;
pub const O_SYNC: u64 = 0x101000;

pub fn init() {
    crate::serial_println!("[LINUX_COMPAT] Inicializando capa de compatibilidad...");
    procfs::populate();
    sysfs::populate();
    crate::serial_println!("[LINUX_COMPAT] /proc y /sys poblados");
    crate::serial_println!(
        "[LINUX_COMPAT] {} syscalls Linux traducidas",
        abi::syscall_numbers::LINUX_MSEAL
    );
    crate::klog_info!("Linux compatibility layer initialized");
}

pub fn is_linux_syscall(num: u64) -> bool {
    use abi::syscall_numbers::*;
    match num {
        LINUX_READ
        | LINUX_WRITE
        | LINUX_OPEN
        | LINUX_CLOSE
        | LINUX_STAT
        | LINUX_FSTAT
        | LINUX_LSTAT
        | LINUX_POLL
        | LINUX_LSEEK
        | LINUX_MMAP
        | LINUX_MPROTECT
        | LINUX_MUNMAP
        | LINUX_BRK
        | LINUX_RT_SIGACTION
        | LINUX_RT_SIGPROCMASK
        | LINUX_RT_SIGRETURN
        | LINUX_IOCTL
        | LINUX_PREAD64
        | LINUX_PWRITE64
        | LINUX_READV
        | LINUX_WRITEV
        | LINUX_ACCESS
        | LINUX_PIPE
        | LINUX_SELECT
        | LINUX_SCHED_YIELD
        | LINUX_MREMAP
        | LINUX_MSYNC
        | LINUX_MINCORE
        | LINUX_MADVISE
        | LINUX_DUP
        | LINUX_DUP2
        | LINUX_PAUSE
        | LINUX_NANOSLEEP
        | LINUX_GETITIMER
        | LINUX_ALARM
        | LINUX_SETITIMER
        | LINUX_GETPID
        | LINUX_SENDFILE
        | LINUX_SOCKET
        | LINUX_CONNECT
        | LINUX_ACCEPT
        | LINUX_SENDTO
        | LINUX_RECVFROM
        | LINUX_SENDMSG
        | LINUX_RECVMSG
        | LINUX_SHUTDOWN
        | LINUX_BIND
        | LINUX_LISTEN
        | LINUX_GETSOCKNAME
        | LINUX_GETPEERNAME
        | LINUX_SOCKETPAIR
        | LINUX_SETSOCKOPT
        | LINUX_GETSOCKOPT
        | LINUX_CLONE
        | LINUX_FORK
        | LINUX_VFORK
        | LINUX_EXECVE
        | LINUX_EXIT
        | LINUX_WAIT4
        | LINUX_KILL
        | LINUX_UNAME
        | LINUX_FCNTL
        | LINUX_FLOCK
        | LINUX_FSYNC
        | LINUX_FDATASYNC
        | LINUX_TRUNCATE
        | LINUX_FTRUNCATE
        | LINUX_GETDENTS
        | LINUX_GETCWD
        | LINUX_CHDIR
        | LINUX_FCHDIR
        | LINUX_RENAME
        | LINUX_MKDIR
        | LINUX_RMDIR
        | LINUX_CREAT
        | LINUX_LINK
        | LINUX_UNLINK
        | LINUX_SYMLINK
        | LINUX_READLINK
        | LINUX_CHMOD
        | LINUX_FCHMOD
        | LINUX_CHOWN
        | LINUX_FCHOWN
        | LINUX_LCHOWN
        | LINUX_UMASK
        | LINUX_GETTIMEOFDAY
        | LINUX_GETRLIMIT
        | LINUX_GETRUSAGE
        | LINUX_SYSINFO
        | LINUX_TIMES
        | LINUX_PTRACE
        | LINUX_GETUID
        | LINUX_SYSLOG
        | LINUX_GETGID
        | LINUX_SETUID
        | LINUX_SETGID
        | LINUX_GETEUID
        | LINUX_GETEGID
        | LINUX_SETPGID
        | LINUX_GETPPID
        | LINUX_GETPGRP
        | LINUX_SETSID
        | LINUX_SETREUID
        | LINUX_SETREGID
        | LINUX_GETGROUPS
        | LINUX_SETGROUPS
        | LINUX_SETRESUID
        | LINUX_GETRESUID
        | LINUX_SETRESGID
        | LINUX_GETRESGID
        | LINUX_GETPGID
        | LINUX_SETFSUID
        | LINUX_SETFSGID
        | LINUX_GETSID
        | LINUX_CAPGET
        | LINUX_CAPSET
        | LINUX_RT_SIGPENDING
        | LINUX_RT_SIGTIMEDWAIT
        | LINUX_RT_SIGQUEUEINFO
        | LINUX_RT_SIGSUSPEND
        | LINUX_SIGALTSTACK
        | LINUX_UTIME
        | LINUX_MKNOD
        | LINUX_STATFS
        | LINUX_FSTATFS
        | LINUX_SCHED_SETPARAM
        | LINUX_SCHED_GETPARAM
        | LINUX_SCHED_SETSCHEDULER
        | LINUX_SCHED_GETSCHEDULER
        | LINUX_SCHED_GET_PRIORITY_MAX
        | LINUX_SCHED_GET_PRIORITY_MIN
        | LINUX_SCHED_RR_GET_INTERVAL
        | LINUX_MLOCK
        | LINUX_MUNLOCK
        | LINUX_MLOCKALL
        | LINUX_MUNLOCKALL
        | LINUX_PRCTL
        | LINUX_ARCH_PRCTL
        | LINUX_ADJTIMEX
        | LINUX_SETRLIMIT
        | LINUX_CHROOT
        | LINUX_SYNC
        | LINUX_SETTIMEOFDAY
        | LINUX_MOUNT
        | LINUX_UMOUNT2
        | LINUX_SWAPON
        | LINUX_SWAPOFF
        | LINUX_REBOOT
        | LINUX_SETHOSTNAME
        | LINUX_SETDOMAINNAME
        | LINUX_IOPL
        | LINUX_IOPERM
        | LINUX_QUOTACTL
        | LINUX_GETTID
        | LINUX_READAHEAD
        | LINUX_TKILL
        | LINUX_TIME
        | LINUX_FUTEX
        | LINUX_SCHED_SETAFFINITY
        | LINUX_SCHED_GETAFFINITY
        | LINUX_SET_THREAD_AREA
        | LINUX_GET_THREAD_AREA
        | LINUX_GETDENTS64
        | LINUX_SET_TID_ADDRESS
        | LINUX_RESTART_SYSCALL
        | LINUX_FADVISE64
        | LINUX_TIMER_CREATE
        | LINUX_TIMER_SETTIME
        | LINUX_TIMER_GETTIME
        | LINUX_TIMER_GETOVERRUN
        | LINUX_TIMER_DELETE
        | LINUX_CLOCK_SETTIME
        | LINUX_CLOCK_GETTIME
        | LINUX_CLOCK_GETRES
        | LINUX_CLOCK_NANOSLEEP
        | LINUX_EXIT_GROUP
        | LINUX_EPOLL_CREATE
        | LINUX_EPOLL_CTL
        | LINUX_EPOLL_WAIT
        | LINUX_TGKILL
        | LINUX_UTIMES
        | LINUX_MQ_OPEN
        | LINUX_MQ_UNLINK
        | LINUX_KEXEC_LOAD
        | LINUX_WAITID
        | LINUX_ADD_KEY
        | LINUX_REQUEST_KEY
        | LINUX_KEYCTL
        | LINUX_IOPRIO_SET
        | LINUX_IOPRIO_GET
        | LINUX_INOTIFY_INIT
        | LINUX_INOTIFY_ADD_WATCH
        | LINUX_INOTIFY_RM_WATCH
        | LINUX_OPENAT
        | LINUX_MKDIRAT
        | LINUX_UNLINKAT
        | LINUX_RENAMEAT
        | LINUX_NEWFSTATAT
        | LINUX_READLINKAT
        | LINUX_FCHMODAT
        | LINUX_FACCESSAT
        | LINUX_PSELECT6
        | LINUX_PPOLL
        | LINUX_UNSHARE
        | LINUX_SPLICE
        | LINUX_TEE
        | LINUX_SYNC_FILE_RANGE
        | LINUX_UTIMENSAT
        | LINUX_EPOLL_PWAIT
        | LINUX_SIGNALFD
        | LINUX_SIGNALFD4
        | LINUX_TIMERFD_CREATE
        | LINUX_TIMERFD_SETTIME
        | LINUX_TIMERFD_GETTIME
        | LINUX_EVENTFD
        | LINUX_EVENTFD2
        | LINUX_FALLOCATE
        | LINUX_ACCEPT4
        | LINUX_EPOLL_CREATE1
        | LINUX_DUP3
        | LINUX_PIPE2
        | LINUX_PREADV
        | LINUX_PWRITEV
        | LINUX_PERF_EVENT_OPEN
        | LINUX_RECVMMSG
        | LINUX_FANOTIFY_INIT
        | LINUX_FANOTIFY_MARK
        | LINUX_PRLIMIT64
        | LINUX_NAME_TO_HANDLE_AT
        | LINUX_OPEN_BY_HANDLE_AT
        | LINUX_CLOCK_ADJTIME
        | LINUX_SYNCFS
        | LINUX_SENDMMSG
        | LINUX_SETNS
        | LINUX_GETCPU
        | LINUX_PROCESS_VM_READV
        | LINUX_PROCESS_VM_WRITEV
        | LINUX_KCMP
        | LINUX_CREATE_MODULE
        | LINUX_INIT_MODULE
        | LINUX_DELETE_MODULE
        | LINUX_FINIT_MODULE
        | LINUX_SCHED_SETATTR
        | LINUX_SCHED_GETATTR
        | LINUX_RENAMEAT2
        | LINUX_SECCOMP
        | LINUX_GETRANDOM
        | LINUX_MEMFD_CREATE
        | LINUX_KEXEC_FILE_LOAD
        | LINUX_BPF
        | LINUX_EXECVEAT
        | LINUX_USERFAULTFD
        | LINUX_MEMBARRIER
        | LINUX_MLOCK2
        | LINUX_COPY_FILE_RANGE
        | LINUX_PREADV2
        | LINUX_PWRITEV2
        | LINUX_PKEY_MPROTECT
        | LINUX_PKEY_ALLOC
        | LINUX_PKEY_FREE
        | LINUX_STATX
        | LINUX_IO_PGETEVENTS
        | LINUX_RSEQ
        | LINUX_PIDFD_SEND_SIGNAL
        | LINUX_IO_URING_SETUP
        | LINUX_IO_URING_ENTER
        | LINUX_IO_URING_REGISTER
        | LINUX_OPEN_TREE
        | LINUX_MOVE_MOUNT
        | LINUX_FSOPEN
        | LINUX_FSCONFIG
        | LINUX_FSMOUNT
        | LINUX_FSPICK
        | LINUX_PIDFD_OPEN
        | LINUX_CLONE3
        | LINUX_CLOSE_RANGE
        | LINUX_OPENAT2
        | LINUX_PIDFD_GETFD
        | LINUX_FACCESSAT2
        | LINUX_PROCESS_MADVISE
        | LINUX_EPOLL_PWAIT2
        | LINUX_QUOTACTL_FD
        | LINUX_LANDLOCK_CREATE_RULESET
        | LINUX_LANDLOCK_ADD_RULE
        | LINUX_LANDLOCK_RESTRICT_SELF
        | LINUX_PROCESS_MRELEASE
        | LINUX_FUTEX_WAITV
        | LINUX_SET_MEMPOLICY_HOME_NODE
        | LINUX_CACHESTAT
        | LINUX_FCHMODAT2
        | LINUX_MSEAL
        | LINUX_SETXATTR
        | LINUX_LSETXATTR
        | LINUX_FSETXATTR
        | LINUX_GETXATTR
        | LINUX_LGETXATTR
        | LINUX_FGETXATTR
        | LINUX_LISTXATTR
        | LINUX_LLISTXATTR
        | LINUX_FLISTXATTR
        | LINUX_REMOVEXATTR
        | LINUX_LREMOVEXATTR
        | LINUX_FREMOVEXATTR => true,
        _ => false,
    }
}
