use alloc::vec::Vec;
use core::sync::atomic::{AtomicU8, Ordering};

pub const DHCP_SERVER_PORT: u16 = 67;
pub const DHCP_CLIENT_PORT: u16 = 68;

pub const MAGIC_COOKIE: u32 = 0x63825363;

pub const OPT_SUBNET_MASK: u8 = 1;
pub const OPT_ROUTER: u8 = 3;
pub const OPT_DNS: u8 = 6;
pub const OPT_REQUESTED_IP: u8 = 50;
pub const OPT_MESSAGE_TYPE: u8 = 53;
pub const OPT_SERVER_ID: u8 = 54;
pub const OPT_PARAM_REQUEST: u8 = 55;
pub const OPT_LEASE_TIME: u8 = 51;   // DHCP lease time (seconds)
pub const OPT_RENEWAL_TIME: u8 = 58; // T1 renewal time
pub const OPT_REBIND_TIME: u8 = 59;  // T2 rebind time
pub const OPT_CLIENT_ID: u8 = 61;
pub const OPT_MAX_MSG_SIZE: u8 = 57;
pub const OPT_END: u8 = 255;

pub const MSG_DISCOVER: u8 = 1;
pub const MSG_OFFER: u8 = 2;
pub const MSG_REQUEST: u8 = 3;
pub const MSG_NAK: u8 = 6;
pub const MSG_ACK: u8 = 5;

// Global transaction ID to match requests/responses
static mut CURRENT_XID: u32 = 0x12345678;

// DNS server seen in the OFFER; some servers omit option 6 from the ACK.
static mut OFFER_DNS: [u8; 4] = [0, 0, 0, 0];

// DHCP lease tracking (for renewal/rebind)
// Lease expiry in ticks (~100 Hz timer), 0 = no active lease
static mut LEASE_EXPIRY_TICKS: u64 = 0;

// DHCP exchange phase. The connect loop uses this to keep waiting for the
// DHCPACK after an OFFER: when the router answers the last DISCOVER just
// before the per-attempt deadline, the old loop gave up right after sending
// the REQUEST and never saw the ACK.
pub const PHASE_IDLE: u8 = 0;
pub const PHASE_DISCOVER: u8 = 1;
pub const PHASE_REQUEST: u8 = 2;
pub const PHASE_BOUND: u8 = 3;

static DHCP_PHASE: AtomicU8 = AtomicU8::new(PHASE_IDLE);
static REQUEST_TRIES: AtomicU8 = AtomicU8::new(0);
static mut OFFERED_IP: [u8; 4] = [0, 0, 0, 0];
static mut OFFERED_SERVER: [u8; 4] = [0, 0, 0, 0];

pub fn phase() -> u8 {
    DHCP_PHASE.load(Ordering::Relaxed)
}

/// True after an OFFER was accepted and a REQUEST is in flight.
pub fn offer_pending() -> bool {
    DHCP_PHASE.load(Ordering::Relaxed) == PHASE_REQUEST
}

/// Resend the REQUEST for the last OFFER (same xid) when the ACK was lost.
/// The first attempts are broadcast; later ones are unicast to the AP's MAC so
/// routers that ignore broadcast REQUESTs still see the second half.
pub fn resend_request() -> Result<(), &'static str> {
    let (ip, srv) = unsafe { (OFFERED_IP, OFFERED_SERVER) };
    if ip == [0, 0, 0, 0] {
        return Err("sin oferta DHCP");
    }
    send_request(ip, srv);
    Ok(())
}

