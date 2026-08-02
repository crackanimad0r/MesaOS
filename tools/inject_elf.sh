#!/bin/bash
# Compila hello.elf, la herramienta mesafs_inject, e inyecta el ELF en disk.img.

set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "[1/3] Compilando hello.elf..."
make -C userland/hello_elf

echo "[2/3] Compilando mesafs_inject..."
cargo build --release -p mesafs_inject --target x86_64-unknown-linux-gnu

echo "[3/3] Inyectando en disk.img..."
INJECT="$ROOT/target/x86_64-unknown-linux-gnu/release/mesafs_inject"
DISK="${1:-$ROOT/disk.img}"
ELF="$ROOT/userland/hello_elf/hello.elf"
DEST="${2:-/bin/hello.elf}"

if [ ! -f "$DISK" ]; then
    echo "Creando disco nuevo..."
    ./create_disk.sh
    echo "Formateando disco (MesaFS)..."
    "$INJECT" format "$DISK" 50
fi

if [ ! -f "$ELF" ]; then
    echo "Error: $ELF no existe."
    exit 1
fi

set +e
OUTPUT=$("$INJECT" inject "$DISK" "$ELF" "$DEST" 2>&1)
EXIT_CODE=$?
set -e

if [ $EXIT_CODE -ne 0 ]; then
    if [[ "$OUTPUT" == *"No es un disco MesaFS"* ]] || [[ "$OUTPUT" == *"magic incorrecto"* ]]; then
        echo "Disco no formateado o corrupto. Formateando..."
        "$INJECT" format "$DISK" 50
        echo "Reintentando inyección..."
        "$INJECT" inject "$DISK" "$ELF" "$DEST"
    else
        echo "Error al inyectar:"
        echo "$OUTPUT"
        exit 1
    fi
else
    echo "$OUTPUT"
fi

echo "Listo. En Mesa OS ejecuta: exec $DEST"
