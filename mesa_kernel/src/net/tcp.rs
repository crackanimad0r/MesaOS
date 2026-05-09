// mesa_kernel/src/net/tcp.rs

use alloc::vec::Vec;
use crate::net::ipv4;

#[derive(Debug, Clone, Copy)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dest_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub flags: u16,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
}

pub const TCP_FLAG_FIN: u16 = 0x01;
pub const TCP_FLAG_SYN: u16 = 0x02;
pub const TCP_FLAG_RST: u16 = 0x04;
pub const TCP_FLAG_PSH: u16 = 0x08;
pub const TCP_FLAG_ACK: u16 = 0x10;

impl TcpHeader {
    pub fn new(src_port: u16, dest_port: u16, seq: u32, ack: u32, flags: u16) -> Self {
        Self {
            src_port,
            dest_port,
            seq_num: seq,
            ack_num: ack,
            flags,
            window_size: 8192,
            checksum: 0,
            urgent_ptr: 0,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(20);
        data.extend_from_slice(&self.src_port.to_be_bytes());
        data.extend_from_slice(&self.dest_port.to_be_bytes());
        data.extend_from_slice(&self.seq_num.to_be_bytes());
        data.extend_from_slice(&self.ack_num.to_be_bytes());
        
        // Data Offset (5 words = 20 bytes) + Reserved + Flags
        let header_len_and_flags = (5u16 << 12) | (self.flags & 0x3F);
        data.extend_from_slice(&header_len_and_flags.to_be_bytes());
        
        data.extend_from_slice(&self.window_size.to_be_bytes());
        data.extend_from_slice(&self.checksum.to_be_bytes());
        data.extend_from_slice(&self.urgent_ptr.to_be_bytes());
        data
    }

    pub fn from_bytes(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < 20 { return None; }
        
        let offset_flags = u16::from_be_bytes([data[12], data[13]]);
        let header_len = ((offset_flags >> 12) as usize) * 4;
        
        if data.len() < header_len { return None; }
        
        let header = Self {
            src_port: u16::from_be_bytes([data[0], data[1]]),
            dest_port: u16::from_be_bytes([data[2], data[3]]),
            seq_num: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            ack_num: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
            flags: offset_flags & 0x3F,
            window_size: u16::from_be_bytes([data[14], data[15]]),
            checksum: u16::from_be_bytes([data[16], data[17]]),
            urgent_ptr: u16::from_be_bytes([data[18], data[19]]),
        };
        
        Some((header, &data[header_len..]))
    }
}

pub fn calculate_tcp_checksum(src_ip: [u8; 4], dest_ip: [u8; 4], tcp_data: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Pseudo-header
    for i in (0..4).step_by(2) {
        sum += u16::from_be_bytes([src_ip[i], src_ip[i+1]]) as u32;
        sum += u16::from_be_bytes([dest_ip[i], dest_ip[i+1]]) as u32;
    }
    sum += 6u32; // Protocol TCP
    sum += tcp_data.len() as u32;

    // TCP Header + Payload
    for i in (0..tcp_data.len()).step_by(2) {
        if i + 1 < tcp_data.len() {
            sum += u16::from_be_bytes([tcp_data[i], tcp_data[i+1]]) as u32;
        } else {
            sum += (tcp_data[i] as u32) << 8;
        }
    }

    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !sum as u16
}

pub fn send_tcp_packet(dest_ip: [u8; 4], mut header: TcpHeader, payload: &[u8]) -> Result<(), &'static str> {
    let mut tcp_data = header.to_bytes();
    tcp_data.extend_from_slice(payload);
    
    let checksum = calculate_tcp_checksum(crate::net::get_ip(), dest_ip, &tcp_data);
    // Insert checksum at bytes 16-17
    tcp_data[16] = (checksum >> 8) as u8;
    tcp_data[17] = (checksum & 0xFF) as u8;
    
    ipv4::send_packet(dest_ip, 6, &tcp_data)
}

pub static mut LAST_TCP_PACKETS: Vec<(TcpHeader, Vec<u8>)> = Vec::new();

pub fn handle_tcp(data: &[u8], _src_ip: [u8; 4]) {
    if let Some((header, payload)) = TcpHeader::from_bytes(data) {
        unsafe {
            LAST_TCP_PACKETS.push((header, payload.to_vec()));
        }
    }
}
