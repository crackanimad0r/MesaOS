// linux6.c - Test getppid + wait4
// Compile: gcc -nostdlib -static-pie -fPIE -T linker.ld -o linux6.elf linux6.c

#define SYS_write 1
#define SYS_getppid 110
#define SYS_wait4 61
#define SYS_exit 60
#define SYS_getpid 39
#define SYS_gettid 186

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

#define WNOHANG 1

static void print_str(const char *s) {
    int len = 0;
    while (s[len]) len++;
    syscall(SYS_write, 1, (long)s, len, 0, 0);
}

static char hexbuf[20];
static char *to_hex(long n) {
    int i = 0;
    int neg = 0;
    if (n < 0) { neg = 1; n = -n; }
    if (n == 0) { hexbuf[0] = '0'; hexbuf[1] = 0; return hexbuf; }
    while (n > 0 && i < 16) {
        int d = n % 16;
        hexbuf[i++] = d < 10 ? '0' + d : 'a' + d - 10;
        n /= 16;
    }
    if (neg) hexbuf[i++] = '-';
    hexbuf[i] = 0;
    // reverse
    for (int j = 0; j < i/2; j++) {
        char t = hexbuf[j]; hexbuf[j] = hexbuf[i-1-j]; hexbuf[i-1-j] = t;
    }
    return hexbuf;
}

void _start() {
    print_str("Linux6: getppid test\n");

    long ppid = syscall(SYS_getppid, 0,0,0,0,0);
    print_str("  ppid = ");
    print_str(to_hex(ppid));
    print_str("\n");

    long pid = syscall(SYS_getpid, 0,0,0,0,0);
    print_str("  pid  = ");
    print_str(to_hex(pid));
    print_str("\n");

    // wait4(-1, &status, WNOHANG, NULL) -> should return -ECHILD (-10)
    // since there are no children
    int status = 0;
    long wret = syscall(SYS_wait4, -1, (long)&status, WNOHANG, 0, 0);
    print_str("  wait4(-1, WNOHANG) = ");
    print_str(to_hex(wret));
    print_str("\n");

    print_str("Linux6: OK!\n");
    syscall(SYS_exit, 0,0,0,0,0);
}
