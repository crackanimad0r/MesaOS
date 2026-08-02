static const char path[] = "/README.md";
static const char ok_msg[] = "OK: Linux compat layer works!\n";
static char buf[512];

void _start(void) {
    long fd;
    long n;

    /* 1. open("/README.md", 0) -> syscall 2 */
    __asm__ volatile (
        "mov $2, %%rax\n"
        "mov %1, %%rdi\n"   // Usamos 'r' para cargar la dirección en RDI
        "xor %%rsi, %%rsi\n"
        "xor %%rdx, %%rdx\n"
        "syscall\n"
        "mov %%rax, %0"
        : "=r"(fd) : "r"(path) : "rax", "rdi", "rsi", "rdx"
    );

    if (fd >= 0) {
        /* 2. read(fd, buf, 512) -> syscall 0 */
        __asm__ volatile (
            "mov $0, %%rax\n"
            "mov %1, %%rdi\n"
            "mov %2, %%rsi\n" // Pasamos la dirección de buf
            "mov $512, %%rdx\n"
            "syscall\n"
            "mov %%rax, %0"
            : "=r"(n) : "r"(fd), "r"(buf) : "rax", "rdi", "rsi", "rdx"
        );

        /* 3. write(1, buf, n) -> syscall 1 */
        if (n > 0) {
            __asm__ volatile (
                "mov $1, %%rax\n"
                "mov $1, %%rdi\n"
                "mov %0, %%rsi\n"
                "mov %1, %%rdx\n"
                "syscall"
                :: "r"(buf), "r"(n) : "rax", "rdi", "rsi", "rdx"
            );
        }
    } else {
        /* Opcional: Escribir mensaje de error si falla la apertura */
    }

    /* 4. exit(0) -> syscall 60 */
    __asm__ volatile (
        "mov $60, %%rax\n"
        "xor %%rdi, %%rdi\n"
        "syscall"
        ::: "rax", "rdi"
    );
}