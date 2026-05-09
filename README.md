# 🪐 MesaOS

> [!CAUTION]
> ### 🚨 ADVERTENCIA CRÍTICA: NO EJECUTAR EN HARDWARE REAL NO DESTINADO A PRUEBAS
> **MesaOS se encuentra en una etapa de desarrollo de bajo nivel.** El driver NVMe actual tiene un comportamiento de escritura que **SOBREESCRIBE LA TABLA DE PARTICIONES (Sector 0)** del disco físico. Ejecutar este sistema en tu PC personal resultará en la **PÉRDIDA TOTAL DE DATOS**. Úsese exclusivamente en máquinas virtuales (QEMU) o hardware de sacrificio.

---

## 🛠 Sobre el Proyecto
MesaOS es un sistema operativo de 64 bits escrito desde cero utilizando **Rust**. Aunque el núcleo del sistema ya se considera **completamente funcional**, el proyecto continúa en desarrollo activo para pulir drivers críticos y ampliar la compatibilidad con hardware moderno.

El repositorio no solo contiene el código fuente, sino también investigación y documentación técnica detallada sobre protocolos complejos como **xHCI (USB 3.0)** y **redes WiFi**, facilitando la implementación de estos controladores en el futuro.

### 🚀 Características principales
- **Lenguaje:** 100% Rust (Kernel Land) para máxima seguridad de memoria.
- **Arquitectura:** x86_64 con soporte para multiprocesamiento.
- **Bootloader:** Protocolo [Limine](https://github.com/limine-bootloader/limine) para un arranque robusto.
- **Documentación Técnica:** Incluye especificaciones analizadas para xHCI y tarjetas de red inalámbricas.

---

## 📊 Estado del Desarrollo

### ✅ Cosas Terminadas
- [x] **Kernel Core:** Gestión de interrupciones (IDT) y excepciones.
- [x] **Memoria:** Paginación dinámica y gestor de memoria física/virtual.
- [x] **Multitarea:** Planificador de procesos (Scheduler) básico.
- [x] **Driver NVMe:** Comunicación funcional con unidades de estado sólido (Lectura/Escritura).
- [x] **Soporte UEFI:** Arranque moderno mediante Limine.
- [x] **Documentación xHCI:** Análisis completo del protocolo para controladores USB modernos.
- [x] **User Mode:** Separación completa entre espacio de kernel y aplicaciones de usuario.

### 🚧 Cosas que estan en beta
- [x/] **Sistema de Archivos:** Implementación de una capa VFS estable (actualmente en pruebas).(Funcional solo RAMFS)
- [x/] **Driver WiFi:** Funcional solo en emulador (Y parcialmente)
- [x/] **USB Stack:** Finalizar el driver xHCI para soporte de teclado y ratón físico. (por ahora solo enumeracion y deshabilitado)
- [ ] **Protección de Particiones:** Implementar salvaguardas para evitar la escritura accidental en el Sector 0 (GPT/MBR).
- [ ] **User Mode:** Separación completa entre espacio de kernel y aplicaciones de usuario.

---

## 🏗 Compilación e Instalación

### Requisitos previos
- **Rust (Nightly toolchain)**
- **Mtools & Xorriso** (Para la creación de la imagen ISO)
- **QEMU** (Para emulación segura)
- **GCC & Make** (Para compilar las utilidades de Limine)

### Pasos para compilar
1. **Compilar:**
   ```bash
   ./build.sh build
