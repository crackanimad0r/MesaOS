#!/bin/bash
# build.sh - Mesa OS Build System (HP Laptop 15s-eq2xxx Emulation)

set -e

# Configuración de Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PROJECT_ROOT=$(pwd)
ARCH=${2:-x86_64}
if [ "$ARCH" == "x86_64" ]; then
    KERNEL_BIN="$PROJECT_ROOT/target/x86_64-unknown-none/release/mesa_kernel"
    QEMU_CMD="qemu-system-x86_64"
else
    KERNEL_BIN="$PROJECT_ROOT/target/aarch64-unknown-none/release/mesa_kernel"
    QEMU_CMD="qemu-system-aarch64"
fi
LIMINE_DIR="$PROJECT_ROOT/limine"
ISO_DIR="$PROJECT_ROOT/iso"
DISK_IMG="$PROJECT_ROOT/disk.img"

# Rutas OVMF (opcionales para modo UEFI)
OVMF_CODE="/usr/share/OVMF/OVMF_CODE_4M.fd"
OVMF_VARS="/usr/share/OVMF/OVMF_VARS_4M.fd"

check_deps() {
    echo -e "${YELLOW}[CHECK]${NC} Verificando dependencias..."
    local missing=0
    
    for cmd in xorriso qemu-system-x86_64 cargo git; do
        if ! command -v $cmd &> /dev/null; then
            echo -e "${RED}[ERROR]${NC} $cmd no encontrado"
            missing=1
        fi
    done
    
    if [ $missing -eq 1 ]; then
        echo "Por favor instala las dependencias necesarias."
        exit 1
    fi
    echo -e "${GREEN}[OK]${NC} Dependencias OK"
}

get_limine() {
    if [ ! -d "$LIMINE_DIR" ]; then
        echo -e "${YELLOW}[DOWNLOAD]${NC} Descargando Limine..."
        git clone https://github.com/limine-bootloader/limine.git \
            --branch=v8.x-binary --depth=1
        make -C limine
    fi
}

build_userland() {
    if [ -d "userland/hello_elf" ]; then
        echo -e "${YELLOW}[BUILD]${NC} Compilando userland..."
        make -C userland/hello_elf
    fi
}

build_kernel() {
    build_userland
    echo -e "${YELLOW}[BUILD]${NC} Compilando kernel MesaOS ($ARCH)..."
    if [ "$ARCH" == "x86_64" ]; then
        cargo build -q --release --bin mesa_kernel --target x86_64-unknown-none
    else
        cargo build -q --release --bin mesa_kernel --target aarch64-unknown-none
    fi
    echo -e "${GREEN}[OK]${NC} Kernel compilado"
}

create_iso() {
    echo -e "${YELLOW}[ISO]${NC} Creando imagen booteable..."
    rm -rf "$ISO_DIR"
    mkdir -p "$ISO_DIR/boot/limine"
    mkdir -p "$ISO_DIR/boot/grub"
    mkdir -p "$ISO_DIR/EFI/BOOT"
    
    # Kernel (único para ambos modos)
    cp "$KERNEL_BIN" "$ISO_DIR/boot/mesa_kernel"
    
    # BIOS: Limine
    cp limine.conf "$ISO_DIR/boot/limine/"
    cp "$LIMINE_DIR/limine-bios.sys" "$ISO_DIR/boot/limine/"
    cp "$LIMINE_DIR/limine-bios-cd.bin" "$ISO_DIR/boot/limine/"
    cp "$LIMINE_DIR/limine-uefi-cd.bin" "$ISO_DIR/boot/limine/"
    
    # UEFI: GRUB2 (más compatible con OVMF)
    cp grub.cfg "$ISO_DIR/boot/grub/"
    
    # Crear imagen GRUB2 EFI si no existe
    if [ ! -f "BOOTX64.EFI" ]; then
        echo -e "${YELLOW}[GRUB]${NC} Creando GRUB2 UEFI bootloader..."
        grub-mkstandalone -O x86_64-efi -o BOOTX64.EFI \
            "boot/grub/grub.cfg=grub.cfg" 2>/dev/null || {
            echo -e "${RED}[ERROR]${NC} grub-mkstandalone no encontrado. Instala: sudo apt install grub-efi-amd64-bin"
            exit 1
        }
    fi
    cp BOOTX64.EFI "$ISO_DIR/EFI/BOOT/"
    
    xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
        -no-emul-boot -boot-load-size 4 -boot-info-table \
        --efi-boot boot/limine/limine-uefi-cd.bin \
        -efi-boot-part --efi-boot-image --protective-msdos-label \
        "$ISO_DIR" -o mesa-os.iso 2>/dev/null
    
    "$LIMINE_DIR/limine" bios-install mesa-os.iso 2>/dev/null
    echo -e "${GREEN}[OK]${NC} ISO lista: mesa-os.iso (BIOS: Limine, UEFI: GRUB2)"
}

