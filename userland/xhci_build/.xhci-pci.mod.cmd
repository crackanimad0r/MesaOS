savedcmd_xhci-pci.mod := printf '%s\n'   xhci-pci.o xhci-pci-renesas.o | awk '!x[$$0]++ { print("./"$$0) }' > xhci-pci.mod
