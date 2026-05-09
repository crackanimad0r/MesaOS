# Hello ELF para Mesa OS
# Compila un binario ELF64 estático que usa syscalls de Mesa OS (write=1, exit=60).

CC = gcc
CFLAGS = -nostdlib -static -fno-pie -no-pie -Wall -Wextra -O2
LDFLAGS = -nostdlib -static -no-pie -T linker.ld

hello.elf: hello.c linker.ld
	$(CC) $(CFLAGS) $(LDFLAGS) -o hello.elf hello.c

clean:
	rm -f hello.elf

.PHONY: clean
