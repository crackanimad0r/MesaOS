// linux9_mini.c - Minimal test
#define SYS_write 1
#define SYS_exit 60

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

void _start() {
    print_str("Mini9: OK\n");
    syscall(SYS_exit, 0, 0, 0, 0, 0);
}
