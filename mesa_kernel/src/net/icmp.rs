use alloc::vec::Vec;
use spin::Mutex;

static PING_REPLIES: Mutex<Vec<([u8; 4], u16, u16, u8)>> = Mutex::new(Vec::new());

pub fn handle_icmp(data: &[u8], src_ip: [u8; 4], ttl: u8) {
    if data.len() < 8 {
        return;
    }
    
    let icmp_type = data[0];
    let icmp_code = data[1];
    
    crate::serial_println!("[ICMP] Inbound: type={} code={} from {}.{}.{}.{}",
        icmp_type, icmp_code, src_ip[0], src_ip[1], src_ip[2], src_ip[3]);

    match icmp_type {
        8 => {
            // Echo Request - send reply
            crate::serial_println!("[ICMP] Echo Request from {}.{}.{}.{}",
                src_ip[0], src_ip[1], src_ip[2], src_ip[3]);
            send_echo_reply(src_ip, data);
        }
        0 => {
            // Echo Reply
            let id = u16::from_be_bytes([data[4], data[5]]);
            let seq = u16::from_be_bytes([data[6], data[7]]);
            PING_REPLIES.lock().push((src_ip, id, seq, ttl));
            crate::serial_println!("[ICMP] Echo reply from {}.{}.{}.{} id={} seq={} ttl={}",
                src_ip[0], src_ip[1], src_ip[2], src_ip[3], id, seq, ttl);
        }
        3 => {
            crate::serial_println!("[ICMP] Destination Unreachable from {}.{}.{}.{} (code={})",
                src_ip[0], src_ip[1], src_ip[2], src_ip[3], icmp_code);
        }
        _ => {
            crate::serial_println!("[ICMP] Other type: {} from {}.{}.{}.{}",
                icmp_type, src_ip[0], src_ip[1], src_ip[2], src_ip[3]);
        }
    }
}

fn send_echo_reply(dest_ip: [u8; 4], request_data: &[u8]) {
    let mut reply = Vec::with_capacity(request_data.len());
    
    // Type = 0 (Echo Reply), Code = 0
    reply.push(0);
    reply.push(0);
    // Checksum (will calculate)
    reply.extend_from_slice(&0u16.to_be_bytes());
    // Copy rest of request (ID, Seq, Data)
    reply.extend_from_slice(&request_data[4..]);
    
    // Calculate checksum
    let checksum = calculate_icmp_checksum(&reply);
    reply[2] = (checksum >> 8) as u8;
    reply[3] = (checksum & 0xFF) as u8;
    
    crate::serial_println!("[ICMP] Outbound Echo Reply checksum: {:#x}", checksum);
    let _ = crate::net::ipv4::send_packet(dest_ip, 1, &reply);
}

pub fn send_ping(dest_ip: [u8; 4], id: u16, seq: u16) -> Result<(), &'static str> {
    let mut data = Vec::with_capacity(64);
    
    // Type = 8 (Echo Request), Code = 0
    data.push(8);
    data.push(0);
    // Checksum
    data.extend_from_slice(&0u16.to_be_bytes());
    // ID
    data.extend_from_slice(&id.to_be_bytes());
    // Sequence
    data.extend_from_slice(&seq.to_be_bytes());
    
    // Payload (56 bytes to make total packet 64 bytes)
    for i in 0..56 {
        data.push((i % 256) as u8);
    }
    
    // Calculate checksum
    let checksum = calculate_icmp_checksum(&data);
    data[2] = (checksum >> 8) as u8;
    data[3] = (checksum & 0xFF) as u8;
    
    crate::serial_println!("[ICMP] Outbound Echo Request to {}.{}.{}.{} (seq={}) checksum: {:#x}", 
        dest_ip[0], dest_ip[1], dest_ip[2], dest_ip[3], seq, checksum);
    crate::net::hex_dump(&data, 32);
    
    crate::net::ipv4::send_packet(dest_ip, 1, &data)
}

fn calculate_icmp_checksum(data: &[u8]) -> u16 {
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

pub fn wait_for_reply(id: u16, seq: u16, timeout_ms: usize) -> Option<([u8; 4], u8)> {
    let start_ticks = crate::curr_arch::get_ticks();
    // Aproximadamente 1 tick = 55ms. 1000ms / 55 = 18 ticks.
    let timeout_ticks = (timeout_ms as u64) / 55;
    let timeout_ticks = if timeout_ticks == 0 { 1 } else { timeout_ticks };
    
    let mut last_dot_tick = start_ticks;

    loop {
        crate::net::poll();
        
        // Check if we got a reply
        {
            let mut replies = PING_REPLIES.lock();
            if let Some(pos) = replies.iter().position(|(_, reply_id, reply_seq, _)| *reply_id == id && *reply_seq == seq) {
                let (ip, _, _, reply_ttl) = replies.remove(pos);
                return Some((ip, reply_ttl));
            }
        }
        
        let current_ticks = crate::curr_arch::get_ticks();
        
        // Timeout check
        if current_ticks.wrapping_sub(start_ticks) >= timeout_ticks {
            break;
        }

        // Print dot every 500ms (~9 ticks)
        if current_ticks.wrapping_sub(last_dot_tick) >= 9 {
            crate::mesa_print!(".");
            last_dot_tick = current_ticks;
        }

        // Yield to other tasks so we don't freeze the system
        crate::scheduler::yield_now();
        
        // Small spin to avoid burning 100% CPU in a tight loop if there are no other tasks
        for _ in 0..100000 {
            core::hint::spin_loop();
        }
    }
    
    crate::mesa_println!(); 
    crate::serial_println!("[ICMP] Timeout for id={} seq={}", id, seq);
    None
}
