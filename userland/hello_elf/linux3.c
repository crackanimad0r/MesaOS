void _start(void) {
    char buf[256];
    char *buf_ptr = buf;
    long ret;
    __asm__ volatile (
        "mov $79, %%rax\n"
        "mov %1, %%rdi\n"
        "mov $256, %%rsi\n"
        "syscall\n"
        "mov %%rax, %0"
        : "=r"(ret) : "r"(buf_ptr) : "rax", "rdi", "rsi"
    );

    if (ret > 0) {
        __asm__ volatile (
            "mov $1, %%rax\n"
            "mov $1, %%rdi\n"
            "mov %0, %%rsi\n"
            "mov %1, %%rdx\n"
            "syscall"
            :: "r"(buf_ptr), "r"(ret) : "rax", "rdi", "rsi", "rdx"
        );
    }

    __asm__ volatile (
        "mov $60, %%rax\n"
        "xor %%rdi, %%rdi\n"
        "syscall"
        ::: "rax", "rdi"
    );
}