create_disk() {
    local size_mb=${1:-100}
    if [ ! -f "$DISK_IMG" ]; then
        echo -e "${YELLOW}[DISK]${NC} Creando disco NVMe de ${size_mb}MB..."
        dd if=/dev/zero of="$DISK_IMG" bs=1M count=$size_mb status=none
        echo -e "${GREEN}[OK]${NC} Disco creado: disk.img"
    fi
}

run_hp() {
    local DISK_MODE=$1
    create_disk
    echo -e "${YELLOW}[RUN]${NC} Ejecutando MesaOS en QEMU..."
    
    qemu-system-x86_64 \
        -cdrom "$PROJECT_ROOT/mesa-os.iso" \
        -drive file="$DISK_IMG",format=raw,index=0,media=disk \
        -m 512 \
        -boot d \
        -netdev user,id=net0,hostfwd=tcp::8080-:80 \
        -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56 \
        -serial stdio \
        -display none
}

run_usb() {
    create_disk
    echo -e "${YELLOW}[RUN-USB]${NC} Ejecutando MesaOS con controlador xHCI y USB Storage..."
    
    qemu-system-x86_64 \
        -cdrom "$PROJECT_ROOT/mesa-os.iso" \
        -m 512 \
        -boot d \
        -device qemu-xhci,id=xhci \
        -device usb-storage,bus=xhci.0,drive=usbdisk \
        -drive file="$DISK_IMG",if=none,id=usbdisk,format=raw \
        -netdev user,id=net0,hostfwd=tcp::8080-:80 \
        -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56 \
        -serial stdio
}

run_wifi() {
    local DISK_MODE=$1
    create_disk
    echo -e "${YELLOW}[RUN-WIFI]${NC} Ejecutando con virtio-net y display gráfico..."
    
    qemu-system-x86_64 \
        -cdrom "$PROJECT_ROOT/mesa-os.iso" \
        -drive file="$DISK_IMG",format=raw,index=0,media=disk \
        -m 512 \
        -boot d \
        -netdev user,id=net0,hostfwd=tcp::8080-:80 \
        -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56 \
        -serial stdio
}

run_nvme() {
    create_disk
    echo -e "${YELLOW}[RUN-NVME]${NC} Ejecutando MesaOS con controlador NVMe..."
    
    qemu-system-x86_64 \
        -cdrom "$PROJECT_ROOT/mesa-os.iso" \
        -m 512 \
        -boot d \
        -drive file="$DISK_IMG",if=none,id=nvme_drv,format=raw \
        -device nvme,serial=deadbeef,drive=nvme_drv \
        -netdev user,id=net0,hostfwd=tcp::8080-:80 \
        -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56 \
        -serial stdio
}

run_tap() {
    local DISK_MODE=$1
    create_disk
    echo -e "${YELLOW}[RUN-TAP]${NC} Iniciando MesaOS con interfaz TAP (tap0)..."
    
    # Este modo requiere que hayas ejecutado ./setup_host_network.sh antes
    sudo qemu-system-x86_64 \
        -cdrom "$PROJECT_ROOT/mesa-os.iso" \
        -drive file="$DISK_IMG",format=raw,index=0,media=disk \
        -m 512 \
        -boot d \
        -netdev tap,id=net0,ifname=tap0,script=no,downscript=no \
        -device virtio-net-pci,netdev=net0,mac=52:54:00:12:34:56 \
        -serial stdio
}

flash_disk() {
    local TARGET=$2
    if [ -z "$TARGET" ]; then
        echo -e "${YELLOW}[FLASH]${NC} Dispositivos disponibles:"
        lsblk -o NAME,SIZE,TYPE,TRAN,MOUNTPOINT | grep -v loop
        echo ""
        read -rp "Ingresa el dispositivo de destino (ej: /dev/sdb): " TARGET
    fi

    if [ -z "$TARGET" ] || [ ! -b "$TARGET" ]; then
        echo -e "${RED}[ERROR]${NC} Dispositivo no válido: $TARGET"
        exit 1
    fi

    # Seguridad: Verificar si es el disco del sistema (raíz)
    if lsblk -no MOUNTPOINT "$TARGET" | grep -q -E "^/$"; then
        echo -e "${RED}[ERROR]${NC} $TARGET es tu disco del sistema operativo (/). ¡OPERACIÓN ABORTADA!"
        exit 1
    fi
    
    # También verificar particiones del disco
    if lsblk -no MOUNTPOINT "$TARGET" | grep -q -E "^/boot|^/home"; then
        echo -e "${RED}[ERROR]${NC} $TARGET contiene particiones críticas (/boot, /home). ¡OPERACIÓN ABORTADA!"
        exit 1
    fi

    # Seguridad: Verificar si hay particiones montadas
    if mount | grep -q "^$TARGET"; then
        echo -e "${RED}[ERROR]${NC} El dispositivo $TARGET o sus particiones están montados. Desmóntalos primero."
        exit 1
    fi

    echo -e "${RED}╔══════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  ⚠️  ADVERTENCIA: OPERACIÓN DESTRUCTIVA  ║${NC}"
    echo -e "${RED}╚══════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "Esto escribirá MesaOS en ${RED}${TARGET}${NC}."
    echo -e "Asegúrate de que no haya datos importantes en $TARGET."
    echo ""
    
    read -rp "¿Estás SEGURO? (escribe 'CONFIRMAR' para continuar): " confirm1
    if [ "$confirm1" != "CONFIRMAR" ]; then
        echo -e "${YELLOW}[CANCELADO]${NC} No se modificó nada."
        exit 0
    fi

    echo -e "${YELLOW}[FLASH]${NC} Escribiendo ISO mesa-os.iso en $TARGET..."
    sudo dd if=mesa-os.iso of="$TARGET" bs=4M status=progress oflag=sync

    echo -e "${GREEN}[OK]${NC} ¡ISO flasheada correctamente en ${TARGET}!"
    echo -e "${BLUE}[INFO]${NC} Ya puedes arrancar MesaOS desde este disco."
}

