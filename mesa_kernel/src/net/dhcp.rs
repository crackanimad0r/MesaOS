use alloc::vec::Vec;


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
pub const OPT_END: u8 = 255;

pub const MSG_DISCOVER: u8 = 1;
pub const MSG_OFFER: u8 = 2;
pub const MSG_REQUEST: u8 = 3;
pub const MSG_ACK: u8 = 5;

// Global transaction ID to match requests/responses
static mut CURRENT_XID: u32 = 0x12345678;

pub fn send_discover() -> Result<(), &'static str> {
    let mac = crate::drivers::net::rtl8139::get_mac().ok_or("No MAC address")?;
    
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
    packet.extend_from_slice(&0x0000u16.to_be_bytes()); // flags (Unicast/Broadcast)
    
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
    
    // Option 55: Parameter Request List
    packet.push(OPT_PARAM_REQUEST);
    packet.push(3); // Length
    packet.push(OPT_SUBNET_MASK);
    packet.push(OPT_ROUTER);
    packet.push(OPT_DNS);
    
    packet.push(OPT_END);
    
    crate::serial_println!("[DHCP] Sending DISCOVER (xid={:#x})...", xid);
    crate::net::udp::send_packet([255, 255, 255, 255], DHCP_CLIENT_PORT, DHCP_SERVER_PORT, &packet)
}

pub fn handle_packet(data: &[u8]) {
    // Basic Parsing
    if data.len() < 240 {
        return; // Too short
    }
    
    let op = data[0];
    if op != 2 { return; } // We only care about BOOTREPLY
    
    let xid_bytes: [u8; 4] = [data[4], data[5], data[6], data[7]];
    let xid = u32::from_be_bytes(xid_bytes);
    
    if unsafe { xid != CURRENT_XID } {
        // crate::serial_println!("[DHCP] Ignoring packet with xid {:#x} (expected {:#x})", xid, CURRENT_XID);
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
        if opt == OPT_END { break; }
        if opt == 0 { i += 1; continue; }
        
        let len = data[i+1] as usize;
        let val = &data[i+2 .. i+2+len];
        
        match opt {
            OPT_MESSAGE_TYPE => msg_type = val[0],
            OPT_SERVER_ID => server_id.copy_from_slice(val),
            OPT_ROUTER => if len >= 4 { router.copy_from_slice(&val[0..4]); },
            OPT_SUBNET_MASK => if len >= 4 { subnet.copy_from_slice(val); },
            OPT_DNS => if len >= 4 { dns.copy_from_slice(&val[0..4]); },
            _ => {}
        }
        
        i += 2 + len;
    }
    
    match msg_type {
        MSG_OFFER => {
            crate::serial_println!("[DHCP] Received OFFER: IP {} from Server {}", 
                format_ip(your_ip), format_ip(server_id));
            send_request(your_ip, server_id);
        }
        MSG_ACK => {
            crate::serial_println!("[DHCP] Received ACK. Configuration Complete!");
            crate::serial_println!("[DHCP] IP: {}", format_ip(your_ip));
            crate::serial_println!("[DHCP] Mask: {}", format_ip(subnet));
            crate::serial_println!("[DHCP] GW: {}", format_ip(router));
            
            crate::net::configure(your_ip, subnet, router);
        }
        _ => {
            // crate::serial_println!("[DHCP] Ignored message type: {}", msg_type);
        }
    }
}

fn send_request(requested_ip: [u8; 4], server_id: [u8; 4]) {
    let mac = crate::drivers::net::rtl8139::get_mac().unwrap();
    
    let mut packet = Vec::with_capacity(300);
    // BOOTP Header (Same as Discover usually)
    packet.push(1); // op
    packet.push(1); // htype
    packet.push(6); // hlen
    packet.push(0); // hops
    let xid = unsafe { CURRENT_XID };
    packet.extend_from_slice(&xid.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes()); // secs
    packet.extend_from_slice(&0x0000u16.to_be_bytes()); // flags
    
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
    
    packet.push(OPT_REQUESTED_IP);
    packet.push(4);
    packet.extend_from_slice(&requested_ip);
    
    packet.push(OPT_SERVER_ID);
    packet.push(4);
    packet.extend_from_slice(&server_id);
    
    packet.push(OPT_END);
    
    crate::serial_println!("[DHCP] Sending REQUEST for {}...", format_ip(requested_ip));
    let _ = crate::net::udp::send_packet([255, 255, 255, 255], DHCP_CLIENT_PORT, DHCP_SERVER_PORT, &packet);
}

fn format_ip(ip: [u8; 4]) -> alloc::string::String {
    alloc::format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}
