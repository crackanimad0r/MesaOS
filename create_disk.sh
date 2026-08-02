#!/bin/bash

# Crear imagen de disco de 50 MB
dd if=/dev/zero of=disk.img bs=1M count=50

echo "Disco virtual creado: disk.img (50 MB)"
echo ""
echo "Para usar con QEMU:"
echo "  qemu-system-x86_64 -cdrom mesa-os.iso -hda disk.img -serial stdio"