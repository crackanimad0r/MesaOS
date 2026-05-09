// mesa_kernel/src/pci/mod.rs

extern crate alloc;

use alloc::vec::Vec;
use spin::Mutex;
#[cfg(target_arch = "x86_64")]
use x86_64::instructions::port::Port;

#[cfg(target_arch = "x86_64")]
const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
#[cfg(target_arch = "x86_64")]
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
    pub fn class_name(&self) -> &'static str {
        match (self.class_code, self.subclass) {
            (0x00, 0x00) => "Non-VGA unclassified",
            (0x01, 0x01) => "IDE controller",
            (0x01, 0x06) => "SATA controller",
            (0x01, 0x08) => "NVMe controller",
            (0x02, 0x00) => "Ethernet controller",
            (0x02, 0x80) => "Network controller (Other/Wireless)",
            (0x03, 0x00) => "VGA controller",
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
                0x0C => "Serial bus",
                _ => "Unknown",
            },
        }
    }

    pub fn vendor_name(&self) -> &'static str {
        match self.vendor_id {
            0x1234 => "QEMU",
            0x8086 => "Intel",
            0x1022 => "AMD",
            0x10DE => "NVIDIA",
            0x1002 => "AMD/ATI",
            0x10EC => "Realtek",
            0x1AF4 => "Red Hat (virtio)",
            0x15AD => "VMware",
            0x80EE => "VirtualBox",
            _ => "Unknown",
        }
    }
}

static PCI_DEVICES: Mutex<Vec<PciDevice>> = Mutex::new(Vec::new());

