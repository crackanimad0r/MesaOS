# MesaOS Shim Layer Architecture
## Linux Driver Compatibility Layer — Aislado y Seguro

### 1. Problema Actual

Actualmente `linux_glue.rs` y el módulo `linux/` implementan una capa de compatibilidad **en-kernel** (Ring 0). Esto significa:

- Un `panic!()` en un driver de Linux mata **todo el kernel** de MesaOS.
- No hay separación de memoria: el driver accede a cualquier dirección.
- No hay control de recursos: un driver puede hacer busy-loop infinito.
- Difícil depuración y aislamiento.

### 2. Arquitectura Propuesta

```
┌─────────────────────────────────────────────────────────┐
│                    MesaOS Kernel (Ring 0)                │
│  ┌──────────┐  ┌───────────┐  ┌──────────────────────┐  │
│  │ Scheduler │  │  Syscalls │  │   Shim Manager       │  │
│  │   (IPC)   │  │ (ioctl)   │  │ (spawn/monitor/kill) │  │
│  └─────┬─────┘  └─────┬─────┘  └──────────┬───────────┘  │
│        │              │                    │              │
├────────┼──────────────┼────────────────────┼──────────────┤
│        │              │  syscall/isolation │              │
│        ▼              ▼                    ▼              │
│  ┌──────────────────────────────────────────────────┐     │
│  │           Shared Memory Region (MMIO)            │     │
│  │  - Command Ring (SCM: Shim Control Messages)     │     │
│  │  - Event Ring (interrupciones → eventos)          │     │
│  │  - Data Buffers (URB pool, sk_buff pool)          │     │
│  └──────────────────────┬───────────────────────────┘     │
│                         │                                 │
├─────────────────────────┼─────────────────────────────────┤
│                         │                                 │
│  Ring -1 (VMX non-root  │  ó  Ring 3 (Userland)          │
│  o proceso userland)     │                                 │
│  ┌──────────────────────▼───────────────────────────┐     │
│  │             Shim Process / VM                    │     │
│  │                                                  │     │
│  │  ┌─────────────────┐  ┌──────────────────────┐  │     │
│  │  │  C Wrapper       │  │  Linux Driver ABI    │  │     │
│  │  │  (traducción     │◄─┤  (struct usb_device, │  │     │
│  │  │   FFI)           │  │   struct urb, etc.)  │  │     │
│  │  └────────┬─────────┘  └──────────┬───────────┘  │     │
│  │           │                       │              │     │
│  │           ▼                       ▼              │     │
│  │  ┌──────────────────────────────────────────┐    │     │
│  │  │  Linux Driver (C code, compilado como     │    │     │
│  │  │  objeto estático o cargado vía initrd)    │    │     │
│  │  │  Ej: rtl8822ce_wifi_driver.o              │    │     │
│  │  └──────────────────────────────────────────┘    │     │
│  │                                                  │     │
│  └──────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────┘
```

### 3. Componentes Clave

#### 3.1 Shim Manager (Kernel Side, Rust)
- **Ubicación**: `mesa_kernel/src/shim/manager.rs`
- Responsabilidades:
  - Spawnear proceso shim (en Ring 3 o VMX non-root)
  - Asignar región de memoria compartida
  - Enviar comandos (SCM: Shim Control Message)
  - Recibir eventos (interrupciones, errores)
  - Watchdog: detectar hangs del driver
  - Policy: decidir si reiniciar o aislar un driver

#### 3.2 Shared Memory Region
- **Cabecera**: `struct shim_region` con:
  - Magic number + versión
  - Command queue (circular buffer, producer/consumer)
  - Event queue (circular buffer, producer/consumer)
  - Data pool (buffers pre-asignados para URB, sk_buff)
  - Status flags + heartbeat

#### 3.3 C Wrapper Layer
- **Headers**: `mesa_kernel/src/shim/c_api/usb_shim.h`, `wifi_shim.h`
- Propósito:
  - Exportar funciones que el driver de Linux espera (`usb_alloc_urb`, `usb_submit_urb`, etc.)
  - Traducir parámetros a mensajes SCM
  - Escribir en shared memory
  - Hacer syscall (VMEXIT o softirq) para notificar al kernel

#### 3.4 Rust FFI Bindings
- **Ubicación**: `mesa_kernel/src/shim/bridge.rs`
- `#[repr(C)]` structs que mapean exactamente los headers C
- Funciones `extern "C"` que el C wrapper llama
- Traits `FromShimMessage`, `ToShimMessage` para serialización

### 4. Flujo de Operación Típico

#### Ejemplo: USB Control Transfer

```
1. Kernel (Rust) necesita enviar un USB control request
   └→ shim_manager.send_command(CMD_USB_CONTROL, {slot_id, request, value, ...})
      └→ Escribe en Command Queue (shared memory)
         └→ Notifica al shim vía IPI o syscall

2. Shim Process recibe notificación
   └→ C wrapper lee el comando
      └→ Construye struct urb (Linux ABI)
         └→ Llama a usb_control_msg() del driver

3. Driver Linux ejecuta
   └→ Accede a MMIO del xHCI (mapeada en shim)
      └→ Completa el URB
         └→ Escribe resultado en Event Queue

4. Kernel recibe Event
   └→ shim_manager.poll_events()
      └→ Lee datos del URB completado
         └→ Retorna al caller original
```

### 5. Aislamiento de Fallos

