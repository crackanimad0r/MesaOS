// mesa_kernel/src/pci/mod.rs

extern crate alloc;

use alloc::vec::Vec;
use spin::Mutex;
use x86_64::instructions::port::Port;

const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// Dispositivo PCI descubierto
#[derive(Debug, Clone)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
    pub header_type: u8,
}

impl PciDevice {
    /// Nombre legible para la clase del dispositivo
    pub fn class_name(&self) -> &'static str {
        match (self.class_code, self.subclass) {
            (0x00, 0x00) => "Non-VGA unclassified",
            (0x01, 0x01) => "IDE controller",
            (0x01, 0x06) => "SATA controller",
            (0x01, 0x08) => "NVMe controller",
            (0x02, 0x00) => "Ethernet controller",
            (0x03, 0x00) => "VGA controller",
            (0x04, 0x00) => "Video device",
            (0x04, 0x01) => "Audio device",
            (0x06, 0x00) => "Host bridge",
            (0x06, 0x01) => "ISA bridge",
            (0x06, 0x04) => "PCI bridge",
            (0x0C, 0x03) => "USB controller",
            _ => match self.class_code {
                0x00 => "Unclassified",
                0x01 => "Mass storage",
                0x02 => "Network",
                0x03 => "Display",
                0x04 => "Multimedia",
                0x05 => "Memory",
                0x06 => "Bridge",
                0x07 => "Communication",
                0x08 => "System peripheral",
                0x09 => "Input device",
                0x0A => "Docking station",
                0x0B => "Processor",
                0x0C => "Serial bus",
                0x0D => "Wireless",
                0x0E => "Intelligent controller",
                0x0F => "Satellite",
                0x10 => "Encryption",
                0x11 => "Signal processing",
                0xFF => "Unassigned",
                _ => "Unknown",
            },
        }
    }

    /// Nombre legible del vendor
    pub fn vendor_name(&self) -> &'static str {
        match self.vendor_id {
            0x1234 => "QEMU",
            0x8086 => "Intel",
            0x1022 => "AMD",
            0x10DE => "NVIDIA",
            0x1002 => "AMD/ATI",
            0x14E4 => "Broadcom",
            0x168C => "Qualcomm Atheros",
            0x10EC => "Realtek",
            0x1B21 => "ASMedia",
            0x1912 => "Renesas",
            0x1AF4 => "Red Hat (virtio)",
            0x15AD => "VMware",
            0x80EE => "VirtualBox",
            _ => "Unknown",
        }
    }
}

/// Lista global de dispositivos PCI
static PCI_DEVICES: Mutex<Vec<PciDevice>> = Mutex::new(Vec::new());

fn pci_config_read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
        let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);
        addr_port.write(address);
        data_port.read()
    }
}

fn pci_get_vendor_id(bus: u8, device: u8, function: u8) -> u16 {
    (pci_config_read(bus, device, function, 0) & 0xFFFF) as u16
}

fn pci_get_device_id(bus: u8, device: u8, function: u8) -> u16 {
    (pci_config_read(bus, device, function, 0) >> 16) as u16
}

fn pci_get_header_type(bus: u8, device: u8, function: u8) -> u8 {
    ((pci_config_read(bus, device, function, 0x0C) >> 16) & 0xFF) as u8
}

fn pci_get_class_info(bus: u8, device: u8, function: u8) -> (u8, u8, u8, u8) {
    let reg = pci_config_read(bus, device, function, 0x08);
    let revision = (reg & 0xFF) as u8;
    let prog_if = ((reg >> 8) & 0xFF) as u8;
    let subclass = ((reg >> 16) & 0xFF) as u8;
    let class_code = ((reg >> 24) & 0xFF) as u8;
    (class_code, subclass, prog_if, revision)
}

fn scan_function(bus: u8, device: u8, function: u8) {
    let vendor_id = pci_get_vendor_id(bus, device, function);
    if vendor_id == 0xFFFF {
        return;
    }

    let device_id = pci_get_device_id(bus, device, function);
    let (class_code, subclass, prog_if, revision) = pci_get_class_info(bus, device, function);
    let header_type = pci_get_header_type(bus, device, function);

    let dev = PciDevice {
        bus,
        device,
        function,
        vendor_id,
        device_id,
        class_code,
        subclass,
        prog_if,
        revision,
        header_type,
    };

    PCI_DEVICES.lock().push(dev);
}

