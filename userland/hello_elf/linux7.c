// linux7.c - Test UDP socket
// gcc -nostdlib -static-pie -fPIE -T linker.ld -o linux7.elf linux7.c

#define SYS_write 1
#define SYS_socket 41
#define SYS_sendto 44
#define SYS_recvfrom 45
#define SYS_bind 49
#define SYS_close 3
#define SYS_exit 60

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

struct sockaddr_in {
    short sin_family;
    unsigned short sin_port;
    unsigned int sin_addr;
    char sin_zero[8];
};

static void print_str(const char *s) {
    int len = 0;
    while (s[len]) len++;
    syscall(SYS_write, 1, (long)s, len, 0, 0);
}

static char hexbuf[20];
static char *to_hex(long n) {
    int i = 0, neg = 0;
    if (n < 0) { neg = 1; n = -n; }
    if (n == 0) { hexbuf[0] = '0'; hexbuf[1] = 0; return hexbuf; }
    while (n > 0 && i < 16) {
        int d = n % 16;
        hexbuf[i++] = d < 10 ? '0' + d : 'a' + d - 10;
        n /= 16;
    }
    if (neg) hexbuf[i++] = '-';
    hexbuf[i] = 0;
    for (int j = 0; j < i/2; j++) {
        char t = hexbuf[j]; hexbuf[j] = hexbuf[i-1-j]; hexbuf[i-1-j] = t;
    }
    return hexbuf;
}

void _start() {
    print_str("Linux7: UDP socket test\n");

    long fd = syscall(SYS_socket, AF_INET, SOCK_DGRAM, 0, 0, 0);
    print_str("  socket fd = ");
    print_str(to_hex(fd));
    print_str("\n");

    // Bind to port 12345
    struct sockaddr_in local;
    local.sin_family = AF_INET;
    local.sin_port = htons(12345);
    local.sin_addr = htonl(0);
    for (int i = 0; i < 8; i++) local.sin_zero[i] = 0;

    long bind_ret = syscall(SYS_bind, fd, (long)&local, 16, 0, 0);
    print_str("  bind = ");
    print_str(to_hex(bind_ret));
    print_str("\n");

    if (bind_ret < 0) {
        print_str("Linux7: bind failed!\n");
        syscall(SYS_exit, 1, 0, 0, 0, 0);
    }

    // Sendto localhost:9999
    struct sockaddr_in dest;
    dest.sin_family = AF_INET;
    dest.sin_port = htons(9999);
    dest.sin_addr = htonl(0x0A00020F); // 10.0.2.15
    for (int i = 0; i < 8; i++) dest.sin_zero[i] = 0;

    const char *msg = "Hello from MesaOS UDP!";
    long sent = syscall(SYS_sendto, fd, (long)msg, 22, 0, (long)&dest);
    print_str("  sendto = ");
    print_str(to_hex(sent));
    print_str("\n");

    // recvfrom will get EAGAIN since nothing is sent back to us
    char recv_buf[64];
    long recvd = syscall(SYS_recvfrom, fd, (long)recv_buf, 64, 0, 0);
    print_str("  recvfrom = ");
    print_str(to_hex(recvd));
    print_str("\n");

    syscall(SYS_close, fd, 0, 0, 0, 0);
    print_str("Linux7: OK!\n");
    syscall(SYS_exit, 0, 0, 0, 0, 0);
}
