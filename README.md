<p align="center">
  <img src="https://img.shields.io/badge/Rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white"/>
  <img src="https://img.shields.io/badge/x86__64-00599C?style=for-the-badge&logo=intel&logoColor=white"/>
  <img src="https://img.shields.io/badge/Limine-FDEFE8?style=for-the-badge&logo=limine&logoColor=black"/>
  <img src="https://img.shields.io/badge/UEFI-0A5CF5?style=for-the-badge&logo=uefi&logoColor=white"/>
  <img src="https://img.shields.io/badge/license-GPL-blue?style=for-the-badge"/>
</p>

<h1 align="center">MesaOS</h1>
<p align="center"><i>Un sistema operativo de 64 bits escrito desde cero en Rust.</i></p>
<p align="center">
  <b>Kernel híbrido</b> · <b>Multitarea apropiativa</b> · <b>Shim de drivers Linux</b> · <b>Audio HD</b> · <b>Red TCP/IP</b>
</p>

---

## 🌟 ¿Qué es MesaOS?

MesaOS es un sistema operativo x86_64 escrito **100% desde cero en Rust**. No es un Linux desde arriba ni un BSD modificado — es un kernel original con arquitectura híbrida que combina la seguridad de Rust con la compatibilidad de Linux vía un shim de más de 400 símbolos del kernel exportados.

### ¿Qué lo hace único?

- **Shim de drivers Linux reales**: exporta `kmalloc`, `printk`, `pci_*`, `dma_*` y 400+ símbolos para cargar controladores de Linux sin modificar
- **Audio WAV streaming** por HDA con resampleo en tiempo real y chunks de 512KB
- **Stack de red completo** (ARP/IPv4/ICMP/DHCP/DNS/Ethernet) sobre virtio-net y RTL8139
- **Initrd permanente**: los archivos inyectados en la ISO sobreviven al reinicio — persistencia sin disco
- **Shell interactivo** con 82 comandos, pipes, redirección, historial, autocompletado y editor de texto
- **Compatibilidad binaria parcial con Linux**: syscalls Linux, `/proc`, `/sys`, epoll, signalfd, eventfd, timerfd

---

## 🚀 Características

### 🧠 Kernel y Sistema

| Característica | Descripción |
|---------------|-------------|
| Kernel híbrido | Monolítico con espacio de usuario en Ring 3 |
| Multitarea apropiativa | Round-robin con 5 estados, sleep queue, zombies |
| Syscalls nativas | SYSCALL/SYSRET con validación SMAP/SMEP software |
| Linux compat | ~350 syscalls reconocidas, 60+ implementadas |
| Shim de drivers | 400+ símbolos del kernel Linux exportados |
| ACPI | RSDP, MADT, AML interpreter vía crate `aml` |
| Multiprocesamiento | Detección SMP (pendiente scheduler multicore) |
| Seguridad | ASLR, stack canary, rate-limiting, password hashing |

### 🎮 Drivers

| Driver | Descripción | Estado |
|--------|-------------|--------|
| **HDA Audio** | 48kHz/16-bit/estéreo, CORB/RIRB, DMA streaming, codec routing | ✅ |
| **Virtio-Net** | MMIO, colas Rx/Tx, para QEMU | ✅ |
| **RTL8139** | Ethernet 10/100, PIO, IRQ | ✅ |
| **PS/2 Keyboard** | Scan codes, layouts ES/US | ✅ |
| **RTC (CMOS)** | Reloj en tiempo real, timezone | ✅ |
| **Batería** | ACPI `_BST`/`_BIF` + EC fallback, cycle count, SOH | ✅ |
| **PC Speaker** | Tonos, beep, TTS experimental | ✅ |
| **Framebuffer** | Consola, Rose Pine, renderizado HTML | ✅ |
| **UEFI NVRAM** | Read/write/list variables runtime services | ✅ |
| **NVMe** | Código completo (desactivado por seguridad) | ⏸️ |
| **xHCI (USB 3.0)** | Init y port scanning (periféricos desactivados) | 🚧 |
| **ATA/IDE** | Código completo (desactivado por seguridad) | ⏸️ |

