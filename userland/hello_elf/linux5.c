void _start(void) {
    char msg[] = "Linux!\n";
    long ret;
    __asm__ volatile (
        "mov $110, %%rax\n"
        "syscall\n"
        "mov %%rax, %0"
        : "=r"(ret) : : "rax"
    );

    if (ret >= 0) {
        __asm__ volatile (
            "mov $1, %%rax\n"
            "mov $1, %%rdi\n"
            "lea %0, %%rsi\n"
            "mov $7, %%rdx\n"
            "syscall"
            :: "m"(msg) : "rax", "rdi", "rsi", "rdx"
        );
    }

    __asm__ volatile (
        "mov $60, %%rax\n"
        "xor %%rdi, %%rdi\n"
        "syscall"
        ::: "rax", "rdi"
    );
}
