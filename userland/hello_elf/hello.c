/*
 * Programa de ejemplo para Mesa OS (Ring 3).
 * Usa las mismas convenciones de syscall que Linux x86_64:
 *   rax = número de syscall, rdi, rsi, rdx = argumentos.
 * Mesa OS: SYS_write=1, SYS_exit=60.
 */

static const char msg[] = "Hello from Mesa OS!\n";

void _start(void) {
    /* write(1, msg, sizeof(msg)-1) */
    __asm__ volatile (
        "mov $1, %%rax\n"
        "mov $1, %%rdi\n"
        "lea %0, %%rsi\n"
        "mov $20, %%rdx\n"
        "syscall"
        : : "m"(msg) : "rax", "rdi", "rsi", "rdx"
    );
    /* exit(0) */
    __asm__ volatile (
        "mov $60, %%rax\n"
        "xor %%rdi, %%rdi\n"
        "syscall"
        : : : "rax", "rdi"
    );
}
