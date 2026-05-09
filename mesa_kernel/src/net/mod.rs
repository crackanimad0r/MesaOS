pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod icmp;
pub mod udp;
pub mod tcp;
pub mod dns;
pub mod dhcp;
pub mod config;

use alloc::vec::Vec;
use spin::Mutex;

static mut IP_ADDR: [u8; 4]    = [10, 0, 2, 15];
static mut NETMASK: [u8; 4]    = [255, 255, 255, 0];
static mut GATEWAY: [u8; 4]    = [10, 0, 2, 2];
static mut IP_ID_COUNTER: u16  = 0x1000;

pub fn init() {
    crate::serial_println!("[NET] Network stack initialized");
}

pub fn configure(ip: [u8; 4], netmask: [u8; 4], gateway: [u8; 4]) {
    unsafe {
        IP_ADDR  = ip;
        NETMASK  = netmask;
        GATEWAY  = gateway;
    }
    crate::serial_println!("[NET] Configured: {}.{}.{}.{}/{}.{}.{}.{} via {}.{}.{}.{}",
        ip[0], ip[1], ip[2], ip[3],
        netmask[0], netmask[1], netmask[2], netmask[3],
        gateway[0], gateway[1], gateway[2], gateway[3]);
}

pub fn get_ip()      -> [u8; 4] { unsafe { IP_ADDR } }
pub fn get_netmask() -> [u8; 4] { unsafe { NETMASK } }
pub fn get_gateway() -> [u8; 4] { unsafe { GATEWAY } }

pub fn next_ip_id() -> u16 {
    unsafe {
        IP_ID_COUNTER = IP_ID_COUNTER.wrapping_add(1);
        IP_ID_COUNTER
    }
}

pub fn is_virtio() -> bool {
    crate::drivers::net::virtio_net::is_active()
}

/// Devuelve la MAC de la NIC activa (virtio-net primero, RTL8139 como fallback)
pub fn get_mac() -> [u8; 6] {
    if let Some(mac) = crate::drivers::net::virtio_net::get_mac() {
        return mac;
    }
    if let Some(mac) = crate::drivers::net::rtl8139::get_mac() {
        return mac;
    }
    [0x52, 0x54, 0x00, 0x12, 0x34, 0x56] // Fallback estático
}

/// Envía un frame Ethernet usando la NIC activa
pub fn send_ethernet(dest_mac: [u8; 6], ethertype: u16, payload: &[u8]) -> Result<(), &'static str> {
    let src_mac = get_mac();
    let frame = ethernet::create_frame(src_mac, dest_mac, ethertype, payload);

    // 1. virtio-net (QEMU)
    if crate::drivers::net::virtio_net::is_active() {
        return crate::drivers::net::virtio_net::send_packet(&frame);
    }

    // 2. USB RNDIS
    if crate::drivers::usb::rndis::usb_net_tx(&frame).is_ok() {
        return Ok(());
    }

    // 3. RTL8139 (hardware físico legacy)
    crate::drivers::net::rtl8139::send_packet(&frame)
}

pub fn hex_dump(data: &[u8], len: usize) {
    let limit = len.min(data.len());
    for i in (0..limit).step_by(16) {
        let mut s = alloc::string::String::new();
        for j in 0..16 {
            if i + j < limit {
                use core::fmt::Write;
                let _ = write!(s, "{:02x} ", data[i + j]);
            } else {
                s.push_str("   ");
            }
        }
        crate::serial_println!("[HEX] {}", s);
    }
}

/// Procesa todos los paquetes entrantes de cualquier NIC activa
pub fn poll() {
    // 1. virtio-net (QEMU)
    while let Some(packet) = crate::drivers::net::virtio_net::poll_rx() {
        handle_packet(&packet);
    }

    // 2. RTL8139 (hardware físico)
    while let Some(packet) = crate::drivers::net::rtl8139::poll_rx() {
        handle_packet(&packet);
    }

    // 3. USB RNDIS
    if let Some(packet) = crate::drivers::usb::rndis::usb_net_poll() {
        handle_packet(&packet);
    }
}

fn handle_packet(packet: &[u8]) {
    if let Some(frame) = ethernet::parse_frame(packet) {
        match frame.ethertype {
            0x0806 => arp::handle_arp(&frame.payload),
            0x0800 => ipv4::handle_ipv4(&frame.payload),
            0x86dd => { /* IPv6 — ignorar */ }
            other  => {
                crate::serial_println!("[NET] Ethertype desconocido: {:#06x}", other);
            }
        }
    }
}