pub fn pci_config_read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
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
    #[cfg(target_arch = "aarch64")]
    {
        0xFFFFFFFF
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

pub fn pci_config_write(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    #[cfg(target_arch = "x86_64")]
    {
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
}


pub fn pci_read_bar(bus: u8, device: u8, function: u8, bar_index: u8) -> Option<(u64, u64)> {
    if bar_index > 5 {
        return None;
    }
    
    let offset = 0x10 + (bar_index * 4);
    let bar_low = pci_config_read(bus, device, function, offset);
    
    // Check BAR type first
    let is_io = (bar_low & 0x01) == 1;
    
    // Decode address (mask off lower bits)
    let mut address = if is_io {
        (bar_low & 0xFFFFFFFC) as u64
    } else {
        (bar_low & 0xFFFFFFF0) as u64
    };

    let is_64bit = if is_io { false } else { ((bar_low >> 1) & 0x03) == 0x02 };
    
    if is_64bit && bar_index < 5 {
        let bar_high = pci_config_read(bus, device, function, offset + 4);
        address |= (bar_high as u64) << 32;
    }
    
    // Special case: if address is 0, BAR might not be configured yet
    // This shouldn't happen on properly configured hardware, but we handle it
    if address == 0 {
        return None;
    }
    
    // Calculate size
    let mask = if is_io { 0xFFFFFFFC } else { 0xFFFFFFF0 };
    pci_config_write(bus, device, function, offset, 0xFFFFFFFF);
    let size_low = pci_config_read(bus, device, function, offset);
    pci_config_write(bus, device, function, offset, bar_low); // Restore
    
    let mut size = (!(size_low & mask)).wrapping_add(1) as u64;

    
    if is_64bit && bar_index < 5 {
        pci_config_write(bus, device, function, offset + 4, 0xFFFFFFFF);
        let size_high = pci_config_read(bus, device, function, offset + 4);
        pci_config_write(bus, device, function, offset + 4, (address >> 32) as u32); // Restore
        
        if size_high != 0 {
            size = (!(((size_high as u64) << 32) | (size_low as u64 & 0xFFFFFFF0))).wrapping_add(1);
        }
    }
    
    Some((address, size))
}

pub fn pci_enable_bus_mastering(bus: u8, device: u8, function: u8) {
    let command = pci_config_read(bus, device, function, 0x04);
    pci_config_write(bus, device, function, 0x04, command | 0x04);
}

pub fn pci_enable_io_space(bus: u8, device: u8, function: u8) {
    let command = pci_config_read(bus, device, function, 0x04);
    pci_config_write(bus, device, function, 0x04, command | 0x01 | 0x04); // IO + Bus Master
}

pub fn pci_enable_memory_space(bus: u8, device: u8, function: u8) {
    let command = pci_config_read(bus, device, function, 0x04);
    pci_config_write(bus, device, function, 0x04, command | 0x02 | 0x04); // Memory + Bus Master
}

/// v11.60: Configura el Bridge padre para que deje pasar tráfico al rango de memoria del dispositivo.
/// También deshabilita ASPM en el bridge para prevenir cortes de enlace.
pub fn pci_setup_bridge_window(bus: u8, base: u32, size: u32) {
    if bus == 0 { return; }

    for dev in devices() {
        if dev.class_code == 0x06 && dev.subclass == 0x04 { // PCI-to-PCI Bridge
            let bus_reg = pci_config_read(dev.bus, dev.device, dev.function, 0x18);
            let secondary_bus = ((bus_reg >> 8) & 0xFF) as u8;
            
            if secondary_bus == bus {
                crate::mesa_println!("[PCI] Bridge found: {:02x}:{:02x}.{} -> controls bus {:02x}", dev.bus, dev.device, dev.function, bus);
                
                // Forzar alineamiento a 1MB (mínimo bridge window)
                let effective_size = if size < 0x100000 { 0x100000 } else { size };
                
                // Memory Base/Limit (Offset 0x20)
                let base_reg = (base >> 16) & 0xFFF0;
                let limit_reg = ((base + effective_size - 1) >> 16) & 0xFFF0;
                let mem_reg = (limit_reg << 16) | base_reg;
                
                pci_config_write(dev.bus, dev.device, dev.function, 0x20, mem_reg);
                
                // Activar comando del Bridge y DESHABILITAR ASPM
                let cmd = pci_config_read(dev.bus, dev.device, dev.function, 0x04);
                pci_config_write(dev.bus, dev.device, dev.function, 0x04, cmd | 0x02 | 0x04);
                
                pci_disable_aspm(dev.bus, dev.device, dev.function);
                
                crate::mesa_println!("[PCI]   Bridge window opened: {:#010x} - {:#010x}", base, (base + effective_size - 1));
                
                // RECURSION: Subir al siguiente nivel
                pci_setup_bridge_window(dev.bus, base, size);
            }
        }
    }
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
    if pci_get_vendor_id(bus, device, 0) == 0xFFFF {
        return;
    }

    scan_function(bus, device, 0);

    let header_type = pci_get_header_type(bus, device, 0);
    if header_type & 0x80 != 0 {
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

pub fn init() {
    #[cfg(target_arch = "x86_64")]
    {
        PCI_DEVICES.lock().clear();
        
        // Escaneo exhaustivo: todos los buses PCI (0-255)
        crate::serial_println!("[PCI] Scanning all buses (0-255)...");
        
        for bus in 0..=255 {
            let mut bus_has_devices = false;
            for device in 0..32 {
                if pci_get_vendor_id(bus, device, 0) != 0xFFFF {
                    bus_has_devices = true;
                    break;
                }
            }
            if bus_has_devices {
                scan_bus(bus);
            }
        }
        
        crate::serial_println!("[PCI] Scan complete: {} devices found", PCI_DEVICES.lock().len());
    }
}

pub fn devices() -> Vec<PciDevice> {
    PCI_DEVICES.lock().clone()
}

pub fn find_device(vendor_id: u16, device_id: u16) -> Option<PciDevice> {
    PCI_DEVICES.lock().iter()
        .find(|d| d.vendor_id == vendor_id && d.device_id == device_id)
        .cloned()
}

pub fn find_usb_controller(prog_if: u8) -> Option<PciDevice> {
    PCI_DEVICES.lock().iter()
        .find(|d| d.class_code == 0x0C && d.subclass == 0x03 && d.prog_if == prog_if)
        .cloned()
}

pub fn find_wifi_device() -> Option<PciDevice> {
    // Preferir RTL8822CE (HP Laptop 15s-eq2xxx y similares)
    if let Some(dev) = PCI_DEVICES.lock().iter()
        .find(|d| d.vendor_id == 0x10EC && d.device_id == 0xC822)
        .cloned() {
        return Some(dev);
    }
    // Cualquier controlador inalámbrico (class 0x02, subclass 0x80)
    if let Some(dev) = PCI_DEVICES.lock().iter()
        .find(|d| d.class_code == 0x02 && d.subclass == 0x80)
        .cloned() {
        return Some(dev);
    }
    // Fallback: cualquier controlador de red que no sea Ethernet (0x00)
    PCI_DEVICES.lock().iter()
        .find(|d| d.class_code == 0x02 && d.subclass != 0x00)
        .cloned()
}

pub fn pci_find_parent_bridge(bus: u8) -> Option<PciDevice> {
    if bus == 0 { return None; }
    PCI_DEVICES.lock().iter()
        .find(|d| d.class_code == 0x06 && d.subclass == 0x04 && 
                  ((pci_config_read(d.bus, d.device, d.function, 0x18) >> 8) & 0xFF) == bus as u32)
        .cloned()
}

pub fn device_count() -> usize {
    PCI_DEVICES.lock().len()
}

pub fn pci_find_capability(bus: u8, device: u8, function: u8, cap_id: u8) -> Option<u8> {
    let status = pci_config_read(bus, device, function, 0x06);
    // Check Bit 4 of Status Register (Capabilities List)
    if (status & 0x0010_0000) == 0 {
        return None;
    }

    let mut cap_offset = (pci_config_read(bus, device, function, 0x34) & 0xFF) as u8;
    
    // Limits traversal to avoid infinite loops
    let mut sanity_check = 0;
    while cap_offset != 0 && sanity_check < 48 {
        let cap_header = pci_config_read(bus, device, function, cap_offset);
        let id = (cap_header & 0xFF) as u8;
        let next = ((cap_header >> 8) & 0xFF) as u8;

        if id == cap_id {
            return Some(cap_offset);
        }

        cap_offset = next;
        sanity_check += 1;
    }

    None
}

pub fn pci_set_power_state(bus: u8, device: u8, function: u8, state: u8) -> Result<(), &'static str> {
    // Capability ID 0x01 is Power Management
    if let Some(pm_cap_offset) = pci_find_capability(bus, device, function, 0x01) {
        // PMCSR is at offset + 4
        let pmcsr_offset = pm_cap_offset + 4;
        let pmcsr = pci_config_read(bus, device, function, pmcsr_offset);
        
        // State is bits 0-1.
        // D0 = 0, D1 = 1, D2 = 2, D3 = 3
        let new_pmcsr = (pmcsr & !0x03) | (state as u32 & 0x03);
        
        if (pmcsr & 0x03) != (state as u32 & 0x03) {
            pci_config_write(bus, device, function, pmcsr_offset, new_pmcsr);
            // Verify
            let verify = pci_config_read(bus, device, function, pmcsr_offset);
            if (verify & 0x03) == (state as u32 & 0x03) {
                return Ok(());
            } else {
                return Err("Failed to change Power State");
            }
        }
        return Ok(());
    }
    
    Err("Power Management Capability not found")
}

pub fn pci_disable_aspm(bus: u8, device: u8, function: u8) {
    // Capability ID 0x10 is PCI Express
    if let Some(pcie_cap_offset) = pci_find_capability(bus, device, function, 0x10) {
        // Link Control Register is at offset + 0x10 from Cap Start
        let link_ctrl_offset = pcie_cap_offset + 0x10;
        let link_ctrl = pci_config_read(bus, device, function, link_ctrl_offset);
        
        // ASPM Control is bits 0:1
        // Common Clock is bit 2
        // CLKREQ Enable is bit 8
        let mut new_ctrl = link_ctrl;
        new_ctrl &= !0x03;   // Disable ASPM L0s/L1
        new_ctrl &= !0x04;   // Disable Common Clock (sometimes helps stability)
        new_ctrl &= !0x100;  // Disable CLKREQ (Critical for some Realtek)
        
        if (link_ctrl & 0xFFFF) != (new_ctrl & 0xFFFF) {
            crate::mesa_println!("[PCI] Hard-disabling ASPM/CLKREQ for {:02x}:{:02x}.{}", bus, device, function);
            pci_config_write(bus, device, function, link_ctrl_offset, new_ctrl);
        }
    }
}

pub fn pci_wait_for_link(bus: u8, device: u8, function: u8, retries: usize) -> bool {
    for i in 0..retries {
        let id = pci_config_read(bus, device, function, 0x00);
        if id != 0xFFFFFFFF && id != 0 {
            if i > 0 { crate::mesa_println!("[PCI] Link up after {} retries", i); }
            return true;
        }
        // Aumentamos el delay significativamente para hardware real
        for _ in 0..100_000u32 { core::hint::spin_loop(); }
    }
    false
}

