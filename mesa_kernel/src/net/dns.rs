// mesa_kernel/src/net/dns.rs

use alloc::vec::Vec;
use alloc::string::String;
use crate::net::udp;

pub fn resolve(hostname: &str) -> Option<[u8; 4]> {
    // Si ya es una IP, devolverla
    let parts: Vec<&str> = hostname.split('.').collect();
    if parts.len() == 4 {
        if let (Ok(p0), Ok(p1), Ok(p2), Ok(p3)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>(), parts[2].parse::<u8>(), parts[3].parse::<u8>()) {
            return Some([p0, p1, p2, p3]);
        }
    }

    let dns_server = [8, 8, 8, 8]; // Google DNS por defecto
    let mut packet = Vec::new();
    
    // Header
    packet.extend_from_slice(&0x1234u16.to_be_bytes()); // ID
    packet.extend_from_slice(&0x0100u16.to_be_bytes()); // Flags: Standard query, recursion desired
    packet.extend_from_slice(&1u16.to_be_bytes());      // Questions
    packet.extend_from_slice(&0u16.to_be_bytes());      // Answer RRs
    packet.extend_from_slice(&0u16.to_be_bytes());      // Authority RRs
    packet.extend_from_slice(&0u16.to_be_bytes());      // Additional RRs
    
    // Question
    for part in hostname.split('.') {
        packet.push(part.len() as u8);
        packet.extend_from_slice(part.as_bytes());
    }
    packet.push(0); // End of name
    
    packet.extend_from_slice(&1u16.to_be_bytes()); // Type A
    packet.extend_from_slice(&1u16.to_be_bytes()); // Class IN
    
    // Enviar y esperar respuesta
    if udp::send_packet(dns_server, 54321, 53, &packet).is_err() {
        return None;
    }
    
    // Esperar respuesta (máximo 2 segundos = 36 ticks aprox)
    let start_tick = crate::curr_arch::get_ticks();
    while crate::curr_arch::get_ticks().wrapping_sub(start_tick) < 36 {
        crate::net::poll();
        unsafe {
            if let Some(ip) = LAST_RESOLVED_IP {
                return Some(ip);
            }
        }
        core::hint::spin_loop();
    }
    None 
}

// Nota: Para este demo, implementaremos el manejador en net/mod.rs
// que guardará la última IP resuelta.
pub static mut LAST_RESOLVED_IP: Option<[u8; 4]> = None;

pub fn handle_dns_response(payload: &[u8]) {
    if payload.len() < 12 { return; }
    
    let answers = u16::from_be_bytes([payload[6], payload[7]]);
    if answers == 0 { return; }
    
    // Saltar el header y la pregunta
    let mut pos = 12;
    while pos < payload.len() && payload[pos] != 0 {
        pos += (payload[pos] as usize) + 1;
    }
    pos += 5; // Salto del 0 final y Type/Class de la pregunta
    
    // Parsear primera respuesta
    if pos + 12 <= payload.len() {
        // RR: Name(2) Type(2) Class(2) TTL(4) DataLen(2) Data(4)
        // Usamos saltos fijos si el nombre es un puntero (0xC0..)
        if payload[pos] & 0xC0 == 0xC0 { pos += 2; } else { return; }
        
        let rtype = u16::from_be_bytes([payload[pos], payload[pos+1]]);
        let rlen = u16::from_be_bytes([payload[pos+8], payload[pos+9]]);
        
        if rtype == 1 && rlen == 4 { // Type A (IPv4)
            let ip = [payload[pos+10], payload[pos+11], payload[pos+12], payload[pos+13]];
            unsafe { LAST_RESOLVED_IP = Some(ip); }
        }
    }
}
