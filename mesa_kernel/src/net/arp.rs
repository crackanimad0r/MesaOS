use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use spin::Mutex;

static ARP_CACHE: Mutex<BTreeMap<[u8; 4], [u8; 6]>> = Mutex::new(BTreeMap::new());

pub fn handle_arp(data: &[u8]) {
    if data.len() < 28 {
        return;
    }
    
    let htype = u16::from_be_bytes([data[0], data[1]]);
    let ptype = u16::from_be_bytes([data[2], data[3]]);
    let hlen = data[4];
    let plen = data[5];
    let oper = u16::from_be_bytes([data[6], data[7]]);
    
    if htype != 1 || ptype != 0x0800 || hlen != 6 || plen != 4 {
        return;
    }
    
    let sender_mac = [data[8], data[9], data[10], data[11], data[12], data[13]];
    let sender_ip = [data[14], data[15], data[16], data[17]];
    let target_ip = [data[24], data[25], data[26], data[27]];
    
    crate::serial_println!("[ARP] Inbound: {}.{}.{}.{} -> Op={}",
        sender_ip[0], sender_ip[1], sender_ip[2], sender_ip[3], oper);
    
    // Update cache for any ARP we see
    ARP_CACHE.lock().insert(sender_ip, sender_mac);
    
    if oper == 1 {
        // ARP Request
        let our_ip = crate::net::get_ip();
        if target_ip == our_ip {
            crate::serial_println!("[ARP] Replying to {}.{}.{}.{}",
                sender_ip[0], sender_ip[1], sender_ip[2], sender_ip[3]);
            send_arp_reply(sender_mac, sender_ip);
        }
    } else if oper == 2 {
        crate::serial_println!("[ARP] Resolved from reply: {}.{}.{}.{} -> {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            sender_ip[0], sender_ip[1], sender_ip[2], sender_ip[3],
            sender_mac[0], sender_mac[1], sender_mac[2], sender_mac[3], sender_mac[4], sender_mac[5]);
    }
}

pub fn send_arp_request(target_ip: [u8; 4]) {
    let our_mac = crate::net::get_mac();
    let our_ip = crate::net::get_ip();
    
    let mut arp_packet = Vec::with_capacity(28);
    arp_packet.extend_from_slice(&1u16.to_be_bytes());      // Hardware type (Ethernet)
    arp_packet.extend_from_slice(&0x0800u16.to_be_bytes()); // Protocol type (IPv4)
    arp_packet.push(6);                                      // Hardware size
    arp_packet.push(4);                                      // Protocol size
    arp_packet.extend_from_slice(&1u16.to_be_bytes());      // Opcode (request)
    arp_packet.extend_from_slice(&our_mac);                 // Sender MAC
    arp_packet.extend_from_slice(&our_ip);                  // Sender IP
    arp_packet.extend_from_slice(&[0u8; 6]);                // Target MAC (unknown)
    arp_packet.extend_from_slice(&target_ip);               // Target IP
    
    let broadcast_mac = [0xFFu8; 6];
    crate::serial_println!("[ARP] Sending request for {}.{}.{}.{}", 
        target_ip[0], target_ip[1], target_ip[2], target_ip[3]);
    let _ = crate::net::send_ethernet(broadcast_mac, 0x0806, &arp_packet);
}

fn send_arp_reply(target_mac: [u8; 6], target_ip: [u8; 4]) {
    let our_mac = crate::net::get_mac();
    let our_ip = crate::net::get_ip();
    
    let mut arp_packet = Vec::with_capacity(28);
    arp_packet.extend_from_slice(&1u16.to_be_bytes());
    arp_packet.extend_from_slice(&0x0800u16.to_be_bytes());
    arp_packet.push(6);
    arp_packet.push(4);
    arp_packet.extend_from_slice(&2u16.to_be_bytes());      // Opcode (reply)
    arp_packet.extend_from_slice(&our_mac);
    arp_packet.extend_from_slice(&our_ip);
    arp_packet.extend_from_slice(&target_mac);
    arp_packet.extend_from_slice(&target_ip);
    
    let _ = crate::net::send_ethernet(target_mac, 0x0806, &arp_packet);
}

pub fn get_from_cache(ip: [u8; 4]) -> Option<[u8; 6]> {
    ARP_CACHE.lock().get(&ip).copied()
}

pub fn resolve(ip: [u8; 4]) -> Option<[u8; 6]> {
    resolve_with_timeout(ip, 200) // 200 * 10ms = 2s
}

pub fn resolve_with_timeout(ip: [u8; 4], iterations: usize) -> Option<[u8; 6]> {
    // Check cache first
    if let Some(mac) = get_from_cache(ip) {
        return Some(mac);
    }
    
    crate::serial_println!("[ARP] Resolving {}.{}.{}.{}...", ip[0], ip[1], ip[2], ip[3]);
    send_arp_request(ip);
    
    // Wait and poll
    for i in 0..iterations {
        crate::net::poll();
        
        if let Some(mac) = get_from_cache(ip) {
            return Some(mac);
        }

        // Yield to other tasks
        crate::scheduler::yield_now();
        
        // Small wait
        for _ in 0..100000 {
            core::hint::spin_loop();
        }
    }
    
    crate::serial_println!("[ARP] ERROR: No se pudo resolver {}.{}.{}.{} tras {} intentos", 
        ip[0], ip[1], ip[2], ip[3], iterations);
    None
}

pub fn get_cache() -> Vec<([u8; 4], [u8; 6])> {
    ARP_CACHE.lock().iter().map(|(k, v)| (*k, *v)).collect()
}

pub fn scan_neighbors() {
    let our_ip = crate::net::get_ip();
    let netmask = crate::net::get_netmask();
    
    // Calculate network base
    let mut network = [0u8; 4];
    for i in 0..4 {
        network[i] = our_ip[i] & netmask[i];
    }
    
    crate::serial_println!("[ARP] Scanning subnet {}.{}.{}.x...", network[0], network[1], network[2]);
    
    // Scan first 50 IPs (safeguard against long delays)
    for i in 1..51 {
        let mut target_ip = network;
        target_ip[3] = i;
        
        if target_ip == our_ip { continue; }
        
        send_arp_request(target_ip);
        
        // Very brief pause to avoid flooding and give time for some replies
        for _ in 0..1000000 { core::hint::spin_loop(); }
        crate::net::poll();
    }
}
