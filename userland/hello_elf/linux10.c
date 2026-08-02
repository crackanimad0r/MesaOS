// linux10.c - Test eventfd, signalfd, timerfd, epoll
#define SYS_write 1
#define SYS_read 0
#define SYS_close 3
#define SYS_exit 60
#define SYS_poll 7
#define SYS_eventfd2 290
#define SYS_signalfd4 289
#define SYS_timerfd_create 283
#define SYS_timerfd_settime 286
#define SYS_timerfd_gettime 287
#define SYS_epoll_create1 291
#define SYS_epoll_ctl 233
#define SYS_epoll_wait 232

#define EFD_SEMAPHORE 1
#define EFD_NONBLOCK 0x800
#define TFD_NONBLOCK 0x800
#define EPOLL_CTL_ADD 1
#define EPOLL_CTL_DEL 2
#define EPOLL_CTL_MOD 3
#define EPOLLIN 0x001
#define EPOLLOUT 0x004
#define POLLIN 0x001
#define SF_NONBLOCK 0x800

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

struct epoll_event {
    unsigned int events;
    unsigned long data;
} __attribute__((packed));

struct pollfd {
    int fd;
    short events;
    short revents;
};

struct itimerspec {
    unsigned long it_interval_sec;
    unsigned long it_interval_nsec;
    unsigned long it_value_sec;
    unsigned long it_value_nsec;
};

#define TEST(n, expr) do { \
    long r = (expr); \
    print("  "); print(n); print("="); pnum(r); \
    if (r >= 0) print(" OK"); \
    print("\n"); \
} while(0)

