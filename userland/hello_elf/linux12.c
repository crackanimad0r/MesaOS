// linux12.c - Test high-effort syscalls (sendmsg, recvmsg, shutdown, TCP, fork)
#define SYS_write 1
#define SYS_read 0
#define SYS_close 3
#define SYS_exit 60
#define SYS_fork 57
#define SYS_vfork 58
#define SYS_clone 56
#define SYS_wait4 61
#define SYS_socket 41
#define SYS_bind 49
#define SYS_listen 50
#define SYS_accept 43
#define SYS_connect 42
#define SYS_sendto 44
#define SYS_recvfrom 45
#define SYS_sendmsg 46
#define SYS_recvmsg 47
#define SYS_shutdown 48
#define SYS_getpid 39
#define SYS_getppid 64

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
    if (r >= 0) print(" OK"); \
    print("\n"); \
} while(0)

void _start() {
    print("Linux12 start\n");

    // === TEST SENDMSG / RECVMSG / SHUTDOWN ===
    print("A1: socket(SOCK_DGRAM)\n");
    long fd = syscall(SYS_socket, 2, 2, 0, 0, 0);  // AF_INET=2, SOCK_DGRAM=2
    TEST("socket", fd);

    print("A2: bind\n");
    // bind to port 0 (ephemeral)
    unsigned char addr[16] = {2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
    addr[2] = 0; addr[3] = 0; // port 0
    long r = syscall(SYS_bind, fd, (long)addr, 16, 0, 0);
    TEST("bind", r);

    print("A3: sendmsg\n");
    // Set up msghdr for loopback to ourselves
    // msg_name (struct sockaddr_in): family=2, port=0, ip=0 (loopback)
    unsigned char name[16] = {2, 0, 0, 0, 0, 0, 0, 0, 127, 0, 0, 1, 0, 0, 0, 0};
    char msgdata[] = "HelloMsg!";
    // iovec
    unsigned long iov[2] = { (unsigned long)msgdata, 9 };
    // msghdr: msg_name ptr, msghdr padding
    unsigned long msg[7];
    msg[0] = (unsigned long)name;  // msg_name
    msg[1] = 0;  // pad
    msg[2] = (unsigned long)iov;   // msg_iov
    msg[3] = 1;                    // msg_iovlen
    msg[4] = 0;                    // msg_control
    msg[5] = 0;                    // msg_controllen
    msg[6] = 0;                    // msg_flags
    // Store namelen at offset 8 of the msghdr
    unsigned int *namelen = (unsigned int *)((unsigned long)msg + 8);
    *namelen = 16;
    TEST("sendmsg", syscall(SYS_sendmsg, fd, (long)msg, 0, 0, 0));

    // Actually sendmsg may fail because we're sending to ourselves
    // The important thing is that it doesn't crash and returns something

    print("A4: recvmsg\n");
    char rbuf[64];
    unsigned long riov[2] = { (unsigned long)rbuf, 64 };
    unsigned long rmsg[7] = {0, 0, (unsigned long)riov, 1, 0, 0, 0};
    *(unsigned int *)((unsigned long)rmsg + 8) = 16;
    r = syscall(SYS_recvmsg, fd, (long)rmsg, 0, 0, 0);
    TEST("recvmsg", r);

    print("A5: shutdown\n");
    r = syscall(SYS_shutdown, fd, 0, 0, 0, 0);
    TEST("shutdown", r);

    print("A6: close socket\n");
    syscall(SYS_close, fd, 0, 0, 0, 0);

    // === TEST TCP SOCKET ===
    print("B1: socket(SOCK_STREAM)\n");
    long tfd = syscall(SYS_socket, 2, 1, 0, 0, 0);  // AF_INET=2, SOCK_STREAM=1
    TEST("tcp_socket", tfd);

    print("B2: bind TCP\n");
    unsigned char taddr[16] = {2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
    taddr[2] = 0x1F; taddr[3] = 0x90; // port 8000
    r = syscall(SYS_bind, tfd, (long)taddr, 16, 0, 0);
    TEST("tcp_bind", r);

    print("B3: listen\n");
    r = syscall(SYS_listen, tfd, 5, 0, 0, 0);
    TEST("listen", r);

    print("B4: listen without bind (should fail)\n");
    long tfd2 = syscall(SYS_socket, 2, 1, 0, 0, 0);
    TEST("socket2", tfd2);
    r = syscall(SYS_listen, tfd2, 5, 0, 0, 0);
    TEST("listen_no_bind", r);

    print("B5: accept (should block, skip in test)\n");
    print("  accept_skipped\n");

    print("B6: close listen socket\n");
    syscall(SYS_close, tfd, 0, 0, 0, 0);
    syscall(SYS_close, tfd2, 0, 0, 0, 0);

    // === TEST CONNECT ===
    print("C1: connect to unreachable (should timeout and fail)\n");
    long cfd = syscall(SYS_socket, 2, 1, 0, 0, 0);
    TEST("connect_socket", cfd);
    unsigned char caddr[16] = {2, 0, 0, 0, 192, 168, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0};
    caddr[2] = 0x00; caddr[3] = 0x50; // port 80
    // This should timeout since we have no network
    // Use a short timeout by just trying once (non-blocking would be better)
    // For now just verify connect returns an error
    r = syscall(SYS_connect, cfd, (long)caddr, 16, 0, 0);
    TEST("connect", r);
    syscall(SYS_close, cfd, 0, 0, 0, 0);

    // === TEST FORK ===
    print("D1: fork\n");
    long pid = syscall(SYS_fork, 0, 0, 0, 0, 0);
    TEST("fork", pid);
    if (pid < 0) {
        print("  fork failed, skipping rest\n");
    } else if (pid == 0) {
        // Child process
        print("  CHILD: forked ok! my_pid=");
        pnum(syscall(SYS_getpid, 0,0,0,0,0));
        print(" parent_pid=");
        pnum(syscall(SYS_getppid, 0,0,0,0,0));
        print("\n");
        // Test some syscalls in child
        print("  CHILD: write test\n");
        long child = syscall(SYS_getpid, 0,0,0,0,0);
        print("  CHILD: getpid="); pnum(child); print("\n");
        // Exit with status 42
        print("  CHILD: exiting with status 42\n");
        syscall(SYS_exit, 42, 0, 0, 0, 0);
    } else {
        // Parent process
        print("  PARENT: child pid="); pnum(pid); print("\n");
        // Wait for child
        long wstatus = 0;
        long wp = syscall(SYS_wait4, pid, (long)&wstatus, 0, 0, 0);
        print("  PARENT: wait4 returned pid="); pnum(wp); print("\n");
        int exit_code = (wstatus >> 8) & 0xFF;
        print("  PARENT: child exit code="); pnum(exit_code); print("\n");
    }

    // === TEST VFORK ===
    print("D2: vfork (calls fork in our impl)\n");
    pid = syscall(SYS_vfork, 0, 0, 0, 0, 0);
    TEST("vfork", pid);
    if (pid == 0) {
        print("  VCHILD: exiting\n");
        syscall(SYS_exit, 0, 0, 0, 0, 0);
    } else if (pid > 0) {
        long ws = 0;
        syscall(SYS_wait4, pid, (long)&ws, 0, 0, 0);
        print("  VPARENT: done\n");
    }

    print("Linux12: ALL TESTS COMPLETED\n");
    syscall(SYS_exit, 0, 0, 0, 0, 0);
}
