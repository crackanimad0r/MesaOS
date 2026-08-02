# Hello ELF para Mesa OS
# Compila un binario ELF64 estático que usa syscalls de Mesa OS (write=1, exit=60).
# También compila modload.elf para probar la carga de módulos .ko.

CC = gcc
CFLAGS = -nostdlib -static -fno-pie -no-pie -Wall -Wextra -O2
LDFLAGS = -nostdlib -static -no-pie -T linker.ld

hello.elf: hello.c linker.ld
	$(CC) $(CFLAGS) $(LDFLAGS) -o hello.elf hello.c

modload.elf: modload.c linker.ld
	$(CC) $(CFLAGS) -fno-builtin $(LDFLAGS) -o modload.elf modload.c

all: hello.elf modload.elf

clean:
	rm -f hello.elf modload.elf

.PHONY: all clean
