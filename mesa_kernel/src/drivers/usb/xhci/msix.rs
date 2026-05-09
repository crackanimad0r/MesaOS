// mesa_kernel/src/drivers/usb/xhci/msix.rs
//
// Configuración de MSI-X para controladores xHCI en hardware físico.
// NOTA: qemu-xhci usa interrupciones legacy (line IRQ), no MSI-X.
//       Esta función es no-fatal: si no hay MSI-X, simplemente continúa.

use crate::pci::{pci_config_read, pci_config_write, pci_find_capability};

pub fn configure_msix(bus: u8, device: u8, function: u8) {
    let cap_offset = match pci_find_capability(bus, device, function, 0x11) {
        Some(off) => off,
        None => {
            crate::serial_println!("[XHCI] MSI-X no disponible (normal en QEMU, usando IRQ legacy)");
            return;
        }
    };

    crate::serial_println!("[XHCI] MSI-X encontrado en offset {:#04x}", cap_offset);

    let msg_ctrl = pci_config_read(bus, device, function, cap_offset);
    let table_size = ((msg_ctrl >> 16) & 0x7FF) + 1;
    crate::serial_println!("[XHCI] MSI-X tabla: {} entradas", table_size);

    // Habilitar MSI-X (bit 31) y desactivar la máscara global (bit 30)
    let new_ctrl = (msg_ctrl | (1 << 31)) & !(1 << 30);
    pci_config_write(bus, device, function, cap_offset, new_ctrl);

    crate::serial_println!("[XHCI] MSI-X habilitado en hardware físico");
}
