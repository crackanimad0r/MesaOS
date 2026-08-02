use alloc::vec::Vec;
use spin::Mutex;

pub const AF_INET: i32 = 2;
pub const SOCK_DGRAM: i32 = 2;
pub const SOCK_STREAM: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynRcvd,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    LastAck,
    TimeWait,
}

/// A TCP socket
pub struct TcpSocket {
    pub local_port: u16,
    pub remote_ip: u32,
    pub remote_port: u16,
    pub state: TcpState,
    pub seq_num: u32,
    pub ack_num: u32,
    pub rx_buffer: Vec<u8>,
    pub bound: bool,
    pub backlog: Vec<(u32, u16, u32, u32)>, // (src_ip, src_port, seq, ack)
}

/// A UDP socket visible to Linux userspace
pub struct UdpSocket {
    pub local_port: u16,
    pub bound: bool,
    pub rx_buffer: Vec<(u32, u16, Vec<u8>)>,
}

/// Linux socket table: FDs start at 10000
pub static SOCKET_TABLE: Mutex<Vec<Option<Socket>>> = Mutex::new(Vec::new());

pub enum Socket {
    Udp(UdpSocket),
    Tcp(TcpSocket),
}

impl Socket {
    pub fn new_udp() -> Self {
        Socket::Udp(UdpSocket {
            local_port: 0,
            bound: false,
            rx_buffer: Vec::new(),
        })
    }

    pub fn new_tcp() -> Self {
        Socket::Tcp(TcpSocket {
            local_port: 0,
            remote_ip: 0,
            remote_port: 0,
            state: TcpState::Closed,
            seq_num: 1000,
            ack_num: 0,
            rx_buffer: Vec::new(),
            bound: false,
            backlog: Vec::new(),
        })
    }
}

pub fn socket_create(domain: i32, type_: i32, _protocol: i32) -> i64 {
    if domain != AF_INET {
        return -crate::linux_compat::errno::EAFNOSUPPORT;
    }
    let sock = match type_ {
        SOCK_DGRAM => Socket::new_udp(),
        SOCK_STREAM => Socket::new_tcp(),
        _ => return -crate::linux_compat::errno::EPROTONOSUPPORT,
    };
    let mut table = SOCKET_TABLE.lock();
    let fd = 10000 + table.len();
    table.push(Some(sock));
    fd as i64
}

pub fn socket_connect(fd: i32, remote_ip: u32, remote_port: u16) -> i64 {
    let local_port;
    let seq;
    let ip_bytes = remote_ip.to_be_bytes();
    {
        let mut table = SOCKET_TABLE.lock();
        let idx = (fd as usize).wrapping_sub(10000);
        if idx >= table.len() {
            return -crate::linux_compat::errno::EBADF;
        }
        match &mut table[idx] {
            Some(Socket::Tcp(tcp)) => {
                if tcp.state != TcpState::Closed {
                    return -crate::linux_compat::errno::EISCONN;
                }
                local_port = (0xC000 + (fd as u16)).wrapping_mul(2);
                tcp.local_port = local_port;
                tcp.remote_ip = remote_ip;
                tcp.remote_port = remote_port;
                tcp.state = TcpState::SynSent;
                seq = tcp.seq_num;
                tcp.ack_num = 0;
            }
            _ => return -crate::linux_compat::errno::EOPNOTSUPP,
        }
    }
    // Send SYN
    let syn_hdr = crate::net::tcp::TcpHeader::new(
        local_port,
        remote_port,
        seq,
        0,
        crate::net::tcp::TCP_FLAG_SYN,
    );
    let _ = crate::net::tcp::send_tcp_packet(ip_bytes, syn_hdr, &[]);
    // Wait for SYN-ACK
    for _ in 0..1000 {
        crate::net::poll();
        let ack_num = unsafe { crate::net::tcp::check_synack(local_port, remote_port, ip_bytes) };
        if let Some(ack) = ack_num {
            let mut table = SOCKET_TABLE.lock();
            let idx = (fd as usize).wrapping_sub(10000);
            if let Some(Some(Socket::Tcp(ref mut tcp))) = table.get_mut(idx) {
                tcp.ack_num = ack;
                tcp.seq_num = tcp.seq_num.wrapping_add(1);
                tcp.state = TcpState::Established;
            }
            // Send ACK
            let ack_hdr = crate::net::tcp::TcpHeader::new(
                local_port,
                remote_port,
                seq.wrapping_add(1),
                ack,
                crate::net::tcp::TCP_FLAG_ACK,
            );
            let _ = crate::net::tcp::send_tcp_packet(ip_bytes, ack_hdr, &[]);
            return 0;
        }
        crate::scheduler::yield_now();
    }
    -crate::linux_compat::errno::ETIMEDOUT
}