void _start() {
    print("Linux10 start\n");

    // === EVENTFD ===
    print("A1\n");
    long efd = syscall(SYS_eventfd2, 5, 0, 0, 0, 0);
    TEST("eventfd", efd);

    print("A2\n");
    unsigned long ev_val = 0;
    long r = syscall(SYS_read, efd, (long)&ev_val, 8, 0, 0);
    TEST("eventfd_read", r);

    print("A3\n");
    ev_val = 3;
    r = syscall(SYS_write, efd, (long)&ev_val, 8, 0, 0);
    TEST("eventfd_write", r);

    print("A4\n");
    ev_val = 0;
    r = syscall(SYS_read, efd, (long)&ev_val, 8, 0, 0);
    TEST("eventfd_read2", r);

    print("A5\n");
    long efd_sem = syscall(SYS_eventfd2, 5, EFD_SEMAPHORE, 0, 0, 0);
    TEST("eventfd_sem", efd_sem);

    print("A6\n");
    ev_val = 0;
    r = syscall(SYS_read, efd_sem, (long)&ev_val, 8, 0, 0);
    TEST("eventfd_sem_read", r);

    print("A7\n");
    TEST("eventfd_close", syscall(SYS_close, efd, 0, 0, 0, 0));
    TEST("eventfd_sem_close", syscall(SYS_close, efd_sem, 0, 0, 0, 0));

    // === SIGNALFD ===
    print("A8\n");
    unsigned long sigmask[2] = {0, 0};
    long sfd = syscall(SYS_signalfd4, -1, (long)sigmask, 16, SF_NONBLOCK, 0);
    TEST("signalfd", sfd);

    print("A9\n");
    unsigned char siginfo[128];
    r = syscall(SYS_read, sfd, (long)siginfo, 128, 0, 0);
    TEST("signalfd_read", r);

    print("A10\n");
    TEST("signalfd_close", syscall(SYS_close, sfd, 0, 0, 0, 0));

    // === TIMERFD ===
    print("A11\n");
    long tfd = syscall(SYS_timerfd_create, 0, 0, 0, 0, 0);
    TEST("timerfd_create", tfd);

    print("A12\n");
    struct itimerspec ts_new;
    ts_new.it_interval_sec = 0;
    ts_new.it_interval_nsec = 0;
    ts_new.it_value_sec = 0;
    ts_new.it_value_nsec = 50000000;
    struct itimerspec ts_old;
    r = syscall(SYS_timerfd_settime, tfd, 0, (long)&ts_new, (long)&ts_old, 0);
    TEST("timerfd_settime", r);

    print("A13\n");
    r = syscall(SYS_timerfd_gettime, tfd, (long)&ts_old, 0, 0, 0);
    TEST("timerfd_gettime", r);

    print("A14\n");
    unsigned long tval = 0;
    r = syscall(SYS_read, tfd, (long)&tval, 8, 0, 0);
    TEST("timerfd_read_now", r);

    if (r == -11) {
        for (int i = 0; i < 20; i++) {
            asm volatile("pause");
        }
        tval = 0;
        r = syscall(SYS_read, tfd, (long)&tval, 8, 0, 0);
        TEST("timerfd_read_after", r);
    }

    print("A15\n");
    TEST("timerfd_close", syscall(SYS_close, tfd, 0, 0, 0, 0));

    // === EPOLL ===
    print("A16\n");
    long tfd2 = syscall(SYS_timerfd_create, 0, TFD_NONBLOCK, 0, 0, 0);
    TEST("timerfd2_create", tfd2);

    print("A17\n");
    long epfd = syscall(SYS_epoll_create1, 0, 0, 0, 0, 0);
    TEST("epoll_create1", epfd);

    print("A18\n");
    ts_new.it_value_sec = 0;
    ts_new.it_value_nsec = 50000000;
    ts_new.it_interval_sec = 0;
    ts_new.it_interval_nsec = 0;
    syscall(SYS_timerfd_settime, tfd2, 0, (long)&ts_new, 0, 0);

    print("A19\n");
    struct epoll_event ev;
    ev.events = EPOLLIN;
    ev.data = 42;
    r = syscall(SYS_epoll_ctl, epfd, EPOLL_CTL_ADD, tfd2, (long)&ev, 0);
    TEST("epoll_ctl_add", r);

    print("A20\n");
    struct epoll_event events[4];
    r = syscall(SYS_epoll_wait, epfd, (long)events, 4, 200, 0);
    TEST("epoll_wait", r);
    if (r > 0) {
        print("  events="); pnum(events[0].events); print(" data="); pnum(events[0].data); print("\n");
    }

    print("A21\n");
    r = syscall(SYS_epoll_ctl, epfd, EPOLL_CTL_DEL, tfd2, 0, 0);
    TEST("epoll_ctl_del", r);

    print("A22\n");
    ev.events = EPOLLIN;
    ev.data = 99;
    r = syscall(SYS_epoll_ctl, epfd, EPOLL_CTL_ADD, tfd2, (long)&ev, 0);
    TEST("epoll_ctl_add2", r);
    r = syscall(SYS_epoll_wait, epfd, (long)events, 4, 100, 0);
    TEST("epoll_wait2", r);

    print("A23\n");
    ev.data = 77;
    r = syscall(SYS_epoll_ctl, epfd, EPOLL_CTL_MOD, tfd2, (long)&ev, 0);
    TEST("epoll_ctl_mod", r);

    print("A24\n");
    TEST("epoll_close", syscall(SYS_close, epfd, 0, 0, 0, 0));
    TEST("timerfd2_close", syscall(SYS_close, tfd2, 0, 0, 0, 0));

    // === POLL with eventfd ===
    print("A25\n");
    long efd2 = syscall(SYS_eventfd2, 7, EFD_NONBLOCK, 0, 0, 0);
    TEST("eventfd_poll_fd", efd2);

    print("A26\n");
    struct pollfd pfds[1];
    pfds[0].fd = efd2;
    pfds[0].events = POLLIN;
    pfds[0].revents = 0;
    r = syscall(SYS_poll, (long)pfds, 1, 0, 0, 0);
    TEST("poll_eventfd", r);
    if (r > 0) {
        print("  revents="); pnum(pfds[0].revents); print("\n");
    }

    print("A27\n");
    TEST("eventfd2_close", syscall(SYS_close, efd2, 0, 0, 0, 0));

    print("Linux10: ALL OK!\n");
    syscall(SYS_exit, 0, 0, 0, 0, 0);
}
