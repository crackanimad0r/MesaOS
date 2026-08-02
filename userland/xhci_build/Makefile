# Build xhci-hcd.ko como módulo externo usando el kernel del sistema.
# Usa los fuentes de Linux/linux-7.0.12/drivers/usb/host/ (ya copiados aquí)
# Ejecutar primero: make prepare  -> luego: make

KDIR := /lib/modules/$(shell uname -r)/build

# Inyectar compat header para APIs Linux 7.x que no existen en el kernel 6.17 del host
EXTRA_CFLAGS += -include $(src)/xhci_compat.h -I$(src)

# Fuentes del módulo xhci-hcd
xhci-hcd-y := xhci.o xhci-mem.o xhci-ext-caps.o xhci-ring.o xhci-hub.o xhci-dbg.o
xhci-hcd-y += xhci-trace.o xhci-dbgcap.o xhci-dbgtty.o xhci-debugfs.o xhci-sideband.o

obj-m := xhci-hcd.o

# PCI companion module
xhci-pci-y := xhci-pci.o xhci-pci-renesas.o
obj-m += xhci-pci.o

SRCDIR := /home/antonio/Documentos/MesaOS-mainn/MesaOS-main/Linux/linux-7.0.12/drivers/usb/host

prepare:
	cp $(SRCDIR)/xhci.c         ./
	cp $(SRCDIR)/xhci.h         ./
	cp $(SRCDIR)/xhci-mem.c     ./
	cp $(SRCDIR)/xhci-ext-caps.c ./
	cp $(SRCDIR)/xhci-ext-caps.h ./
	cp $(SRCDIR)/xhci-ring.c    ./
	cp $(SRCDIR)/xhci-hub.c     ./
	cp $(SRCDIR)/xhci-dbg.c     ./
	cp $(SRCDIR)/xhci-trace.c   ./
	cp $(SRCDIR)/xhci-trace.h   ./
	cp $(SRCDIR)/xhci-dbgcap.c  ./
	cp $(SRCDIR)/xhci-dbgcap.h  ./
	cp $(SRCDIR)/xhci-dbgtty.c  ./
	cp $(SRCDIR)/xhci-debugfs.c ./
	cp $(SRCDIR)/xhci-debugfs.h ./
	cp $(SRCDIR)/xhci-sideband.c ./
	cp $(SRCDIR)/xhci-pci.c    ./
	cp $(SRCDIR)/xhci-pci.h    ./
	cp $(SRCDIR)/xhci-pci-renesas.c ./
	cp $(SRCDIR)/xhci-caps.h   ./
	cp $(SRCDIR)/xhci-port.h   ./
	cp $(SRCDIR)/pci-quirks.h  ./
	sed -i 's/kzalloc_flex(\*urb_priv, td, num_tds, mem_flags)/kzalloc(struct_size((struct urb_priv *)NULL, td, num_tds), mem_flags)/' xhci.c
	sed -i 's/usb_endpoint_is_hs_isoc_double(udev, host_ep)/0/' xhci.c
	sed -i 's/kzalloc_obj(\*dev, flags)/kzalloc(sizeof(*dev), flags)/' xhci-mem.c
	sed -i 's/usb_endpoint_is_hs_isoc_double(udev, ep)/0/' xhci-mem.c
	sed -i 's/usb_endpoint_max_periodic_payload(udev, ep)/usb_endpoint_maxp(\&ep->desc) \* usb_endpoint_maxp_mult(\&ep->desc)/' xhci-mem.c
	for f in *.c; do sed -i 's/kzalloc_obj(\*\([a-z_]*\), \([a-z_]*\))/kzalloc(sizeof(*(\1)), \2)/g' "$$f"; sed -i 's/kzalloc_obj(\*\([a-z_]*\))/kzalloc(sizeof(*(\1)), GFP_KERNEL)/g' "$$f"; done
	sed -i 's/kvmalloc_objs(struct page \*, n_pages)/kvmalloc_array(n_pages, sizeof(struct page *), GFP_KERNEL)/' xhci-sideband.c
	sed -i 's/active = usb_offload_check(udev);/active = false;/' xhci-sideband.c
	sed -i 's/bool xhci_sideband_check(struct usb_hcd \*hcd)/bool xhci_sideband_check(struct usb_hcd *hcd);\n\nbool xhci_sideband_check(struct usb_hcd *hcd)/' xhci-sideband.c
	sed -i 's|#define TRACE_INCLUDE_PATH \.$$|#define TRACE_INCLUDE_PATH /home/antonio/Documentos/MesaOS-mainn/MesaOS-main/userland/xhci_build|' xhci-trace.h
	@echo "[PREPARE] Sources copied and patched."

# Kbuild fails at modpost because xhci is built-in.
# So we only build the objects, and manually link.
modules:
	$(MAKE) -C $(KDIR) M=$(CURDIR) xhci-hcd.o xhci-pci.o || true
	cp xhci-hcd.o xhci-hcd.ko
	cp xhci-pci.o xhci-pci.ko
	@echo "[BUILD] xhci-hcd.ko & xhci-pci.ko built successfully."

all: prepare modules

install:
	cp xhci-hcd.ko ../../inyect/bin/
	cp xhci-pci.ko ../../inyect/bin/
	@echo "[INSTALL] xhci-hcd.ko & xhci-pci.ko -> inyect/bin/"

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) clean
	rm -f *.c *.h *.ko

.PHONY: prepare modules all install clean