```
┌─────────────────────────────────────────────────────┐
│ Estrategia de 3 capas:                              │
│                                                      │
│ 1. HW Isolation (opcional):                         │
│    - Intel VT-x / AMD-V : VM guest para el driver   │
│    - IOMMU (VT-d/AMD-Vi): restringe acceso DMA      │
│                                                      │
│ 2. Process Isolation (recomendado):                  │
│    - Ring 3 proceso userland                         │
│    - Page table separada (no acceso a kernel mem)    │
│    - Seccomp-style syscall whitelist                 │
│                                                      │
│ 3. Watchdog + Heartbeat:                             │
│    - Shim escribe heartbeat cada 100ms               │
│    - Kernel monitorea → si timeout, mata shim       │
│    - Si driver panic:                                │
│      a) Shim process muere (limpio)                  │
│      b) Kernel detecta missing heartbeat             │
│      c) Opcional: respawn automático                 │
│      d) Kernel NO se ve afectado                     │
└─────────────────────────────────────────────────────┘
```

### 6. Message Types (SCM Protocol)

| Comando | ID | Dirección | Descripción |
|---------|----|-----------|-------------|
| USB_CONTROL_TRANSFER | 0x01 | Kernel→Shim | Enviar control request |
| USB_BULK_TRANSFER | 0x02 | Kernel→Shim | Enviar bulk transfer |
| USB_ALLOC_URB | 0x03 | Kernel→Shim | Reservar URB |
| USB_FREE_URB | 0x04 | Kernel→Shim | Liberar URB |
| USB_SUBMIT_URB | 0x05 | Kernel→Shim | Encolar URB |
| USB_KILL_URB | 0x06 | Kernel→Shim | Cancelar URB |
| WIFI_SEND_SKB | 0x10 | Kernel→Shim | Enviar paquete WiFi |
| WIFI_RECV_SKB | 0x11 | Shim→Kernel | Recibir paquete WiFi |
| SHIM_HEARTBEAT | 0xFF | Shim→Kernel | Heartbeat periódico |
| SHIM_PANIC | 0xFE | Shim→Kernel | Driver panic (graceful) |

### 7. Estructura de Memoria Compartida

```c
#define SHIM_MAGIC   0x4D455341  // "MESA"
#define SHIM_VERSION 1

struct shim_command {
    uint32_t type;       // SCM message type
    uint32_t id;         // Unique request ID
    uint64_t arg[4];     // Arguments (typed per command)
    uint32_t data_len;   // Length of optional payload
    uint32_t padding;
    uint64_t data_ptr;   // Pointer into shared data pool
};

struct shim_event {
    uint32_t type;       // Event type (completion, error, etc.)
    uint32_t id;         // Matching request ID
    int32_t  status;     // Result status (0 = success, negative = errno)
    uint32_t actual_len; // Actual transferred bytes
    uint64_t data_ptr;   // Pointer into shared data pool
};

struct shim_region {
    uint32_t magic;                    // SHIM_MAGIC
    uint32_t version;                  // SHIM_VERSION
    uint32_t flags;                    // Status flags
    uint32_t heartbeat_counter;        // Incremented by shim
    
    // Command queue (kernel → shim)
    struct {
        volatile uint32_t head;
        volatile uint32_t tail;
        struct shim_command entries[64];
    } cmd_queue;
    
    // Event queue (shim → kernel)
    struct {
        volatile uint32_t head;
        volatile uint32_t tail;
        struct shim_event entries[64];
    } evt_queue;
    
    // Data pool for URB/skbuff payloads
    uint8_t data_pool[SHIM_DATA_POOL_SIZE];
    
    // Padding to page boundary
    uint8_t _reserved[0];
};
```

### 8. Routing de Interrupciones

Las interrupciones del hardware (xHCI MSI/MSI-X, WiFi PCIe) deben ir al **proceso shim**, no al kernel:

```
Hardware IRQ
    │
    ├─→ Si pasa por kernel: Shim redirige vía IPI
    │   - Kernel recibe IRQ
    │   - Kernel sabe qué shim maneja ese device
    │   - Kernel inyecta IRQ virtual al shim (vía IPI o EPT violation)
    │   - Shim procesa, escribe evento en shared memory
    │   - Kernel lee evento en próxima poll()
    │
    └─→ Ideal: IRQ directamente al shim (con IOMMU + interrupt remapping)
        - VT-d Interrupt Remapping: mapea MSI directo al VM
        - No necesidad de redirección
```

### 9. Implementación por Fases

| Fase | Descripción | Dependencias |
|------|-------------|-------------|
| 1 | Shared memory + SCM protocol (headers + Rust structs) | Ninguna |
| 2 | C Wrapper: `usb_alloc_urb`, `usb_submit_urb`, etc. | Fase 1 |
| 3 | Shim Manager: spawn, monitor, heartbeat | Fase 1 |
| 4 | Integración IRQ routing | Fase 2, 3 |
| 5 | WiFi driver (rtl8822ce) port | Fases 1-4 |
| 6 | IOMMU isolation (opcional) | Fase 3 |

### 10. Consideraciones de Seguridad de Memoria

- **Rust-side**: Todos los punteros a shared memory se mantienen como `NonNull<u8>` con lifetimes acotados. No `unsafe` expuesto público.
- **C-side**: `repr(C)` estricto, zero padding assumptions. Alineación verificada con `static_assert` en C y `#[repr(align(N))]` en Rust.
- **Validación**: Cada mensaje SCM se valida (type, bounds de punteros, rangos) antes de actuar.
- **Pool de datos**: No se permiten punteros arbitrarios; solo índices dentro del data_pool.
- **Timeouts**: Toda operación tiene timeout; si no hay respuesta, se aborta y reinicia el shim.