pub fn send_discover() -> Result<(), &'static str> {
    let mac = crate::net::get_mac();

    // New lease attempt: forget the previous network's resolver until this
    // DHCP exchange provides one (prevents using a stale DNS across networks).
    crate::net::reset_dns();
    crate::net::clear_pending_ip();
    unsafe {
        OFFER_DNS = [0, 0, 0, 0];
    }
    DHCP_PHASE.store(PHASE_DISCOVER, Ordering::Relaxed);
    REQUEST_TRIES.store(0, Ordering::Relaxed);

    // Increment XID
    let xid = unsafe {
        CURRENT_XID = CURRENT_XID.wrapping_add(1);
        CURRENT_XID
    };

    // Construct DHCP Packet
    let mut packet = Vec::with_capacity(300);

    // BOOTP Header
    packet.push(1); // op: BOOTREQUEST
    packet.push(1); // htype: Ethernet
    packet.push(6); // hlen: Mac length
    packet.push(0); // hops

    // xid
    packet.extend_from_slice(&xid.to_be_bytes());

    packet.extend_from_slice(&0u16.to_be_bytes()); // secs
    // Broadcast flag (bit 15): mobile hotspots drop unicast responses to
    // 0.0.0.0 clients, so keep asking for a broadcast OFFER here. The REQUEST
    // switches to a unicast reply later (see build_request_packet).
    packet.extend_from_slice(&0x8000u16.to_be_bytes()); // flags (Broadcast)

    packet.extend_from_slice(&[0, 0, 0, 0]); // ciaddr
    packet.extend_from_slice(&[0, 0, 0, 0]); // yiaddr
    packet.extend_from_slice(&[0, 0, 0, 0]); // siaddr
    packet.extend_from_slice(&[0, 0, 0, 0]); // giaddr

    packet.extend_from_slice(&mac); // chaddr (6 bytes)
    packet.extend_from_slice(&[0u8; 10]); // chaddr padding (10 bytes)

    packet.extend_from_slice(&[0u8; 64]); // sname
    packet.extend_from_slice(&[0u8; 128]); // file

    // DHCP Options
    packet.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());

    // Option 53: Message Type = DISCOVER
    packet.push(OPT_MESSAGE_TYPE);
    packet.push(1); // Length
    packet.push(MSG_DISCOVER);

    // Option 61: Client Identifier (type 1 = Ethernet + MAC). Some routers key
    // the lease on it and ignore requests that do not repeat it.
    packet.push(OPT_CLIENT_ID);
    packet.push(7);
    packet.push(0x01);
    packet.extend_from_slice(&mac);

    // Option 57: Maximum DHCP Message Size.
    packet.push(OPT_MAX_MSG_SIZE);
    packet.push(2);
    packet.extend_from_slice(&576u16.to_be_bytes());

    // Option 55: Parameter Request List
    packet.push(OPT_PARAM_REQUEST);
    packet.push(3); // Length
    packet.push(OPT_SUBNET_MASK);
    packet.push(OPT_ROUTER);
    packet.push(OPT_DNS);

    packet.push(OPT_END);
    // BOOTP requires at least 300 bytes; pad with zero (pad) options.
    while packet.len() < 300 {
        packet.push(0);
    }

    crate::mesa_println!("  [DHCP] Enviando DISCOVER (xid={:#x})...", xid);
    // DHCP DISCOVER must be sourced from 0.0.0.0 (client has no IP yet)
    crate::net::udp::send_packet(
        [255, 255, 255, 255],
        [0, 0, 0, 0],
        DHCP_CLIENT_PORT,
        DHCP_SERVER_PORT,
        &packet,
    )
}

