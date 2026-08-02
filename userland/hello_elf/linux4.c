struct utsname {
    char sysname[65];
    char nodename[65];
    char release[65];
    char version[65];
    char machine[65];
    char domainname[65];
};

void _start(void) {
    struct utsname name;
    char *name_ptr = (char *)&name;
    long ret;
    __asm__ volatile (
        "mov $63, %%rax\n"
        "mov %1, %%rdi\n"
        "syscall\n"
        "mov %%rax, %0"
        : "=r"(ret) : "r"(name_ptr) : "rax", "rdi"
    );

    if (ret == 0) {
        char *sysname = name.sysname;
        long len = 0;
        while (len < 64 && sysname[len] != 0) len++;
        if (len > 0) {
            __asm__ volatile (
                "mov $1, %%rax\n"
                "mov $1, %%rdi\n"
                "mov %0, %%rsi\n"
                "mov %1, %%rdx\n"
                "syscall"
                :: "r"(sysname), "r"(len) : "rax", "rdi", "rsi", "rdx"
            );
        }
    }

    __asm__ volatile (
        "mov $60, %%rax\n"
        "xor %%rdi, %%rdi\n"
        "syscall"
        ::: "rax", "rdi"
    );
}
