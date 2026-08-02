savedcmd_xhci-pci.o := ld -m elf_x86_64 -z noexecstack --no-warn-rwx-segments   -r -o xhci-pci.o @xhci-pci.mod 