install_dual_boot() {
    local DISK="/dev/nvme0n1"
    echo -e "${BLUE}[DUAL-BOOT]${NC} Configurando Dual-Boot en $DISK..."
    
    if [ ! -b "$DISK" ]; then
        echo -e "${YELLOW}[INFO]${NC} No se encontró $DISK, buscando otros discos..."
        DISK=$(lsblk -dno NAME,TYPE | grep disk | head -n 1 | awk '{print "/dev/"$1}')
    fi

    echo -e "${YELLOW}[DUAL-BOOT]${NC} Usando disco: $DISK"
    
    # 1. Copiar Kernel a /boot (requiere que /boot sea accesible)
    echo -e "${YELLOW}[DUAL-BOOT]${NC} Instalando kernel en /boot/mesa_kernel..."
    sudo cp "$KERNEL_BIN" /boot/mesa_kernel
    
    # 2. Crear entrada de GRUB
    echo -e "${YELLOW}[DUAL-BOOT]${NC} Creando entrada en /etc/grub.d/40_mesa_os..."
    sudo bash -c "cat <<EOF > /etc/grub.d/40_mesa_os
#!/bin/sh
exec tail -n +3 \$0
menuentry \"Mesa OS (Dual-Boot)\" {
    insmod all_video
    insmod part_gpt
    insmod part_msdos
    multiboot2 /boot/mesa_kernel
}
EOF"
    sudo chmod +x /etc/grub.d/40_mesa_os
    
    # 3. Actualizar GRUB
    echo -e "${YELLOW}[DUAL-BOOT]${NC} Actualizando configuración de GRUB..."
    if command -v update-grub &> /dev/null; then
        sudo update-grub
    else
        sudo grub-mkconfig -o /boot/grub/grub.cfg
    fi
    
    echo -e "${GREEN}[OK]${NC} Configuración finalizada."
    echo ""
    echo -e "${BLUE}[INFO]${NC} Reinicia tu PC y busca 'Mesa OS (Dual-Boot)' en el menú."
}


show_help() {
    echo -e "${GREEN}Mesa OS Build Script (HP Edition)${NC}"
    echo "Uso: $0 [comando]"
    echo ""
    echo "Comandos:"
    echo "  build       - Compilar todo y crear ISO"
    echo "  run         - Ejecutar emulación rápida (RAM) con virtio-net"
    echo "  run-disk    - Ejecutar emulación completa (NVMe) con virtio-net"
    echo "  run-usb     - Ejecutar emulación con USB xHCI y USB Mass Storage"
    echo "  run-nvme    - Ejecutar emulación con controlador NVMe PCI"
    echo "  run-wifi    - Ejecutar con virtio-net (muestra info de red)"
    echo "  flash [dev] - ⚠️  Flashear ISO al disco (Operación destructiva)"
    echo "                (Ejemplo: ./build.sh flash /dev/sdb)"
    echo "  dual-boot   - 🛡️  Configurar Dual-Boot seguro (Kernel + GRUB)"
    echo "  clean       - Eliminar builds"
    echo "  help        - Esta ayuda"
    echo ""
    echo "Red QEMU (virtio-net NAT):"
    echo "  IP MesaOS  : 10.0.2.15 | Gateway: 10.0.2.2"
    echo "  MAC        : 52:54:00:12:34:56"
    echo "  HTTP forward: localhost:8080 → MesaOS:80"
    echo "  SSH forward : localhost:2222 → MesaOS:22"
}

case "${1:-build}" in
    build)
        check_deps
        get_limine
        build_kernel
        create_iso
        ;;
    run)
        run_hp "ram"
        ;;
    run-disk)
        run_hp "disk"
        ;;
    run-usb)
        run_usb
        ;;
    run-nvme)
        run_nvme
        ;;
    run-wifi)
        run_wifi "ram"
        ;;
    run-wifi-disk)
        run_wifi "disk"
        ;;
    run-tap)
        run_tap "ram"
        ;;
    flash)
        flash_disk "$@"
        ;;
    dual-boot)
        install_dual_boot
        ;;
    clean)
        echo -e "${YELLOW}[CLEAN]${NC} Limpiando..."
        cargo clean
        rm -rf iso/ mesa-os.iso
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        echo "Uso: $0 {build|run|run-disk|run-usb|run-nvme|run-wifi|clean|help}"
        exit 1
        ;;
esac