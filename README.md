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

### ✅ Terminados
- [x] **Kernel Core:** Gestión de interrupciones (IDT) y excepciones.
- [x] **Memoria:** Paginación dinámica y gestor de memoria física/virtual.
- [x] **Multitarea:** Planificador de procesos (Scheduler) básico.
- [x] **Driver NVMe:** Comunicación funcional (Lectura/Escritura).
- [x] **Soporte UEFI:** Arranque moderno mediante Limine.
- [x] **User Mode:** Separación completa entre espacio de kernel y aplicaciones de usuario.
- [x] **Documentación xHCI:** Análisis completo del protocolo USB 3.0.

### 🚧 En fase Beta / Parcial
- [/] **Sistema de Archivos:** Capa VFS funcional únicamente con **RAMFS** por ahora.
- [/] **Driver WiFi:** Funcionalidad parcial (limitado principalmente a entornos de emulación).
- [/] **USB Stack:** Driver xHCI permite la **enumeración** de dispositivos (soporte de periféricos deshabilitado temporalmente).

### ⏳ Pendientes
- [ ] **Protección de Particiones:** Implementar salvaguardas para evitar la escritura accidental en el Sector 0 (GPT/MBR).
- [ ] **Persistencia:** Extender el VFS para soporte de sistemas de archivos en disco.

---

## 🏗 Compilación e Instalación

### Requisitos previos
- **Rust (Nightly toolchain)**
- **Mtools & Xorriso** (Para la creación de la imagen ISO)
- **QEMU** (Para emulación segura)
- **GCC & Make** (Para las utilidades de Limine)

### Pasos para compilar
1. **Compilación completa:**
   ```bash
   ./build.sh build
2. **Ejecucion:**
   ```bash
   ./build.sh run-wifi //recomendado
Creador: Crackanimad0r/Crackanimador ⛩️⛩️

