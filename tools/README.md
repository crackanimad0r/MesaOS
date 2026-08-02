# Herramientas Mesa OS

## mesafs_inject

Inyecta archivos en un disco con formato MesaFS (el filesystem de Mesa OS).

### Compilar (para host Linux)

```bash
cargo build --release -p mesafs_inject --target x86_64-unknown-linux-gnu
```

El binario queda en `target/x86_64-unknown-linux-gnu/release/mesafs_inject`.

### Uso

```bash
mesafs_inject disk.img archivo_local ruta_en_disco
```

(No uses los símbolos `< >` en la terminal; son solo placeholders en la documentación.)

Ejemplos:

```bash
# Copiar hello.elf a /bin/hello.elf
mesafs_inject disk.img userland/hello_elf/hello.elf /bin/hello.elf

# Copiar a la raíz
mesafs_inject disk.img mi_programa.elf /mi_programa.elf
```

**Requisito:** El disco debe tener ya MesaFS creado (arrancar Mesa OS al menos una vez con ese disco).

## inject_elf.sh

Script que compila el ELF de ejemplo, la herramienta mesafs_inject, e inyecta en disk.img:

```bash
./tools/inject_elf.sh [disk.img] [ruta_destino]
```

Por defecto usa `disk.img` y destino `/bin/hello.elf`.