### 🌐 Red

| Protocolo | Estado |
|-----------|--------|
| Ethernet — Frame parsing/creation | ✅ |
| ARP — Caché, requests, replies | ✅ |
| IPv4 — Enrutamiento, fragmentación | ✅ |
| ICMP — Echo (ping) | ✅ |
| TCP — Parseo de headers | 🚧 |
| UDP | 🚧 |
| DHCP — Cliente | ✅ |
| DNS — Resolución | ✅ |
| RNDIS — USB tethering | 🚧 |

### 📂 Sistema de Archivos

| Componente | Estado |
|------------|--------|
| VFS abstracto con File/Directory/Symlink/Device | ✅ |
| RamFS — Árbol en memoria | ✅ |
| Initrd — Archivos embebidos en ISO (persistente) | ✅ |
| Particiones — MBR/GPT parseadas | ✅ |
| `write`/`read`/`mkdir`/`rmdir`/`rename`/`chown`/`link`/`symlink` | ✅ |

### 🎵 Audio Player

| Característica | Estado |
|----------------|--------|
| WAV PCM 8/16/24/32 bits, mono/estéreo | ✅ |
| Resampleo lineal por chunks (evita OOM) | ✅ |
| Streaming DMA con chunks de 512KB (~2.7s) | ✅ |
| MP3 — Metadatos (duración, bitrate, sample rate) | ✅ |
| Comandos: `play`, `audio-info`, `audio-list` | ✅ |

### 🖥️ Shell (82 comandos)

| Comando | Función |
|---------|---------|
| `play f.wav` | Reproduce audio WAV |
| `hda test` / `hda vol 80` | Test / volumen HDA |
| `battery-report` | Reporte completo de batería |
| `nano` | Editor de texto |
| `neofetch` | Información del sistema |
| `ping 10.0.2.2` | ICMP echo |
| `dhcp` | Configuración de red |
| `bios-analyze` | Escaneo de BIOS |
| `nvram list/read/write/del` | Gestión UEFI NVRAM |
| `su`, `passwd`, `useradd`, `userdel` | Multiusuario |
| `exec f.elf` | Ejecutar ELF en Ring 3 |
| `ps`, `kill`, `top` | Gestión de procesos |
| `|`, `>`, `>>` | Pipes y redirección |

---

## 🔋 Battery Report

Inspirado en `upower -i` de Linux. Dos fuentes de datos en cascada:

1. **ACPI AML** (preferido): evalúa `_BST`/`_BIF`/`_BTP`
2. **EC directo** (fallback): registros estándar 0xD0/0xD8

```
╔══════════════════════════════════════════════════════════════╗
║              MESA OS  -  B A T T E R Y   R E P O R T         ║
╚══════════════════════════════════════════════════════════════╝

  state                    discharging
  percentage               72%
  energy-full-design       45000 mWh
  energy-now               30600 mWh
  time-to-empty            2:27:33
  capacity (SOH)           94%
  cycle-count              124
  source                   AML (ACPI _BST/_BIF)
```

```bash
battery-report        # Reporte completo
battery-report -w     # Modo monitor (refresca cada 2s)
battery-report -b     # Brief (una línea)
battery-report -v     # Verbose (OEM info, warnings)
```

---

## 📦 Initrd Persistente

Los archivos que inyectes en la ISO **se quedan para siempre**. No se pierden al reiniciar porque van embebidos en el kernel dentro de la ISO.

```bash
# Inyectar archivos y reconstruir en un solo paso
./tools/inject_build.sh mis_datos/

# O por separado
./tools/inject_to_iso.sh archivo.wav config.conf
./build.sh build

# Restaurar ISO limpia (sin archivos)
./build.sh restore
```

> Los archivos aparecen en `/inyect/` dentro del OS.

