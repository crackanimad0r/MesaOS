// linux9.c - Test low-effort syscalls (one at a time)
#define SYS_write 1
#define SYS_open 2
#define SYS_close 3
#define SYS_exit 60
#define SYS_getpeername 52
#define SYS_getsockname 51
#define SYS_times 100
#define SYS_clock_getres 229
#define SYS_syslog 103
#define SYS_chroot 161
#define SYS_setreuid 113
#define SYS_setregid 114
#define SYS_getresuid 118
#define SYS_getresgid 120
#define SYS_getpgid 121
#define SYS_getsid 124
#define SYS_getgroups 115
#define SYS_setgroups 116
#define SYS_setresuid 117
#define SYS_setresgid 119
#define SYS_setfsuid 122
#define SYS_setfsgid 123
#define SYS_utime 132
#define SYS_mknod 133
#define SYS_fchmod 91
#define SYS_fchmodat 268
#define SYS_faccessat 269
#define SYS_sched_setattr 314
#define SYS_sched_getattr 315
#define SYS_fchdir 81
#define SYS_getcwd 79
#define SYS_setpriority 140
#define SYS_getpriority 141
#define SYS_sched_rr_get_interval 148
#define SYS_membarrier 324
#define SYS_setpgid 109

static long syscall(long n, long a1, long a2, long a3, long a4, long a5) {
    long ret;
    register long r10 asm("r10") = a4;
    register long r8 asm("r8") = a5;
    asm volatile("syscall"
        : "=a"(ret)
        : "a"(n), "D"(a1), "S"(a2), "d"(a3), "r"(r10), "r"(r8)
        : "rcx", "r11", "memory");
    return ret;
}

static void print(const char *s) {
    int len = 0;
    while (s[len]) len++;
    syscall(SYS_write, 1, (long)s, len, 0, 0);
}

static void pnum(long n) {
    char buf[20];
    int i = 0;
    if (n < 0) { print("-"); n = -n; }
    if (n == 0) { print("0"); return; }
    while (n > 0 && i < 18) { buf[i++] = '0' + (n % 10); n /= 10; }
    buf[i] = 0;
    for (int j = 0; j < i/2; j++) { char t = buf[j]; buf[j] = buf[i-1-j]; buf[i-1-j] = t; }
    print(buf);
}

#define TEST(n, expr) do { \
    long r = (expr); \
    print("  "); print(n); print("="); pnum(r); \
    if (r == 0) print(" OK"); \
    print("\n"); \
} while(0)

void _start() {
    print("Linux9 start\n");
    TEST("getpeername", syscall(52, 0, 0, 0, 0, 0));
    print("A1\n");
    { long b[4]; TEST("getsockname", syscall(51, 0, (long)b, 16, 0, 0)); }
    print("A2\n");
    { long b[4]; TEST("times", syscall(100, (long)b, 0, 0, 0, 0)); }
    print("A3\n");
    { long b[2]; TEST("clock_getres", syscall(229, 0, (long)b, 0, 0, 0)); }
    print("A4\n");
    TEST("syslog", syscall(103, 0, 0, 0, 0, 0));
    print("A5\n");
    TEST("chroot", syscall(161, (long)"/", 0, 0, 0, 0));
    print("A6\n");
    TEST("setpgid", syscall(109, 0, 0, 0, 0, 0));
    print("A7\n");
    TEST("setreuid", syscall(113, -1, -1, 0, 0, 0));
    print("A8\n");
    TEST("setregid", syscall(114, -1, -1, 0, 0, 0));
    print("A9\n");
    { unsigned int ids[3]; TEST("getresuid", syscall(118, (long)ids, 0, 0, 0, 0)); }
    print("A10\n");
    { unsigned int ids[3]; TEST("getresgid", syscall(120, (long)ids, 0, 0, 0, 0)); }
    print("A11\n");
    TEST("getpgid", syscall(121, 0, 0, 0, 0, 0));
    print("A12\n");
    TEST("getsid", syscall(124, 0, 0, 0, 0, 0));
    print("A13\n");
    TEST("getgroups", syscall(115, 0, 0, 0, 0, 0));
    print("A14\n");
    TEST("setgroups", syscall(116, 0, 0, 0, 0, 0));
    print("A15\n");
    TEST("setresuid", syscall(117, -1, -1, -1, 0, 0));
    print("A16\n");
    TEST("setresgid", syscall(119, -1, -1, -1, 0, 0));
    print("A17\n");
    TEST("setfsuid", syscall(122, 0, 0, 0, 0, 0));
    print("A18\n");
    TEST("setfsgid", syscall(123, 0, 0, 0, 0, 0));
    print("A19\n");
    TEST("utime", syscall(132, (long)"/", 0, 0, 0, 0));
    print("A20\n");
    TEST("mknod", syscall(133, (long)"/tmp/testnode", 0, 0, 0, 0));
    print("A21\n");
    TEST("fchmod", syscall(91, 999, 0, 0, 0, 0));
    print("A22\n");
    TEST("fchmodat", syscall(268, -100, (long)"/", 0, 0, 0));
    print("A23\n");
    TEST("faccessat", syscall(269, -100, (long)"/", 0, 0, 0));
    print("A24\n");
    TEST("sched_setattr", syscall(314, 0, 0, 0, 0, 0));
    print("A25\n");
    TEST("sched_getattr", syscall(315, 0, 0, 0, 0, 0));
    print("A26\n");
    TEST("membarrier", syscall(324, 0, 0, 0, 0, 0));
    print("A27\n");
    TEST("setpriority", syscall(140, 0, 0, 0, 0, 0));
    print("A28\n");
    TEST("getpriority", syscall(141, 0, 0, 0, 0, 0));
    print("A29\n");
    // fchdir test
    { long fd = syscall(2, (long)"/tmp", 0, 0, 0, 0);
      if (fd >= 0) {
        TEST("fchdir", syscall(81, fd, 0, 0, 0, 0));
        syscall(3, fd, 0, 0, 0, 0);
      }
    }
    print("A30\n");
    { long b[2]; TEST("sched_rr_get_interval", syscall(148, 0, (long)b, 0, 0, 0)); }
    print("A31\n");
    print("Linux9: ALL OK!\n");
    syscall(SYS_exit, 0, 0, 0, 0, 0);
}
