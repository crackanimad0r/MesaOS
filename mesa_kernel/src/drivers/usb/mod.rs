// mesa_kernel/src/drivers/usb/mod.rs
pub mod xhci;
pub mod descriptors;
pub mod rndis;
pub mod hub;
pub mod msc;
pub(crate) mod linux_glue;

use spin::Mutex;
use alloc::vec::Vec;
use crate::pci::PciDevice;

pub static XHCI_CONTROLLERS: Mutex<Vec<crate::drivers::usb::xhci::XhciDriver>> = Mutex::new(Vec::new());

pub fn init() {
    /*
    crate::mesa_println!("[USB] Escaneando controladores xHCI...");
    
    let mut found = Vec::new();

    // Escanear PCI: Class 0x0C (Serial), Subclass 0x03 (USB), Prog IF 0x30 (xHCI)
    for dev in crate::pci::devices() {
        if dev.class_code == 0x0C && dev.subclass == 0x03 && dev.prog_if == 0x30 {
            crate::mesa_println!("[USB] xHCI en {:02x}:{:02x}.{}", dev.bus, dev.device, dev.function);

            let Some((bar0_phys, size)) = crate::pci::pci_read_bar(dev.bus, dev.device, dev.function, 0) else {
                crate::serial_println!("[USB] xHCI BAR0 no legible en {:02x}:{:02x}.{}",
                    dev.bus, dev.device, dev.function);
                continue;
            };

            crate::serial_println!("[USB] xHCI BAR0={:#x} size={:#x}", bar0_phys, size);
            crate::pci::pci_enable_memory_space(dev.bus, dev.device, dev.function);

            if let Err(e) = crate::memory::vmm::map_mmio(bar0_phys, size) {
                crate::serial_println!("[USB] Error mapeando MMIO xHCI: {}", e);
                continue;
            }

            if let Some(mut ctrl) = xhci::XhciDriver::new(&dev, bar0_phys) {
                if ctrl.init().is_ok() {
                    ctrl.scan_ports();
                    found.push(ctrl);
                } else {
                    crate::serial_println!("[USB] xHCI init falló en {:02x}:{:02x}.{}",
                        dev.bus, dev.device, dev.function);
                }
            }
        }
    }

    let count = found.len();
    *XHCI_CONTROLLERS.lock() = found;

    if count > 0 {
        crate::mesa_println!("       USB 3.0: {} controlador(es) xHCI listo(s)", count);
    } else {
        crate::mesa_println!("[USB] Sin controladores xHCI");
    }
    */
}
