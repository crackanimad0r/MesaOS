pub mod rtl8139;
pub mod rtw8822c_phy_tables;
pub mod rtl8822ce;
pub mod virtio_net;

use core::sync::atomic::AtomicUsize;
pub static RNDIS_ACTIVE_SLOT: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    // 1. Intentar virtio-net primero (modo QEMU/VM)
    match virtio_net::init() {
        Ok(_) => {
            crate::serial_println!("[NET] virtio-net inicializado (modo QEMU)");
            return;
        }
        Err(e) => crate::serial_println!("[NET] virtio-net no disponible: {}", e),
    }

    // 2. Intentar RTL8822CE WiFi (hardware real)
    match rtl8822ce::init() {
        Ok(_) => {
            crate::serial_println!("[NET] RTL8822CE WiFi inicializado");
            return;
        }
        Err(e) => crate::serial_println!("[NET] RTL8822CE no disponible: {}", e),
    }

    // 3. Fallback: RTL8139 (hardware físico legacy)
    match rtl8139::init() {
        Ok(_) => crate::serial_println!("[NET] RTL8139 inicializado (fallback)"),
        Err(e) => crate::serial_println!("[NET] RTL8139 no disponible: {}", e),
    }
}

/// Devuelve true si virtio-net está activo (QEMU)
pub fn is_virtio() -> bool {
    virtio_net::is_active()
}

/// Envía un paquete Ethernet por la NIC activa
pub fn send(data: &[u8]) -> Result<(), &'static str> {
    if virtio_net::is_active() {
        virtio_net::send_packet(data)
    } else if rtl8822ce::is_ready() {
        rtl8822ce::send_packet(data)
    } else {
        rtl8139::send_packet(data)
    }
}

/// Obtiene la MAC de la NIC activa
pub fn mac() -> Option<[u8; 6]> {
    if virtio_net::is_active() {
        virtio_net::get_mac()
    } else {
        rtl8822ce::get_mac()
            .or_else(|| rtl8139::get_mac())
    }
}