pub fn handle_packet(data: &[u8]) {
    // Basic Parsing
    if data.len() < 240 {
        return; // Too short
    }

    let op = data[0];
    if op != 2 {
        return;
    } // We only care about BOOTREPLY

    let xid_bytes: [u8; 4] = [data[4], data[5], data[6], data[7]];
    let xid = u32::from_be_bytes(xid_bytes);

    if unsafe { xid != CURRENT_XID } {
        crate::mesa_println!("  [DHCP] rx BOOTREPLY xid={:#x} (esperado {:#x})", xid, unsafe { CURRENT_XID });
        return;
    }

    // Parse Options
    // Offset 236 is start of flags/magic cookie.
    // Magic Cookie is at 236 (4 bytes). Options start at 240.

    if data[236] != 0x63 || data[237] != 0x82 || data[238] != 0x53 || data[239] != 0x63 {
        crate::serial_println!("[DHCP] Bad Magic Cookie");
        return;
    }

    let mut i = 240;
    let mut msg_type = 0;
    let mut server_id = [0u8; 4];
    let mut router = [0u8; 4];
    let mut subnet = [0u8; 4];
    let mut dns = [0u8; 4];

    let your_ip = [data[16], data[17], data[18], data[19]];
    let _server_ip = [data[20], data[21], data[22], data[23]]; // siaddr (might be zero)

    while i < data.len() {
        let opt = data[i];
        if opt == OPT_END {
            break;
        }
        if opt == 0 {
            i += 1;
            continue;
        }

        let len = data[i + 1] as usize;
        let val = &data[i + 2..i + 2 + len];

        match opt {
            OPT_MESSAGE_TYPE => msg_type = val[0],
            OPT_SERVER_ID => server_id.copy_from_slice(val),
            OPT_ROUTER => {
                if len >= 4 {
                    router.copy_from_slice(&val[0..4]);
                }
            }
            OPT_SUBNET_MASK => {
                if len >= 4 {
                    subnet.copy_from_slice(val);
                }
            }
            OPT_DNS => {
                if len >= 4 {
                    dns.copy_from_slice(&val[0..4]);
                }
            }
            _ => {}
        }

        i += 2 + len;
    }

    match msg_type {
        MSG_OFFER => {
            if dns != [0, 0, 0, 0] {
                unsafe {
                    OFFER_DNS = dns;
                }
            }
            crate::mesa_println!(
                "  [DHCP] OFFER {} de {}",
                format_ip(your_ip),
                format_ip(server_id)
            );
            unsafe {
                OFFERED_IP = your_ip;
                OFFERED_SERVER = server_id;
            }
            crate::net::set_pending_ip(your_ip);
            // Do NOT transmit from inside the RX path: the polling loop sends
            // the REQUEST (and retries it) so TX failures are visible.
            DHCP_PHASE.store(PHASE_REQUEST, Ordering::Relaxed);
            REQUEST_TRIES.store(0, Ordering::Relaxed);
        }
        MSG_ACK => {
            // Fall back to the resolver from the OFFER when the ACK omits it.
            let dns = if dns != [0, 0, 0, 0] {
                dns
            } else {
                unsafe { OFFER_DNS }
            };
            crate::mesa_println!("  [DHCP] ACK recibido");
            crate::mesa_println!("  [DHCP] IP: {}  Mask: {}  GW: {}  DNS: {}",
                format_ip(your_ip), format_ip(subnet), format_ip(router), format_ip(dns));

            // Store lease time for renewal (option 51, default 600s if not provided)
            let mut lease_secs: u32 = 600;
            let mut i = 240;
            while i + 1 < data.len() {
                let opt = data[i];
                if opt == OPT_END || opt == 0 {
                    break;
                }
                let len = data[i + 1] as usize;
                if opt == OPT_LEASE_TIME && len >= 4 && i + 2 + len <= data.len() {
                    lease_secs = u32::from_be_bytes([data[i+2], data[i+3], data[i+4], data[i+5]]);
                }
                i += 2 + len;
            }
            let now = crate::curr_arch::get_ticks();
            // Lease expires at lease_secs * 100 ticks (~100 Hz timer). Renew at 50% of lease.
            unsafe {
                LEASE_EXPIRY_TICKS = now.wrapping_add(lease_secs as u64 * 100 / 2);
            }
            crate::mesa_println!("  [DHCP] Lease: {}s, renewal at ~{}s", lease_secs, lease_secs / 2);

            DHCP_PHASE.store(PHASE_BOUND, Ordering::Relaxed);
            REQUEST_TRIES.store(0, Ordering::Relaxed);
            crate::net::clear_pending_ip();
            crate::net::configure(your_ip, subnet, router);
            crate::net::set_dns(dns);
        }
        MSG_NAK => {
            // A NAK used to be dropped silently, which looked exactly like a
            // lost ACK. Surface it so the real reason is visible on screen.
            crate::mesa_println!("  [DHCP] NAK de {} (no se concede {})",
                format_ip(server_id), format_ip(your_ip));
            crate::net::clear_pending_ip();
            DHCP_PHASE.store(PHASE_DISCOVER, Ordering::Relaxed);
        }
        _ => {
            crate::serial_println!("[DHCP] Ignored message type: {}", msg_type);
        }
    }
}

fn build_request_packet(requested_ip: [u8; 4], server_id: [u8; 4], broadcast_reply: bool) -> Vec<u8> {
    let mac = crate::net::get_mac();
    let mut packet = Vec::with_capacity(320);
    packet.push(1); // op
    packet.push(1); // htype
    packet.push(6); // hlen
    packet.push(0); // hops
    let xid = unsafe { CURRENT_XID };
    packet.extend_from_slice(&xid.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes()); // secs
    packet.extend_from_slice(
        &(if broadcast_reply { 0x8000u16 } else { 0x0000u16 }).to_be_bytes()
    ); // flags

    packet.extend_from_slice(&[0, 0, 0, 0]); // ciaddr
    packet.extend_from_slice(&[0, 0, 0, 0]); // yiaddr
    packet.extend_from_slice(&[0, 0, 0, 0]); // siaddr
    packet.extend_from_slice(&[0, 0, 0, 0]); // giaddr

    packet.extend_from_slice(&mac);
    packet.extend_from_slice(&[0u8; 10]);
    packet.extend_from_slice(&[0u8; 64]); // sname
    packet.extend_from_slice(&[0u8; 128]); // file

    packet.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());

    packet.push(OPT_MESSAGE_TYPE);
    packet.push(1);
    packet.push(MSG_REQUEST);

    packet.push(OPT_CLIENT_ID);
    packet.push(7);
    packet.push(0x01);
    packet.extend_from_slice(&mac);

    packet.push(OPT_REQUESTED_IP);
    packet.push(4);
    packet.extend_from_slice(&requested_ip);

    packet.push(OPT_SERVER_ID);
    packet.push(4);
    packet.extend_from_slice(&server_id);

    packet.push(OPT_MAX_MSG_SIZE);
    packet.push(2);
    packet.extend_from_slice(&576u16.to_be_bytes());

    packet.push(OPT_PARAM_REQUEST);
    packet.push(3);
    packet.push(OPT_SUBNET_MASK);
    packet.push(OPT_ROUTER);
    packet.push(OPT_DNS);

    packet.push(OPT_END);
    while packet.len() < 300 {
        packet.push(0);
    }
    packet
}

