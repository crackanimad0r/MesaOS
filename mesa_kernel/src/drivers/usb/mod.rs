pub mod descriptors;
pub(crate) mod linux_glue;
pub mod mesa_fs;
pub mod xhci_native;

pub fn init() {
    crate::mesa_println!("[USB] Buscando controlador xHCI...");

    if let Some(dev) = crate::pci::find_usb_controller(0x30) {
        crate::mesa_println!(
            "[USB] xHCI encontrado: {:04x}:{:04x} (B{}.D{}.F{})",
            dev.vendor_id,
            dev.device_id,
            dev.bus,
            dev.device,
            dev.function
        );

        // Inicializar de forma nativa en lugar del shim
        xhci_native::init(&dev);
    } else {
        crate::mesa_println!("[USB] No se encontró controlador xHCI en PCI");
    };
}
