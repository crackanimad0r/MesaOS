use alloc::vec::Vec;

pub struct EthernetFrame {
    pub src_mac: [u8; 6],
    pub dest_mac: [u8; 6],
    pub ethertype: u16,
    pub payload: Vec<u8>,
}

pub fn parse_frame(data: &[u8]) -> Option<EthernetFrame> {
    if data.len() < 14 {
        return None;
    }
    
    let mut dest_mac = [0u8; 6];
    let mut src_mac = [0u8; 6];
    dest_mac.copy_from_slice(&data[0..6]);
    src_mac.copy_from_slice(&data[6..12]);
    
    let ethertype = u16::from_be_bytes([data[12], data[13]]);
    let payload = data[14..].to_vec();
    
    Some(EthernetFrame {
        src_mac,
        dest_mac,
        ethertype,
        payload,
    })
}

pub fn create_frame(src_mac: [u8; 6], dest_mac: [u8; 6], ethertype: u16, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(14 + payload.len());
    
    frame.extend_from_slice(&dest_mac);
    frame.extend_from_slice(&src_mac);
    frame.extend_from_slice(&ethertype.to_be_bytes());
    frame.extend_from_slice(payload);
    
    // Pad to minimum Ethernet frame size (60 bytes without CRC)
    while frame.len() < 60 {
        frame.push(0);
    }
    
    frame
}
