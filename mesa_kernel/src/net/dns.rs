// mesa_kernel/src/net/dns.rs

use crate::net::udp;
use alloc::string::String;
use alloc::vec::Vec;

/// Transaction ID of the query in flight. Responses with a different ID are
/// from a previous/other lookup and must be ignored.
static mut EXPECTED_DNS_ID: u16 = 0;
static mut DNS_QUERY_ID: u16 = 0x1234;

fn build_query(hostname: &str, id: u16) -> Vec<u8> {
    let mut packet = Vec::new();
    packet.extend_from_slice(&id.to_be_bytes());
    packet.extend_from_slice(&0x0100u16.to_be_bytes()); // RD
    packet.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    packet.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
    packet.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
    packet.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT
    for part in hostname.split('.') {
        packet.push(part.len() as u8);
        packet.extend_from_slice(part.as_bytes());
    }
    packet.push(0);
    packet.extend_from_slice(&1u16.to_be_bytes()); // Type A
    packet.extend_from_slice(&1u16.to_be_bytes()); // Class IN
    packet
}

/// Ask one server. Retransmits every ~1 s for up to 10 s: ISP routers with a
/// bad uplink often need several seconds (or a retransmission) to answer.
fn query(hostname: &str, server: [u8; 4]) -> Option<[u8; 4]> {
    let id = unsafe {
        DNS_QUERY_ID = DNS_QUERY_ID.wrapping_add(0x1111);
        DNS_QUERY_ID
    };
    let packet = build_query(hostname, id);
    unsafe {
        EXPECTED_DNS_ID = id;
        LAST_RESOLVED_IP = None;
    }
    crate::mesa_println!(
        "[DNS] query '{}' -> {}.{}.{}.{} (id={:#06x})",
        hostname, server[0], server[1], server[2], server[3], id
    );

    let start = crate::curr_arch::get_ticks();
    let mut last_send = start;
    let mut sends: u32 = 0;
    // Timer runs at ~100 Hz (1 tick = 10 ms): 1000 ticks == 10 s, resend
    // every 100 ticks (1 s) in case the first query is lost.
    while crate::curr_arch::get_ticks().wrapping_sub(start) < 1000 {
        let now = crate::curr_arch::get_ticks();
        if sends == 0 || now.wrapping_sub(last_send) >= 100 {
            let _ = udp::send_packet(server, crate::net::get_ip(), 54321, 53, &packet);
            last_send = now;
            sends += 1;
        }
        crate::net::poll();
        unsafe {
            if let Some(ip) = LAST_RESOLVED_IP {
                crate::mesa_println!(
                    "[DNS] OK '{}' -> {}.{}.{}.{} ({} envios)",
                    hostname, ip[0], ip[1], ip[2], ip[3], sends
                );
                return Some(ip);
            }
        }
        crate::linux::mdelay(1);
    }
    crate::mesa_println!(
        "[DNS] timeout '{}' con {}.{}.{}.{} ({} envios)",
        hostname, server[0], server[1], server[2], server[3], sends
    );
    None
}

pub fn resolve(hostname: &str) -> Option<[u8; 4]> {
    // Si ya es una IP, devolverla
    let parts: Vec<&str> = hostname.split('.').collect();
    if parts.len() == 4 {
        if let (Ok(p0), Ok(p1), Ok(p2), Ok(p3)) = (
            parts[0].parse::<u8>(),
            parts[1].parse::<u8>(),
            parts[2].parse::<u8>(),
            parts[3].parse::<u8>(),
        ) {
            return Some([p0, p1, p2, p3]);
        }
    }

    // Without a lease every DNS query carries src 0.0.0.0 and is silently
    // dropped by the router: fail fast instead of waiting 30 s.
    if crate::net::get_ip() == [0, 0, 0, 0] {
        crate::mesa_println!("[DNS] sin IP (DHCP no completó): no se puede resolver");
        return None;
    }

    // Try the DHCP-provided resolver first, then well-known publics. Some ISP
    // routers advertise a DNS that is unreachable/slow; others block 8.8.8.8
    // but answer on their own. Trying both makes resolution robust.
    let servers: [[u8; 4]; 3] = [crate::net::get_dns(), [8, 8, 8, 8], [1, 1, 1, 1]];
    let mut i = 0;
    while i < servers.len() {
        let server = servers[i];
        i += 1;
        if server == [0, 0, 0, 0] {
            continue;
        }
        let mut dup = false;
        let mut j = 0;
        while j + 1 < i {
            if servers[j] == server {
                dup = true;
                break;
            }
            j += 1;
        }
        if dup {
            continue;
        }
        if let Some(ip) = query(hostname, server) {
            return Some(ip);
        }
    }
    None
}

// Nota: Para este demo, implementaremos el manejador en net/mod.rs
// que guardará la última IP resuelta.
pub static mut LAST_RESOLVED_IP: Option<[u8; 4]> = None;

pub fn handle_dns_response(payload: &[u8]) {
    if payload.len() < 12 {
        return;
    }

    // Ignore responses to a different query (late reply from another server).
    let id = u16::from_be_bytes([payload[0], payload[1]]);
    let expected = unsafe { EXPECTED_DNS_ID };
    if id != expected {
        return;
    }

    let answers = u16::from_be_bytes([payload[6], payload[7]]);
    if answers == 0 {
        // NXDOMAIN / SERVFAIL / truncated: useful on screen to tell a broken
        // resolver apart from a lost packet.
        let flags = u16::from_be_bytes([payload[2], payload[3]]);
        crate::mesa_println!(
            "[DNS] respuesta sin A (rcode={}, tc={})",
            flags & 0x000F,
            (flags & 0x0200) != 0
        );
        return;
    }

    // Skip the question section (QNAME + QTYPE + QCLASS).
    let mut pos = 12;
    while pos < payload.len() && payload[pos] != 0 {
        if payload[pos] & 0xC0 == 0xC0 {
            pos += 2;
            break;
        }
        pos += (payload[pos] as usize) + 1;
    }
    if pos < payload.len() && payload[pos] == 0 {
        pos += 1;
    }
    pos += 4;
    if pos > payload.len() {
        return;
    }

    // Walk every answer: the first one may be a CNAME, so do not assume the
    // first RR is the A record. Names can be compressed (0xC0 pointer) or
    // literal labels; both are handled.
    for _ in 0..answers {
        if pos >= payload.len() {
            break;
        }
        if payload[pos] & 0xC0 == 0xC0 {
            pos += 2;
        } else {
            while pos < payload.len() && payload[pos] != 0 {
                pos += (payload[pos] as usize) + 1;
            }
            pos += 1;
        }
        if pos + 10 > payload.len() {
            break;
        }
        let rtype = u16::from_be_bytes([payload[pos], payload[pos + 1]]);
        let rlen = u16::from_be_bytes([payload[pos + 8], payload[pos + 9]]) as usize;
        pos += 10;
        if pos + rlen > payload.len() {
            break;
        }
        if rtype == 1 && rlen == 4 {
            let ip = [
                payload[pos],
                payload[pos + 1],
                payload[pos + 2],
                payload[pos + 3],
            ];
            unsafe {
                LAST_RESOLVED_IP = Some(ip);
            }
            return;
        }
        pos += rlen;
    }
}
