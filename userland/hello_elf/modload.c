#define SYS_write 1
#define SYS_open 2
#define SYS_close 3
#define SYS_exit 60
#define SYS_finit_module 313
#define SYS_delete_module 176

static long syscall(long n, long a1, long a2, long a3, long a4, long a5)
{
    long ret;
    register long r10 asm("r10") = a4;
    register long r8 asm("r8") = a5;
    asm volatile("syscall"
        : "=a"(ret)
        : "a"(n), "D"(a1), "S"(a2), "d"(a3), "r"(r10), "r"(r8)
        : "rcx", "r11", "memory");
    return ret;
}

static void print(const char *s)
{
    int len = 0;
    const char *p = s;
    while (*p) { len++; p++; }
    syscall(SYS_write, 1, (long)s, len, 0, 0);
}

static void pnum(long n)
{
    char buf[20];
    int i = 0;
    if (n < 0) { print("-"); n = -n; }
    if (n == 0) { print("0"); return; }
    while (n > 0 && i < 18) { buf[i++] = '0' + (n % 10); n /= 10; }
    buf[i] = 0;
    int j;
    for (j = 0; j < i/2; j++) { char t = buf[j]; buf[j] = buf[i-1-j]; buf[i-1-j] = t; }
    print(buf);
}

void load_module(const char *path)
{
    long fd = syscall(SYS_open, (long)path, 0, 0, 0, 0);
    if (fd < 0) {
        print("modload: FAILED to open ");
        print(path);
        print(" (errno=");
        pnum(-fd);
        print(")\n");
        return;
    }
    print("modload: Opened ");
    print(path);
    print(", fd=");
    pnum(fd);
    print("\n");

    long ret = syscall(SYS_finit_module, fd, 0, 0, 0, 0);
    if (ret == 0) {
        print("modload: SUCCESS - ");
        print(path);
        print(" loaded!\n");
    } else {
        print("modload: FAILED - finit_module returned ");
        pnum(ret);
        print("\n");
    }

    syscall(SYS_close, fd, 0, 0, 0, 0);
}

void _start(void)
{
    print("modload: Starting kernel module loader (USB disabled)\n");

    // USB modules disabled — causing issues on real hardware
    // load_module("/inyect/bin/xhci-hcd.ko");
    // load_module("/inyect/bin/xhci-pci.ko");

    print("modload: All tests completed!\n");
    syscall(SYS_exit, 0, 0, 0, 0, 0);
}
