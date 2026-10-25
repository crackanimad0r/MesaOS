use alloc::vec::Vec;
use spin::Mutex;

static PING_REPLIES: Mutex<Vec<([u8; 4], u16, u16, u8)>> = Mutex::new(Vec::new());

pub fn handle_icmp(data: &[u8], src_ip: [u8; 4], ttl: u8) {
    if data.len() < 8 {
        return;
    }

    let icmp_type = data[0];
    let icmp_code = data[1];

    crate::serial_println!(
        "[ICMP] Inbound: type={} code={} from {}.{}.{}.{}",
        icmp_type,
        icmp_code,
        src_ip[0],
        src_ip[1],
        src_ip[2],
        src_ip[3]
    );

    match icmp_type {
        8 => {
            // Echo Request. Never answer 0.0.0.0: with no lease, our broadcast
            // echo request comes back from the AP and replying to 0.0.0.0
            // creates a loop that looks like a successful ping.
            if src_ip == [0, 0, 0, 0] {
                return;
            }
            crate::serial_println!(
                "[ICMP] Echo Request from {}.{}.{}.{}",
                src_ip[0],
                src_ip[1],
                src_ip[2],
                src_ip[3]
            );
            send_echo_reply(src_ip, data);
        }
        0 => {
            // Echo Reply. Drop 0.0.0.0 sources (our own frames reflected back).
            if src_ip == [0, 0, 0, 0] {
                return;
            }
            let id = u16::from_be_bytes([data[4], data[5]]);
            let seq = u16::from_be_bytes([data[6], data[7]]);
            PING_REPLIES.lock().push((src_ip, id, seq, ttl));
            crate::mesa_println!(
                "[ICMP] Echo reply de {}.{}.{}.{} id={} seq={} ttl={}",
                src_ip[0],
                src_ip[1],
                src_ip[2],
                src_ip[3],
                id,
                seq,
                ttl
            );
        }
        3 => {
            crate::serial_println!(
                "[ICMP] Destination Unreachable from {}.{}.{}.{} (code={})",
                src_ip[0],
                src_ip[1],
                src_ip[2],
                src_ip[3],
                icmp_code
            );
        }
        _ => {
            crate::serial_println!(
                "[ICMP] Other type: {} from {}.{}.{}.{}",
                icmp_type,
                src_ip[0],
                src_ip[1],
                src_ip[2],
                src_ip[3]
            );
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
    let _ = crate::net::ipv4::send_packet(dest_ip, crate::net::get_ip(), 1, &reply);
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

    crate::serial_println!(
        "[ICMP] Outbound Echo Request to {}.{}.{}.{} (seq={}) checksum: {:#x}",
        dest_ip[0],
        dest_ip[1],
        dest_ip[2],
        dest_ip[3],
        seq,
        checksum
    );
    crate::net::hex_dump(&data, 32);

    crate::net::ipv4::send_packet(dest_ip, crate::net::get_ip(), 1, &data)
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

/// Discard pending replies (call right before sending a new request).
pub fn clear_replies() {
    PING_REPLIES.lock().clear();
}

pub fn wait_for_reply(
    _id: u16,
    seq: u16,
    timeout_ms: usize,
    expected_ip: [u8; 4],
) -> Option<([u8; 4], u8)> {
    let start_ticks = crate::curr_arch::get_ticks();
    // Timer is ~100 Hz: 1 tick = 10 ms.
    let timeout_ticks = (timeout_ms as u64 / 10).max(1);

    let mut last_dot_tick = start_ticks;

    loop {
        crate::net::poll();

        // Check if we got a reply
        {
            let mut replies = PING_REPLIES.lock();
            // Match by sequence + source only: many routers/NATs rewrite the
            // ICMP identifier of relayed pings, so requiring the original id
            // dropped every reply to external hosts (ping "almost never").
            if let Some(pos) = replies.iter().position(|(ip, _reply_id, reply_seq, _)| {
                *reply_seq == seq && *ip == expected_ip
            }) {
                let (ip, _, _, reply_ttl) = replies.remove(pos);
                return Some((ip, reply_ttl));
            }
        }

        let current_ticks = crate::curr_arch::get_ticks();

        // Timeout check
        if current_ticks.wrapping_sub(start_ticks) >= timeout_ticks {
            break;
        }

        // Print dot every 500 ms (50 ticks at ~100 Hz)
        if current_ticks.wrapping_sub(last_dot_tick) >= 50 {
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
    crate::serial_println!("[ICMP] Timeout for seq={}", seq);
    None
}
