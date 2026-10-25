pub mod arp;
pub mod config;
pub mod dhcp;
pub mod dns;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod tcp;
pub mod udp;
pub mod wpa;

static mut IP_ADDR: [u8; 4] = [10, 0, 2, 15];
static mut NETMASK: [u8; 4] = [255, 255, 255, 0];
static mut GATEWAY: [u8; 4] = [10, 0, 2, 2];
// DNS server learned from DHCP option 6. Defaults to Google DNS only for
// links that do not provide one (and for QEMU/virtio); most real APs block
// 8.8.8.8 and require their own resolver, which broke `curl` host resolution.
static mut DNS_SERVER: [u8; 4] = [8, 8, 8, 8];
static mut IP_ID_COUNTER: u16 = 0x1000;
// Address we were just offered but do not own yet. Answering ARP for it while
// the DHCP REQUEST/ACK is in flight lets a router that unicasts the ACK (and
// therefore ARPs the offered address) resolve us instead of dropping it.
static mut PENDING_IP: [u8; 4] = [0, 0, 0, 0];

pub fn init() {
    crate::serial_println!("[NET] Network stack initialized");
}

pub fn configure(ip: [u8; 4], netmask: [u8; 4], gateway: [u8; 4]) {
    // A zero mask (option 1 missing on some open/hotspot APs) makes is_local()
    // consider EVERY destination link-local, so external ping/curl ARP the
    // remote IP directly instead of using the gateway. Normalise to /24.
    let netmask = if netmask == [0, 0, 0, 0] {
        [255, 255, 255, 0]
    } else {
        netmask
    };
    unsafe {
        IP_ADDR = ip;
        NETMASK = netmask;
        GATEWAY = gateway;
    }
    crate::serial_println!(
        "[NET] Configured: {}.{}.{}.{}/{}.{}.{}.{} via {}.{}.{}.{}",
        ip[0],
        ip[1],
        ip[2],
        ip[3],
        netmask[0],
        netmask[1],
        netmask[2],
        netmask[3],
        gateway[0],
        gateway[1],
        gateway[2],
        gateway[3]
    );
}

pub fn get_ip() -> [u8; 4] {
    unsafe { IP_ADDR }
}
pub fn set_pending_ip(ip: [u8; 4]) {
    unsafe { PENDING_IP = ip }
}
pub fn get_pending_ip() -> [u8; 4] {
    unsafe { PENDING_IP }
}
pub fn clear_pending_ip() {
    unsafe { PENDING_IP = [0, 0, 0, 0] }
}
pub fn get_netmask() -> [u8; 4] {
    unsafe { NETMASK }
}
pub fn get_gateway() -> [u8; 4] {
    unsafe { GATEWAY }
}

pub fn set_dns(dns: [u8; 4]) {
    // Ignore an all-zero/missing option 6.
    if dns != [0, 0, 0, 0] {
        unsafe {
            DNS_SERVER = dns;
        }
        crate::mesa_println!(
            "[NET] DNS server: {}.{}.{}.{}",
            dns[0], dns[1], dns[2], dns[3]
        );
    }
}

pub fn get_dns() -> [u8; 4] {
    unsafe { DNS_SERVER }
}

/// Drop any resolver learned from a previous network before a new DHCP run.
pub fn reset_dns() {
    unsafe {
        DNS_SERVER = [8, 8, 8, 8];
    }
}

pub fn next_ip_id() -> u16 {
    unsafe {
        IP_ID_COUNTER = IP_ID_COUNTER.wrapping_add(1);
        IP_ID_COUNTER
    }
}

pub fn is_virtio() -> bool {
    crate::drivers::net::virtio_net::is_active()
}

/// Devuelve la MAC de la NIC activa (virtio-net primero, RTL8822CE, RTL8139 como fallback)
pub fn get_mac() -> [u8; 6] {
    if let Some(mac) = crate::drivers::net::virtio_net::get_mac() {
        return mac;
    }
    if let Some(mac) = crate::drivers::net::rtl8822ce::get_mac() {
        return mac;
    }
    if let Some(mac) = crate::drivers::net::rtl8139::get_mac() {
        return mac;
    }
    [0x52, 0x54, 0x00, 0x12, 0x34, 0x56] // Fallback estático
}

/// Envía un frame Ethernet usando la NIC activa
pub fn send_ethernet(
    dest_mac: [u8; 6],
    ethertype: u16,
    payload: &[u8],
) -> Result<(), &'static str> {
    let src_mac = get_mac();
    let frame = ethernet::create_frame(src_mac, dest_mac, ethertype, payload);

    // 1. virtio-net (QEMU)
    if crate::drivers::net::virtio_net::is_active() {
        return crate::drivers::net::virtio_net::send_packet(&frame);
    }

    // 2. RTL8822CE WiFi (hardware real)
    if crate::drivers::net::rtl8822ce::is_ready() {
        return crate::drivers::net::rtl8822ce::send_packet(&frame);
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

    // 2. RTL8822CE WiFi: drain pending RX and run periodic keepalive.
    // (RX is primarily IRQ-driven; polling here acts as a safety net and lets
    //  the connection stay alive while the shell/other commands are idle.)
    crate::drivers::net::rtl8822ce::wifi_tick();

    // 3. RTL8139 (hardware físico)
    while let Some(packet) = crate::drivers::net::rtl8139::poll_rx() {
        handle_packet(&packet);
    }

    // 4. USB RNDIS has been removed
}

/// Receive a raw 802.11 frame from the WiFi driver
/// The frame includes the 802.11 header + LLC/SNAP header + IP payload
pub fn wifi_recv(frame: &[u8]) {
    // WiFi frames have an 802.11 header, but for data frames
    // the payload after LLC/SNAP is an Ethernet-compatible frame.
    // We pass it through handle_packet which expects an Ethernet frame.
    // For now, we try to extract the Ethernet-like portion.
    handle_packet(frame);
}

pub fn handle_packet(packet: &[u8]) {
    if let Some(frame) = ethernet::parse_frame(packet) {
        match frame.ethertype {
            0x0806 => arp::handle_arp(&frame.payload),
            0x0800 => ipv4::handle_ipv4(&frame.payload),
            0x888E => {
                // EAPOL (WPA2 4-way handshake)
                crate::net::wpa::handle_eapol(&frame.payload, frame.src_mac);
            }
            0x86dd => { /* IPv6 — ignorar */ }
            other => {
                crate::serial_println!("[NET] Ethertype desconocido: {:#06x}", other);
            }
        }
    }
}
