CC = gcc
CFLAGS = -m64 -mcmodel=kernel -nostdinc -I include -fno-pic -fno-stack-protector -fno-builtin \
         -fno-omit-frame-pointer -O2 -Wall -Werror -Wno-error -c -DCONFIG_USB_XHCI_PCI

# Files with unavoidable pointer signedness or compatibility issues
CFLAGS_xhci = -Wno-error=incompatible-pointer-types -Wno-error=maybe-uninitialized -Wno-error=pointer-sign -Wno-error=unused-variable -Wno-error=unused-function -Wno-error=implicit-function-declaration

# Core files (from Linux Makefile: xhci-hcd-y)
CORE_OBJS = xhci.o xhci-mem.o xhci-ext-caps.o xhci-ring.o xhci-hub.o xhci-dbg.o \
            xhci-trace.o xhci-dbgcap.o xhci-dbgtty.o xhci-debugfs.o xhci-sideband.o

# PCI files
PCI_OBJS = xhci-pci.o

ALL_OBJS = $(CORE_OBJS) $(PCI_OBJS)

xhci.ko: $(ALL_OBJS)
	ld -r -o xhci.ko $(ALL_OBJS)
	@echo "[BUILD] xhci.ko ready"
	@ls -la xhci.ko

%.o: %.c
	$(CC) $(CFLAGS) $(CFLAGS_xhci) $< -o $@

clean:
	rm -f $(ALL_OBJS) xhci.ko

.PHONY: clean
