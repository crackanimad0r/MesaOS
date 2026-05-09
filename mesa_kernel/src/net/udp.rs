use alloc::vec::Vec;

pub struct UdpPacket {
    pub src_port: u16,
    pub dest_port: u16,
    pub length: u16,
    pub checksum: u16,
    pub payload: Vec<u8>,
}

pub fn parse_packet(data: &[u8]) -> Option<UdpPacket> {
    if data.len() < 8 {
        return None;
    }
    
    let src_port = u16::from_be_bytes([data[0], data[1]]);
    let dest_port = u16::from_be_bytes([data[2], data[3]]);
    let length = u16::from_be_bytes([data[4], data[5]]);
    let checksum = u16::from_be_bytes([data[6], data[7]]);
    
    if data.len() < length as usize {
        return None;
    }
    
    let payload = data[8..length as usize].to_vec();
    
    Some(UdpPacket {
        src_port,
        dest_port,
        length,
        checksum,
        payload,
    })
}

pub fn create_packet(src_ip: [u8; 4], dest_ip: [u8; 4], src_port: u16, dest_port: u16, payload: &[u8]) -> Vec<u8> {
    let length = 8 + payload.len();
    let mut packet = Vec::with_capacity(length);
    
    packet.extend_from_slice(&src_port.to_be_bytes());
    packet.extend_from_slice(&dest_port.to_be_bytes());
    packet.extend_from_slice(&(length as u16).to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes()); // Placeholder for checksum
    packet.extend_from_slice(payload);
    
    // Calculate and insert checksum
    let checksum = calculate_udp_checksum(src_ip, dest_ip, &packet);
    packet[6] = (checksum >> 8) as u8;
    packet[7] = (checksum & 0xFF) as u8;
    
    packet
}

fn calculate_udp_checksum(src_ip: [u8; 4], dest_ip: [u8; 4], udp_data: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Pseudo-header
    for i in (0..4).step_by(2) {
        sum += u16::from_be_bytes([src_ip[i], src_ip[i+1]]) as u32;
        sum += u16::from_be_bytes([dest_ip[i], dest_ip[i+1]]) as u32;
    }
    sum += 17u32; // Protocol UDP
    sum += udp_data.len() as u32;

    // UDP Header + Payload
    for i in (0..udp_data.len()).step_by(2) {
        if i + 1 < udp_data.len() {
            sum += u16::from_be_bytes([udp_data[i], udp_data[i+1]]) as u32;
        } else {
            sum += (udp_data[i] as u32) << 8;
        }
    }

    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    let result = !sum as u16;
    if result == 0 { 0xFFFF } else { result }
}

pub fn handle_udp(data: &[u8], src_ip: [u8; 4]) {
    if let Some(packet) = parse_packet(data) {
        crate::serial_println!("[UDP] Packet: {}.{}.{}.{}:{} -> :{} (len={})", 
            src_ip[0], src_ip[1], src_ip[2], src_ip[3], packet.src_port, packet.dest_port, packet.length);
            
        match packet.dest_port {
            68 => {
                // DHCP Client
                crate::net::dhcp::handle_packet(&packet.payload);
            }
            54321 => {
                // DNS Response
                crate::net::dns::handle_dns_response(&packet.payload);
            }
            _ => {
                crate::serial_println!("[UDP] Port {} closed", packet.dest_port);
            }
        }
    }
}

pub fn send_packet(dest_ip: [u8; 4], src_port: u16, dest_port: u16, payload: &[u8]) -> Result<(), &'static str> {
    let src_ip = crate::net::get_ip();
    let udp_packet = create_packet(src_ip, dest_ip, src_port, dest_port, payload);
    crate::net::ipv4::send_packet(dest_ip, 17, &udp_packet)
}