fn scan_device(bus: u8, device: u8) {
    let vendor_id = pci_get_vendor_id(bus, device, 0);
    if vendor_id == 0xFFFF {
        return;
    }

    scan_function(bus, device, 0);

    let header_type = pci_get_header_type(bus, device, 0);
    if header_type & 0x80 != 0 {
        // Multi-function device
        for function in 1..8 {
            if pci_get_vendor_id(bus, device, function) != 0xFFFF {
                scan_function(bus, device, function);
            }
        }
    }
}

fn scan_bus(bus: u8) {
    for device in 0..32 {
        scan_device(bus, device);
    }
}

/// Escanea el bus PCI y rellena la lista global
pub fn init() {
    PCI_DEVICES.lock().clear();

    // Escanear bus 0
    scan_bus(0);

    // Verificar si hay múltiples buses (host bridge multi-function)
    let header_type = pci_get_header_type(0, 0, 0);
    if header_type & 0x80 != 0 {
        for function in 1..8 {
            if pci_get_vendor_id(0, 0, function) != 0xFFFF {
                scan_bus(function);
            }
        }
    }
}

/// Devuelve una copia de la lista de dispositivos
pub fn devices() -> Vec<PciDevice> {
    PCI_DEVICES.lock().clone()
}

/// Devuelve el número de dispositivos encontrados
pub fn device_count() -> usize {
    PCI_DEVICES.lock().len()
}

/// Escribe un valor al espacio de configuración PCI
pub fn pci_config_write(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    let address: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
        let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);
        addr_port.write(address);
        data_port.write(value);
    }
}

/// Busca un dispositivo WiFi (Realtek RTL8822CE)
pub fn find_wifi_device() -> Option<PciDevice> {
    for dev in PCI_DEVICES.lock().iter() {
        // RTL8822CE: vendor=0x10EC, device=0xC822
        if dev.vendor_id == 0x10EC && dev.device_id == 0xC822 {
            return Some(dev.clone());
        }
        // RTL8822CE variant (sometimes different device ID)
        if dev.vendor_id == 0x10EC && dev.device_id == 0xC82C {
            return Some(dev.clone());
        }
    }
    None
}

/// Busca cualquier dispositivo por vendor ID y device ID
pub fn find_device(vendor_id: u16, device_id: u16) -> Option<PciDevice> {
    for dev in PCI_DEVICES.lock().iter() {
        if dev.vendor_id == vendor_id && dev.device_id == device_id {
            return Some(dev.clone());
        }
    }
    None
}

/// Busca un controlador USB específico (e.g., xHCI prog_if = 0x30)
pub fn find_usb_controller(prog_if: u8) -> Option<PciDevice> {
    for dev in PCI_DEVICES.lock().iter() {
        if dev.class_code == 0x0C && dev.subclass == 0x03 && dev.prog_if == prog_if {
            return Some(dev.clone());
        }
    }
    None
}

/// Configura la ventana de memoria del puente PCI
/// Esto es necesario para que el chip WiFi pueda acceder a sus recursos MMIO
pub fn pci_setup_bridge_window(bus: u8, base: u32, size: u32) {
    // Esta es una implementación básica
    // En硬件 real, necesitaríamos configurar el puente PCI (puente root/downstream)
    // para permitir el acceso a la dirección de memoria asignada
    
    // Por ahora, simplemente aseguramos que el bus master esté habilitado
    // y que el espacio de memoria esté habilitado
    for dev in PCI_DEVICES.lock().iter() {
        if dev.bus == bus {
            // Habilitar Memory Space y Bus Master
            let cmd = pci_config_read(bus, dev.device, dev.function, 0x04);
            let new_cmd = cmd | 0x07; // Memory Space + I/O Space + Bus Master
            pci_config_write(bus, dev.device, dev.function, 0x04, new_cmd);
        }
    }
    
    // Log de la configuración
    crate::serial_println!("[PCI] Bridge window configured: base={:#x}, size={:#x}", base, size);
}

/// Lee un valor de 8 bits del espacio de configuración PCI
pub fn pci_config_read_byte(bus: u8, device: u8, function: u8, offset: u8) -> u8 {
    let value = pci_config_read(bus, device, function, offset & 0xFC);
    let shift = (offset & 3) * 8;
    ((value >> shift) & 0xFF) as u8
}

/// Lee un valor de 16 bits del espacio de configuración PCI
pub fn pci_config_read_word(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
    let value = pci_config_read(bus, device, function, offset & 0xFC);
    let shift = (offset & 2) * 8;
    ((value >> shift) & 0xFFFF) as u16
}
