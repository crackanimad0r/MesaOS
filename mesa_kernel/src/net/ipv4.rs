use alloc::vec::Vec;

pub struct Ipv4Packet {
    pub src_ip: [u8; 4],
    pub dest_ip: [u8; 4],
    pub protocol: u8,
    pub ttl: u8,
    pub payload: Vec<u8>,
}

pub fn parse_packet(data: &[u8]) -> Option<Ipv4Packet> {
    if data.len() < 20 {
        return None;
    }
    
    let version = data[0] >> 4;
    let ihl = (data[0] & 0x0F) as usize;
    
    if version != 4 || ihl < 5 {
        return None;
    }
    
    let header_len = ihl * 4;
    if data.len() < header_len {
        return None;
    }
    
    let total_len = u16::from_be_bytes([data[2], data[3]]) as usize;
    if data.len() < total_len {
        return None;
    }
    
    let protocol = data[9];
    let ttl = data[8];
    let src_ip = [data[12], data[13], data[14], data[15]];
    let dest_ip = [data[16], data[17], data[18], data[19]];
    
    let payload = data[header_len..total_len].to_vec();
    
    Some(Ipv4Packet {
        src_ip,
        dest_ip,
        protocol,
        ttl,
        payload,
    })
}

pub fn create_packet(src_ip: [u8; 4], dest_ip: [u8; 4], protocol: u8, payload: &[u8]) -> Vec<u8> {
    let total_length = 20 + payload.len();
    let mut packet = Vec::with_capacity(total_length);
    
    // Version (4) + IHL (5)
    packet.push(0x45);
    // DSCP + ECN
    packet.push(0x00);
    // Total length
    packet.extend_from_slice(&(total_length as u16).to_be_bytes());
    // Identification
    let id = crate::net::next_ip_id();
    packet.extend_from_slice(&id.to_be_bytes());
    // Flags + Fragment offset
    packet.extend_from_slice(&0u16.to_be_bytes());
    // TTL
    packet.push(64);
    // Protocol
    packet.push(protocol);
    // Header checksum (calculate later)
    packet.extend_from_slice(&0u16.to_be_bytes());
    // Source IP
    packet.extend_from_slice(&src_ip);
    // Destination IP
    packet.extend_from_slice(&dest_ip);
    
    // Calculate and insert checksum
    let checksum = calculate_checksum(&packet[..20]);
    packet[10] = (checksum >> 8) as u8;
    packet[11] = (checksum & 0xFF) as u8;
    
    // Payload
    packet.extend_from_slice(payload);
    
    packet
}

fn calculate_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    
    for i in (0..data.len()).step_by(2) {
        let word = if i + 1 < data.len() {
            u16::from_be_bytes([data[i], data[i + 1]]) as u32
        } else {
            (data[i] as u32) << 8
        };
        sum += word;
    }
    
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    
    !sum as u16
}

pub fn handle_ipv4(data: &[u8]) {
    if let Some(packet) = parse_packet(data) {
        let our_ip = crate::net::get_ip();

        if packet.dest_ip != our_ip && packet.dest_ip != [255, 255, 255, 255] {
            return; // No es para nosotros
        }

        match packet.protocol {
            1  => crate::net::icmp::handle_icmp(&packet.payload, packet.src_ip, packet.ttl),
            6  => crate::net::tcp::handle_tcp(&packet.payload, packet.src_ip),
            17 => crate::net::udp::handle_udp(&packet.payload, packet.src_ip),
            _  => {
                crate::serial_println!("[IPv4] Protocolo no manejado: {}", packet.protocol);
            }
        }
    }
}

pub fn send_packet(dest_ip: [u8; 4], protocol: u8, payload: &[u8]) -> Result<(), &'static str> {
    let src_ip = crate::net::get_ip();

    let dest_mac = if dest_ip == [255, 255, 255, 255] {
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    } else {
        // Para destinos no locales usar el gateway
        let target_ip = if is_local(dest_ip) { dest_ip } else { crate::net::get_gateway() };
        crate::net::arp::resolve(target_ip).ok_or("ARP resolution failed")?
    };

    let ip_packet = create_packet(src_ip, dest_ip, protocol, payload);
    crate::serial_println!("[IPv4] TX -> {}.{}.{}.{} proto={} via {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        dest_ip[0], dest_ip[1], dest_ip[2], dest_ip[3], protocol,
        dest_mac[0], dest_mac[1], dest_mac[2], dest_mac[3], dest_mac[4], dest_mac[5]);

    crate::net::send_ethernet(dest_mac, 0x0800, &ip_packet)
}

fn is_local(ip: [u8; 4]) -> bool {
    let our_ip = crate::net::get_ip();
    let netmask = crate::net::get_netmask();
    
    for i in 0..4 {
        if (ip[i] & netmask[i]) != (our_ip[i] & netmask[i]) {
            return false;
        }
    }
    true
}
