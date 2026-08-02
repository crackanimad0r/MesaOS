// linux8.c - Test new medium-effort syscalls
// gcc -nostdlib -static-pie -fPIE -T linker.ld -o linux8.elf linux8.c

#define SYS_write 1
#define SYS_open 2
#define SYS_close 3
#define SYS_stat 4
#define SYS_fstat 5
#define SYS_lseek 8
#define SYS_mmap 9
#define SYS_brk 12
#define SYS_select 23
#define SYS_sched_yield 24
#define SYS_dup 32
#define SYS_sendfile 40
#define SYS_poll 7
#define SYS_flock 73
#define SYS_truncate 76
#define SYS_rename 82
#define SYS_mkdir 83
#define SYS_link 86
#define SYS_unlink 87
#define SYS_symlink 88
#define SYS_readlink 89
#define SYS_chmod 90
#define SYS_chown 92
#define SYS_lchown 94
#define SYS_statfs 137
#define SYS_fstatfs 138
#define SYS_exit 60
#define SYS_clock_gettime 228
#define SYS_renameat 264

#define AT_FDCWD (-100)

#define O_RDONLY 0
#define O_WRONLY 1
#define O_CREAT 0x40
#define O_TRUNC 0x200

#define AF_INET 2
#define SOCK_DGRAM 2

#define htons(x) ((((x) >> 8) & 0xff) | (((x) & 0xff) << 8))
#define htonl(x) __builtin_bswap32(x)

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

static void print_str(const char *s) {
    int len = 0;
    while (s[len]) len++;
    syscall(SYS_write, 1, (long)s, len, 0, 0);
}

static void print_hex(long n) {
    char buf[20];
    int i = 0, neg = 0;
    if (n < 0) { neg = 1; n = -n; }
    if (n == 0) { buf[0] = '0'; buf[1] = 0; print_str(buf); return; }
    while (n > 0 && i < 16) {
        int d = n % 16;
        buf[i++] = d < 10 ? '0' + d : 'a' + d - 10;
        n /= 16;
    }
    if (neg) buf[i++] = '-';
    buf[i] = 0;
    for (int j = 0; j < i/2; j++) {
        char t = buf[j]; buf[j] = buf[i-1-j]; buf[i-1-j] = t;
    }
    print_str(buf);
}

static void test_pass(const char *name) {
    print_str("  OK ");
    print_str(name);
    print_str("\n");
}

static void test_fail(const char *name) {
    print_str("  FAIL ");
    print_str(name);
    print_str("\n");
}

#define TEST(name, expr) do { \
    long r = (expr); \
    if (r == 0) test_pass(name); \
    else { print_str("  "); print_str(name); print_str(" returned "); print_hex(r); print_str("\n"); } \
} while(0)