pub fn socket_bind(fd: i32, port: u16) -> i64 {
    let port_in_use = {
        let table = SOCKET_TABLE.lock();
        table.iter().any(|entry| match entry {
            Some(Socket::Udp(udp)) => udp.bound && udp.local_port == port,
            Some(Socket::Tcp(tcp)) => tcp.bound && tcp.local_port == port,
            _ => false,
        })
    };
    if port_in_use {
        return -crate::linux_compat::errno::EADDRINUSE;
    }

    let mut table = SOCKET_TABLE.lock();
    let idx = (fd as usize).wrapping_sub(10000);
    if idx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    match table[idx].as_mut() {
        Some(Socket::Udp(ref mut udp)) => {
            udp.local_port = port;
            udp.bound = true;
            0
        }
        Some(Socket::Tcp(ref mut tcp)) => {
            tcp.local_port = port;
            tcp.bound = true;
            0
        }
        _ => -crate::linux_compat::errno::EBADF,
    }
}

pub fn socket_listen(fd: i32, _backlog: i32) -> i64 {
    let mut table = SOCKET_TABLE.lock();
    let idx = (fd as usize).wrapping_sub(10000);
    if idx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    match &mut table[idx] {
        Some(Socket::Tcp(tcp)) => {
            if !tcp.bound {
                return -crate::linux_compat::errno::EINVAL;
            }
            tcp.state = TcpState::Listen;
            0
        }
        _ => -crate::linux_compat::errno::EOPNOTSUPP,
    }
}

pub fn socket_accept(fd: i32, addr_ptr: u64, _addrlen_ptr: u64) -> i64 {
    let local_port;
    let src_ip;
    let src_port;
    let remote_seq;
    // Wait for incoming SYN
    loop {
        crate::net::poll();
        let pending = {
            let mut table = SOCKET_TABLE.lock();
            let idx = (fd as usize).wrapping_sub(10000);
            if idx >= table.len() {
                return -crate::linux_compat::errno::EBADF;
            }
            match &mut table[idx] {
                Some(Socket::Tcp(tcp)) => {
                    if tcp.state != TcpState::Listen {
                        return -crate::linux_compat::errno::EINVAL;
                    }
                    // check for incoming SYN
                    let syn = unsafe { crate::net::tcp::check_syn(tcp.local_port) };
                    if let Some((hdr, _payload, src)) = syn {
                        let src_ip_u32 = u32::from_be_bytes(src);
                        tcp.backlog
                            .push((src_ip_u32, hdr.src_port, hdr.seq_num, tcp.seq_num));
                        Some((
                            tcp.local_port,
                            src_ip_u32,
                            hdr.src_port,
                            hdr.seq_num,
                            tcp.seq_num,
                            src,
                        ))
                    } else {
                        None
                    }
                }
                _ => return -crate::linux_compat::errno::EOPNOTSUPP,
            }
        };
        if let Some((lp, sip, sp, rs, as_seq, src_ip_bytes)) = pending {
            local_port = lp;
            src_ip = sip;
            src_port = sp;
            remote_seq = rs;
            // Create child socket
            let child_fd = 10000 + {
                let t = SOCKET_TABLE.lock();
                t.len()
            };
            let child_seq = as_seq.wrapping_add(1);
            {
                let mut table = SOCKET_TABLE.lock();
                table.push(Some(Socket::Tcp(TcpSocket {
                    local_port,
                    remote_ip: src_ip,
                    remote_port: src_port,
                    state: TcpState::SynRcvd,
                    seq_num: child_seq,
                    ack_num: remote_seq.wrapping_add(1),
                    rx_buffer: Vec::new(),
                    bound: true,
                    backlog: Vec::new(),
                })));
            }
            // Send SYN-ACK
            let synack_hdr = crate::net::tcp::TcpHeader::new(
                local_port,
                src_port,
                child_seq,
                remote_seq.wrapping_add(1),
                crate::net::tcp::TCP_FLAG_SYN | crate::net::tcp::TCP_FLAG_ACK,
            );
            let _ = crate::net::tcp::send_tcp_packet(src_ip_bytes, synack_hdr, &[]);
            // Wait for final ACK
            let child_idx = (child_fd - 10000) as usize;
            for _ in 0..500 {
                crate::net::poll();
                if unsafe { crate::net::tcp::check_ack(local_port, src_port, src_ip_bytes) } {
                    let mut table = SOCKET_TABLE.lock();
                    if let Some(Some(Socket::Tcp(ref mut ctcp))) = table.get_mut(child_idx) {
                        ctcp.state = TcpState::Established;
                    }
                    break;
                }
                crate::scheduler::yield_now();
            }
            // write peer address if requested
            if addr_ptr != 0 && crate::security::validate_user_ptr(addr_ptr) {
                unsafe {
                    core::ptr::write_volatile(addr_ptr as *mut u16, 2u16);
                    core::ptr::write_volatile((addr_ptr + 2) as *mut u16, src_port.to_be());
                    core::ptr::write_volatile((addr_ptr + 4) as *mut u32, src_ip);
                }
            }
            return child_fd as i64;
        }
        drop(pending);
        crate::scheduler::yield_now();
    }
}