---

## 🧱 Arquitectura

```
┌──────────────────────────────────────────────────────────┐
│                    RING 3 (USUARIO)                       │
│  Procesos ELF · Shell · Pipes · Syscalls Linux/MesaOS    │
├──────────────────────────────────────────────────────────┤
│                    RING 0 (KERNEL)                        │
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐ │
│  │  Scheduler   │  │   Memoria    │  │  Linux Shim    │ │
│  │  Round-Robin │  │  PMM/VMM     │  │  400+ símbolos │ │
│  │  Sleep Queue │  │  Heap/HHDM   │  │  workqueues    │ │
│  │  Zombies     │  │  Paginación  │  │  timers        │ │
│  └──────────────┘  └──────────────┘  └────────────────┘ │
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐ │
│  │   Drivers    │  │   Red        │  │  Filesystem    │ │
│  │  HDA Audio   │  │  ARP/IPv4    │  │  VFS · RamFS   │ │
│  │  RTL8139     │  │  ICMP/DHCP   │  │  Initrd · MBR  │ │
│  │  Virtio-Net  │  │  DNS/TCP     │  │  GPT           │ │
│  │  xHCI/NVMe   │  │  Ethernet    │  │                │ │
│  │  Keyboard    │  │              │  │                │ │
│  └──────────────┘  └──────────────┘  └────────────────┘ │
│                                                          │
│  ┌──────────────────────────────────────────────────────┐│
│  │  x86_64: GDT · IDT · APIC/PIC · SYSCALL/SYSRET · TSS││
│  └──────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────┘
```

---

## ⚙️ Compilación

### Requisitos

```bash
rustup toolchain install nightly
rustup default nightly
sudo apt install xorriso qemu-system-x86_64 git make gcc
```

### Build

```bash
chmod +x build.sh

./build.sh build           # Compilar kernel + ISO
```

### Ejecutar en QEMU

```bash
./build.sh run             # Virtio-net (recomendado)
./build.sh run-nvme        # Con NVMe simulado
./build.sh run-usb         # Con xHCI USB
./build.sh run-wifi        # Con display gráfico
```

### Variables de entorno

```bash
NO_INJECT=1 ./build.sh build    # Compilar sin inyección
ARCH=aarch64 ./build.sh build   # ARM64 (experimental)
```

---

## 🗺️ Roadmap

- [x] **v0.1** — Kernel base, memoria, scheduler, shell básico
- [x] **v0.2** — HDA Audio, WAV streaming, red (ARP/IP/DHCP)
- [x] **v0.3** — Initrd persistente, battery report, UEFI NVRAM
- [x] **v0.4** — Linux shim, 350 syscalls, Ring 3, ELF loader, editor nano
- [ ] **v0.5** — xHCI funcional (control transfers reales), USB storage
- [ ] **v0.6** — TCP state machine (conexiones reales)
- [ ] **v0.7** — SMP (scheduler multicore)
- [ ] **v0.8** — Servidor web HTTP embebido
- [ ] **v0.9** — Decodificador MP3 (Huffman + MDCT)
- [ ] **v1.0** — WiFi RTL8822CE

---

## 🧪 Hardware probado

| Plataforma | Funciona |
|------------|----------|
| QEMU (KVM/TCG) | ✅ Completo |
| HP 15s-eq2xxx | ✅ Audio, batería, red, NVMe |
| UEFI (Limine) | ✅ |
| BIOS (GRUB) | ✅ |

---

## ⚠️ Aviso

El driver **NVMe sobrescribe el Sector 0** (tabla de particiones) durante la inicialización.  
**No ejecutes en hardware con datos importantes si llega a descomentar el driver NVMe.** Usa QEMU o un disco de pruebas.

---

## 👤 Creador

**Crackanimad0r / Crackanimador** ⛩️

---

<p align="center">
  <i>Hecho con ☕, 🦀 y mucha paciencia.</i>
</p>
