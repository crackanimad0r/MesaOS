# 🔧 Guía Completa: Configuración BIOS HP Laptop 15s-eq2xxx para WiFi RTL8822CE

## 🎯 **Objetivo**
Habilitar el acceso completo al chip WiFi RTL8822CE que actualmente está bloqueado por configuraciones agresivas de energía en el BIOS/UEFI.

## 📋 **Información del Hardware**
- **Modelo**: HP Laptop 15s-eq2xxx
- **Chip WiFi**: Realtek RTL8822CE (10EC:C822)
- **Problema**: BAR0 = 0, chip en Low Power State, MMIO inaccesible

## 🚀 **Pasos para Configurar el BIOS**

### **Paso 1: Acceder al BIOS**
1. **Reiniciar la laptop**
2. **Presionar F10** repetidamente durante el boot (antes del logo de HP)
3. **Si no funciona**: Presionar F2, ESC, o buscar "BIOS Setup" en el menú de boot
4. **Contraseña**: Si pide contraseña, es la de administrador de Windows o la predeterminada de HP

### **Paso 2: Navegación en el BIOS**
- **Teclas de navegación**: Flechas ↑↓←→
- **Entrar en opciones**: Enter
- **Cambiar valores**: F5/F6 o +/- o espacio
- **Guardar y salir**: F10
- **Salir sin guardar**: ESC

### **Paso 3: Configuraciones Críticas para WiFi**

#### **🔋 Power Management (Más Importante)**
```
Main → Power Management Options
├── PCIe Power Management [Disabled]
├── USB Power Management [Disabled]
├── Wireless Radio Control [Enabled]
└── Network Standby [Disabled]
```

#### **💻 System Configuration**
```
Main → System Configuration
├── Boot Options
│   ├── UEFI Boot Order [Enabled]
│   └── Legacy Support [Disabled]
├── Device Configurations
│   ├── PCIe Speed [Gen 2] (NO Gen 3)
│   └── PCIe ASPM Support [Disabled]
└── Virtualization
    └── SVM [Disabled] (temporalmente)
```

#### **🔌 Advanced Options**
```
Advanced → Power-On Options
├── POST Hotkeys [Enabled]
└── Factory Recovery [Disabled]

Advanced → Built-in Device Options
├── Wireless Button State [On]
├── Wireless Radio [Enabled]
└── Action Keys Mode [Enabled]
```

#### **⚙️ Security Settings**
```
Security → System Security
├── Secure Boot [Disabled] (IMPORTANTE)
└── TPM Device [Hidden]

Security → Secure Boot Configuration
├── Secure Boot [Disabled]
└── Legacy Support [Enabled]
```

#### **🔧 Power Settings**
```
Power → Sleep States
├── S3 (Suspend to RAM) [Enabled]
└── S4 (Hibernate) [Disabled]

Power → PCI Express Settings
├── PCIe Link State Power Management [Disabled]
├── PCIe ASPM [Disabled]
└── PCIe Clock Power Management [Disabled]
```

### **Paso 4: Configuración de Red/Conectividad**
```
Main → Network Options
├── Wake on LAN [Disabled]
├── Wireless LAN [Enabled]
└── Bluetooth [Enabled]
```

### **Paso 5: Configuración de Energía Avanzada**
```
Advanced → Energy Saver Options
├── Hard Disk Timeout [Never]
├── System Standby [Never]
└── Screen Blank [Never]
```

## 🛠️ **Configuraciones Específicas para RTL8822CE**

### **Problema Identificado**
- El chip está en D3cold o LPS (Low Power State)
- BAR0 no se asigna porque el dispositivo está "invisible" para el sistema

### **Solución por BIOS**
1. **Deshabilitar PCIe ASPM**: Evita que el link entre en L1/L2 states
2. **Forzar PCIe Gen 2**: Gen 3 puede causar inestabilidad con este chip
3. **Deshabilitar Secure Boot**: Puede bloquear acceso directo al hardware
4. **Habilitar Legacy Support**: Para compatibilidad

## 📱 **Configuración desde Windows (Si tienes dual-boot)**

### **Método 1: Panel de Control**
```
Panel de Control → Sistema y Seguridad → Opciones de Energía
→ Configurar el plan → Cambiar configuración avanzada
→ PCI Express → Configuración de estado de enlace
  → Configuración: Apagado [Deshabilitado]
```

### **Método 2: Administrador de Dispositivos**
```
Administrador de Dispositivos → Adaptadores de red
→ Realtek RTL8822CE → Propiedades → Administración de energía
  → Permitir que el equipo apague este dispositivo [Desmarcado]
```

### **Método 3: Registro (Avanzado)**
```cmd
# Abrir regedit y navegar a:
HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Power\PowerSettings\501a4d13-42af-4429-9fd1-a8218c268e20\ee12f906-d277-404b-b290-7c279407c14b

# Cambiar valor a 0 (deshabilitado)
```

## 🔍 **Verificación de Cambios**

### **Después de Configurar el BIOS**
1. **Boot MesaOS** y ejecutar:
   ```bash
   wifi diag
   ```
2. **Verificar que aparezca**:
   - ✅ BAR0 asignado: [dirección no cero]
   - ✅ Power State: D0
   - ✅ Memory Space: Enabled

### **Si aún no funciona**
1. **Probar diferentes combinaciones** de configuraciones
2. **Actualizar BIOS** a la versión más reciente de HP
3. **Reset BIOS** a valores de fábrica y reconfigurar

## 📊 **Resultados Esperados**

### **Antes de la Configuración**
```
🚨 PROBLEMAS IDENTIFICADOS:
  ❌ BAR0 = 0: BIOS no asignó memoria MMIO
  ❌ Power State: D3 (debería ser D0)
  ❌ Memory Space: Disabled
```

### **Después de la Configuración**
```
✅ BAR0 asignado: 0xfe000000
✅ Power State: D0
✅ Memory Space: Enabled
✅ Bus Master: Enabled
```

## 🚨 **Precauciones**

### **⚠️ Riesgos**
- **Pérdida de datos**: Backup importante antes de cambiar BIOS
- **Inestabilidad**: Algunas configuraciones pueden causar crashes
- **Batería**: Deshabilitar power management reduce duración de batería

### **🔄 Reversión**
- **Reset BIOS**: F9 → Yes para restaurar defaults
- **Boot override**: Si no puedes entrar al BIOS, usar USB boot

## 📞 **Soporte HP**

Si nada funciona:
1. **Sitio web HP**: Buscar "HP Laptop 15s-eq2xxx BIOS update"
2. **Soporte HP**: Contactar con número de serie
3. **Foros**: Reddit r/HP o r/linuxhardware

## 🎯 **Próximos Pasos Después del BIOS**

Una vez configurado el BIOS correctamente, el driver WiFi debería funcionar. Ejecutar:

```bash
wifi init    # Inicializar driver
wifi status  # Ver estado
wifi scan    # Escanear redes
```

---

**Recuerda**: Los cambios en el BIOS requieren reinicio completo. Guarda esta guía para referencia futura.