pub fn socket_sendto(fd: i32, data: &[u8], dest_ip: u32, dest_port: u16) -> i64 {
    let local_port;
    {
        let table = SOCKET_TABLE.lock();
        let idx = (fd as usize).wrapping_sub(10000);
        if idx >= table.len() {
            return -crate::linux_compat::errno::EBADF;
        }
        match &table[idx] {
            Some(Socket::Udp(udp)) => {
                if !udp.bound || udp.local_port == 0 {
                    return -crate::linux_compat::errno::EINVAL;
                }
                local_port = udp.local_port;
            }
            _ => return -crate::linux_compat::errno::EBADF,
        }
    }
    let ip = dest_ip.to_be_bytes();
    match crate::net::udp::send_packet(ip, local_port, dest_port, data) {
        Ok(_) => data.len() as i64,
        Err(_) => -crate::linux_compat::errno::EIO,
    }
}

pub fn socket_recvfrom(fd: i32, buf: &mut [u8]) -> i64 {
    crate::net::poll();

    let mut table = SOCKET_TABLE.lock();
    let idx = (fd as usize).wrapping_sub(10000);
    if idx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    match &mut table[idx] {
        Some(Socket::Udp(udp)) => {
            if udp.rx_buffer.is_empty() {
                return -crate::linux_compat::errno::EAGAIN;
            }
            let (_src_ip, _src_port, data) = udp.rx_buffer.remove(0);
            let len = data.len().min(buf.len());
            buf[..len].copy_from_slice(&data[..len]);
            len as i64
        }
        Some(Socket::Tcp(tcp)) => {
            if tcp.rx_buffer.is_empty() {
                return -crate::linux_compat::errno::EAGAIN;
            }
            let len = tcp.rx_buffer.len().min(buf.len());
            buf[..len].copy_from_slice(&tcp.rx_buffer[..len]);
            tcp.rx_buffer.drain(..len);
            len as i64
        }
        _ => -crate::linux_compat::errno::EBADF,
    }
}

pub fn socket_close(fd: i32) -> i64 {
    let mut table = SOCKET_TABLE.lock();
    let idx = (fd as usize).wrapping_sub(10000);
    if idx >= table.len() {
        return -crate::linux_compat::errno::EBADF;
    }
    table[idx] = None;
    0
}

pub fn udp_deliver(src_port: u16, dest_port: u16, src_ip: [u8; 4], data: &[u8]) -> bool {
    let src_ip_u32 = u32::from_be_bytes(src_ip);
    let mut table = SOCKET_TABLE.lock();
    for entry in table.iter_mut() {
        if let Some(Socket::Udp(udp)) = entry {
            if udp.bound && udp.local_port == dest_port {
                if udp.rx_buffer.len() < 64 {
                    udp.rx_buffer.push((src_ip_u32, src_port, data.to_vec()));
                }
                return true;
            }
        }
    }
    false
}

/// Deliver incoming TCP data to an established socket
pub fn tcp_deliver(dest_port: u16, src_port: u16, src_ip: [u8; 4], data: &[u8]) -> bool {
    let src_ip_u32 = u32::from_be_bytes(src_ip);
    let mut table = SOCKET_TABLE.lock();
    for entry in table.iter_mut() {
        if let Some(Socket::Tcp(tcp)) = entry {
            if tcp.state == TcpState::Established
                && tcp.local_port == dest_port
                && tcp.remote_port == src_port
                && tcp.remote_ip == src_ip_u32
            {
                if tcp.rx_buffer.len() < 65536 {
                    tcp.rx_buffer.extend_from_slice(data);
                }
                return true;
            }
        }
    }
    false
}

pub fn socket_check_ready(fd: i32) -> i32 {
    let table = SOCKET_TABLE.lock();
    let idx = (fd as usize).wrapping_sub(10000);
    if let Some(Some(sock)) = table.get(idx) {
        let mut mask = 2i32;
        match sock {
            Socket::Udp(udp) => {
                if !udp.rx_buffer.is_empty() {
                    mask |= 1;
                }
            }
            Socket::Tcp(tcp) => {
                if tcp.state == TcpState::Listen {
                    return 3;
                }
                if tcp.state == TcpState::Established {
                    if !tcp.rx_buffer.is_empty() {
                        mask |= 1;
                    }
                }
            }
        }
        mask
    } else {
        0
    }
}
