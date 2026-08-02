# Configuración HP Laptop 15s-eq2xxx para MesaOS

## Especificaciones de Hardware

### Procesador
- **Modelo**: AMD Ryzen 3/5/7 (Picasso/Renoir)
- **Arquitectura**: Zen/Zen 2
- **Núcleos**: 2-4 núcleos físicos
- **Frecuencia**: 2.0-3.5 GHz
- **Cache L3**: 4-8 MB compartido

### Chipset
- **Tipo**: AMD integrado (Promontory)
- **APIC**: Compatible con xAPIC/x2APIC
- **PCIe**: Gen 3.0
- **USB**: 3.1 Gen 1

### Memoria
- **Tipo**: DDR4-2400 SODIMM
- **Canales**: Dual channel
- **Capacidad**: 8-32 GB
- **ECC**: No soportado

### Almacenamiento
- **Tipo**: NVMe PCIe SSD
- **Interfaz**: PCIe Gen 3 x4
- **Capacidad**: 256GB-1TB

### Pantalla
- **Tamaño**: 15.6"
- **Resolución**: HD (1366x768) / FHD (1920x1080)
- **Tecnología**: IPS/TN
- **GPU integrada**: AMD Radeon Vega

### WiFi
- **Chip**: Realtek RTL8822CE
- **Estándares**: 802.11a/b/g/n/ac
- **Bandas**: 2.4GHz + 5GHz
- **Bluetooth**: 5.0

## Configuraciones MesaOS Específicas

### CPU y Chipset
```rust
// Configuración específica para Ryzen
-cpu EPYC-v4,kvm=off
-smp 4,cores=4,threads=1,sockets=1
-machine q35,accel=kvm
```

### Memoria
```rust
-m 8G  // RAM típica de estas laptops
```

### SMBIOS
```rust
-smbios type=1,manufacturer="HP",product="HP Laptop 15s-eq2xxx",version="Type 1 Product ConfigId"
-smbios type=2,manufacturer="HP",product="HP Laptop System Board"
```

### Almacenamiento
```rust
-device nvme,serial=HP-NVME-2026  // NVMe compatible con HP
```

### Red/WiFi
- **Nota**: En QEMU, usar `-net none` para evitar conflictos
- **Hardware real**: Requiere modo UEFI para funcionamiento completo

## Problemas Conocidos y Soluciones

### 1. WiFi no detecta redes
- **Causa**: Modo BIOS en QEMU no soporta WiFi real
- **Solución**: Usar hardware real con modo UEFI

### 2. Rendimiento lento en QEMU
- **Causa**: Emulación TCG sin KVM
- **Solución**: Instalar KVM o usar hardware real

### 3. Pantalla negra en boot
- **Causa**: Configuración VGA incorrecta
- **Solución**: Usar `-vga virtio` en QEMU

## Comandos de Prueba

### En MesaOS:
```bash
wifi                # Ver estado del driver
wifi scan          # Escanear redes WiFi
wifi log           # Ver logs de inicialización
```

### En QEMU:
```bash
./build.sh run      # Emulación básica
./build.sh run-disk # Con disco NVMe
```

### En Hardware Real:
1. Grabar ISO en USB
2. Reiniciar y entrar a menú boot (F9/F10)
3. Seleccionar USB como dispositivo de boot
4. Elegir "Mesa OS" en menú Limine

## Verificación de Compatibilidad

Para verificar que MesaOS funciona correctamente en tu HP Laptop 15s-eq2xxx:

1. **CPU**: Debería detectar AMD Ryzen correctamente
2. **Memoria**: Debería detectar toda la RAM instalada
3. **WiFi**: Debería encontrar el adaptador RTL8822CE
4. **Pantalla**: Debería mostrar framebuffer correctamente
5. **Teclado**: Debería funcionar input PS/2

## Logs Esperados

```
[CPU] AMD Ryzen detected (4 cores, 8 threads)
[MEM] 8192 MB RAM detected
[WIFI] RTL8822CE detected at PCI 00:00:00
[FB] Framebuffer initialized: 1920x1080
[KBD] PS/2 Keyboard initialized
[APIC] Local APIC initialized
```

## Optimizaciones Futuras

1. **SMP**: Soporte multi-core para Ryzen
2. **AMD-V**: Virtualización anidada
3. **PCIe**: Optimizaciones para Gen 3
4. **Power Management**: Soporte AMD P-State
5. **WiFi 6**: Actualización a RTL8852BE si disponible