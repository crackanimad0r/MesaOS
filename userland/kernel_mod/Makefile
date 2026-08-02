# Makefile for MesaOS test kernel module (.ko)
# Compiles a relocatable ELF object that the MesaOS shim loader can load

CC = gcc
CFLAGS = -m64 -mcmodel=kernel -nostdinc -I include -fno-pic -fno-stack-protector -fno-builtin \
         -fno-omit-frame-pointer -O2 -Wall -Werror -c

test_mod.ko: test_mod.c
	$(CC) $(CFLAGS) test_mod.c -o test_mod.o
	ld -r -o test_mod.ko test_mod.o
	@echo "[KERNEL_MOD] Built test_mod.ko"
	@readelf -h test_mod.ko | head -3
	@readelf -S test_mod.ko | grep -E "init.text|exit.text|text"

clean:
	rm -f test_mod.ko

.PHONY: clean
