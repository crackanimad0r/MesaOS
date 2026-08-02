# Hello ELF para Mesa OS

Binario de ejemplo que se ejecuta en Ring 3 en Mesa OS (usa syscalls write=1 y exit=60).

## Compilar

```bash
make
```

Genera `hello.elf`. Requiere `gcc` (x86_64).

## Copiar al disco de Mesa OS

Desde la raíz del proyecto:

```bash
# 1. Crear disco si no existe
./create_disk.sh

# 2. Arrancar Mesa OS una vez para que cree MesaFS y /bin
./build.sh run-disk
# (reinicia o cierra QEMU cuando veas el shell)

# 3. Inyectar hello.elf en el disco (desde la raíz del proyecto MesaOS)
./tools/inject_elf.sh
# o manualmente (sustituye por rutas reales, no uses los símbolos < >):
# cargo build --release -p mesafs_inject --target x86_64-unknown-linux-gnu
# ./target/x86_64-unknown-linux-gnu/release/mesafs_inject disk.img userland/hello_elf/hello.elf /bin/hello.elf
#
# Si estás en userland/hello_elf:
# ../../target/x86_64-unknown-linux-gnu/release/mesafs_inject ../../disk.img hello.elf /bin/hello.elf
```

## Ejecutar en Mesa OS

En el shell de Mesa OS:

```
exec /bin/hello.elf
```

Deberías ver: `Hello from Mesa OS!`
