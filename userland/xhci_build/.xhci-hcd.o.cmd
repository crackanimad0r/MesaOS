savedcmd_xhci-hcd.o := ld -m elf_x86_64 -z noexecstack --no-warn-rwx-segments   -r -o xhci-hcd.o @xhci-hcd.mod 