fn send_request(requested_ip: [u8; 4], server_id: [u8; 4]) {
    let n = REQUEST_TRIES.fetch_add(1, Ordering::Relaxed);
    let wifi = crate::drivers::net::rtl8822ce::is_ready();
    // Attempts 0-1 keep the exact previous behaviour (broadcast, hotspot-safe).
    // From attempt 2 on, ask for a UNICAST reply (flags=0) while still
    // broadcasting the REQUEST: the server answers to our chaddr with the
    // pairwise CCMP key instead of the TKIP group key, whose broadcast RX is
    // broken on this AP. From attempt 6, additionally address the frame to the
    // AP MAC in case the server ignores broadcast REQUESTs.
    let ask_unicast = n >= 2;
    let unicast_mac = n >= 6 && wifi;
    let packet = build_request_packet(requested_ip, server_id, !ask_unicast);
    let result = if unicast_mac {
        let bssid = crate::drivers::wifi::state::get_bssid();
        let udp = crate::net::udp::create_packet([0, 0, 0, 0], server_id, DHCP_CLIENT_PORT, DHCP_SERVER_PORT, &packet);
        let ip = crate::net::ipv4::create_packet([0, 0, 0, 0], server_id, 17, &udp);
        crate::net::send_ethernet(bssid, 0x0800, &ip)
    } else {
        crate::net::udp::send_packet(
            [255, 255, 255, 255],
            [0, 0, 0, 0],
            DHCP_CLIENT_PORT,
            DHCP_SERVER_PORT,
            &packet,
        )
    };
    match result {
        Ok(()) => crate::mesa_println!(
            "  [DHCP] REQUEST {} ({}{})",
            format_ip(requested_ip),
            if ask_unicast { "reply-unicast" } else { "reply-broadcast" },
            if unicast_mac { ", mac=AP" } else { "" }
        ),
        Err(e) => crate::mesa_println!("  [DHCP] error enviando REQUEST: {}", e),
    }
}

fn format_ip(ip: [u8; 4]) -> alloc::string::String {
    alloc::format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}

/// Returns true if the DHCP lease is about to expire and needs renewal.
/// Called from wifi_tick() to trigger RENEW before the lease lapses.
pub fn lease_renewal_needed() -> bool {
    let expiry = unsafe { LEASE_EXPIRY_TICKS };
    if expiry == 0 {
        return false;
    }
    let now = crate::curr_arch::get_ticks();
    now >= expiry
}

/// Send DHCP REQUEST (renew) to extend the lease.
/// Uses unicast to the server (ciaddr = our current IP, since we have one now).
pub fn send_renew() {
    let mac = crate::net::get_mac();
    let our_ip = crate::net::get_ip();
    let server_id = crate::net::get_gateway(); // Renew via gateway/server

    let mut packet = Vec::with_capacity(300);
    packet.push(1); // op: BOOTREQUEST
    packet.push(1); // htype: Ethernet
    packet.push(6); // hlen
    packet.push(0); // hops

    let xid = unsafe { CURRENT_XID };
    packet.extend_from_slice(&xid.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes()); // secs
    packet.extend_from_slice(&0x0000u16.to_be_bytes()); // flags (unicast)

    // ciaddr = our current IP (we have a lease, renewing it)
    packet.extend_from_slice(&our_ip);
    packet.extend_from_slice(&[0, 0, 0, 0]); // yiaddr
    packet.extend_from_slice(&[0, 0, 0, 0]); // siaddr
    packet.extend_from_slice(&[0, 0, 0, 0]); // giaddr

    packet.extend_from_slice(&mac);
    packet.extend_from_slice(&[0u8; 10]);
    packet.extend_from_slice(&[0u8; 64]);
    packet.extend_from_slice(&[0u8; 128]);

    packet.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());

    packet.push(OPT_MESSAGE_TYPE);
    packet.push(1);
    packet.push(MSG_REQUEST);

    packet.push(OPT_REQUESTED_IP);
    packet.push(4);
    packet.extend_from_slice(&our_ip);

    packet.push(OPT_SERVER_ID);
    packet.push(4);
    packet.extend_from_slice(&server_id);

    packet.push(OPT_END);

    crate::mesa_println!("  [DHCP] Enviando RENEW for {}...", format_ip(our_ip));
    // RENEW is unicast to the server, sourced from our IP (not 0.0.0.0)
    let _ = crate::net::udp::send_packet(
        server_id,
        our_ip,
        DHCP_CLIENT_PORT,
        DHCP_SERVER_PORT,
        &packet,
    );
}