void _start() {
    print_str("Linux8: Testing new syscalls\n");

    // 1. flock - should return 0 (no-op)
    {
        long r = syscall(SYS_flock, 999, 0, 0, 0, 0);
        TEST("flock(EBADF)", r);
    }

    // 2. statfs
    {
        char buf[120];
        long r = syscall(SYS_statfs, (long)"/", (long)buf, 0, 0, 0);
        print_str("  statfs = ");
        print_hex(r);
        print_str("\n");
        // Check f_type field (first 8 bytes)
        unsigned long f_type = *(unsigned long*)buf;
        print_str("  f_type = ");
        print_hex(f_type);
        print_str("\n");
    }

    // 3. chown / lchown
    {
        // Create a test file first
        long fd = syscall(SYS_open, (long)"/tmp/chown_test", O_CREAT|O_WRONLY|O_TRUNC, 0, 0, 0);
        if (fd >= 0) {
            syscall(SYS_close, fd, 0, 0, 0, 0);
        }
        long r = syscall(SYS_chown, (long)"/tmp/chown_test", 1000, 1000, 0, 0);
        TEST("chown", r);
        r = syscall(SYS_lchown, (long)"/tmp/chown_test", 0, 0, 0, 0);
        TEST("lchown", r);
    }

    // 4. renameat
    {
        long r = syscall(SYS_renameat, AT_FDCWD, (long)"/tmp/chown_test", AT_FDCWD, (long)"/tmp/renamed_test", 0);
        TEST("renameat", r);
    }

    // 5. symlink + readlink (now reads actual symlink target)
    {
        long r = syscall(SYS_symlink, (long)"/tmp/renamed_test", (long)"/tmp/mylink", 0, 0, 0);
        TEST("symlink", r);
        char linkbuf[64];
        r = syscall(SYS_readlink, (long)"/tmp/mylink", (long)linkbuf, 64, 0, 0);
        if (r > 0) {
            linkbuf[r] = 0;
            print_str("  readlink target = ");
            print_str(linkbuf);
            print_str("\n");
        } else {
            print_str("  readlink returned ");
            print_hex(r);
            print_str("\n");
        }
    }

    // 6. link (hard link)
    {
        long r = syscall(SYS_link, (long)"/tmp/renamed_test", (long)"/tmp/hardlink_test", 0, 0, 0);
        TEST("link", r);
    }

    // 7. sendfile (read from /tmp/renamed_test, write to stdout)
    {
        long in_fd = syscall(SYS_open, (long)"/tmp/renamed_test", O_RDONLY, 0, 0, 0);
        if (in_fd >= 0) {
            long out_fd = 1; // stdout
            long r = syscall(SYS_sendfile, out_fd, in_fd, 0, 64, 0);
            print_str("  sendfile returned ");
            print_hex(r);
            print_str("\n");
            syscall(SYS_close, in_fd, 0, 0, 0, 0);
        }
    }

    // 8. poll (self-pipe test)
    {
        int p[2];
        long r = syscall(22, (long)p, 0, 0, 0, 0); // pipe
        if (r == 0) {
            // pollfd struct: { fd, events, revents }
            struct { int fd; short events; short revents; } pfds[1];
            pfds[0].fd = p[0]; // read end
            pfds[0].events = 1; // POLLIN
            pfds[0].revents = 0;
            r = syscall(SYS_poll, (long)pfds, 1, 0, 0, 0); // timeout=0
            print_str("  poll(pipe, timeout=0) = ");
            print_hex(r);
            print_str(" revents=");
            print_hex(pfds[0].revents);
            print_str("\n");

            // Write to pipe to make it readable
            syscall(SYS_write, p[1], (long)"x", 1, 0, 0);
            pfds[0].revents = 0;
            r = syscall(SYS_poll, (long)pfds, 1, 0, 0, 0);
            print_str("  poll(pipe+data, timeout=0) = ");
            print_hex(r);
            print_str(" revents=");
            print_hex(pfds[0].revents);
            print_str("\n");
            syscall(SYS_close, p[0], 0, 0, 0, 0);
            syscall(SYS_close, p[1], 0, 0, 0, 0);
        } else {
            print_str("  pipe failed: ");
            print_hex(r);
            print_str("\n");
        }
    }

    // 9. select (simple timeout=0 test)
    {
        long tv[2] = {0, 0}; // timeout = 0
        long rfds[2] = {0, 0}; // 1024-bit fd_set, all zero
        rfds[0] |= 1; // set fd 0 (stdin)
        long r = syscall(SYS_select, 1, (long)rfds, 0, 0, (long)tv);
        print_str("  select(stdin, timeout=0) = ");
        print_hex(r);
        print_str("\n");
    }

    // Cleanup
    syscall(SYS_unlink, (long)"/tmp/mylink", 0, 0, 0, 0);
    syscall(SYS_unlink, (long)"/tmp/hardlink_test", 0, 0, 0, 0);
    syscall(SYS_unlink, (long)"/tmp/renamed_test", 0, 0, 0, 0);

    print_str("Linux8: Done!\n");
    syscall(SYS_exit, 0, 0, 0, 0, 0);
}
