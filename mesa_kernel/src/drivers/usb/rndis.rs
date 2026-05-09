// mesa_kernel/src/drivers/usb/rndis.rs
use crate::drivers::usb::xhci::XhciDriver as XhciController;
use crate::memory::pmm;

#[repr(C, packed)]
struct RndisInitMsg {
    msg_type: u32, // 1 = Init
    msg_len: u32,
    request_id: u32,
    major_ver: u32,
    minor_ver: u32,
    max_transfer_size: u32,
}

pub fn init(ctrl: &mut XhciController, slot: u8) -> bool {
    crate::serial_println!("[RNDIS] Initializing Slot {}...", slot);
    
    // 1. Send REMOTE_NDIS_INITIALIZE_MSG via SEND_ENCAPSULATED_COMMAND (0x00)
    let msg_phys = pmm::alloc_frame().expect("RNDIS alloc");
    let msg = crate::memory::vmm::phys_to_virt(msg_phys) as *mut RndisInitMsg;
    unsafe {
        (*msg).msg_type = 1;
        (*msg).msg_len = 24;
        (*msg).request_id = 1;
        (*msg).major_ver = 1;
        (*msg).minor_ver = 0;
        (*msg).max_transfer_size = 16384;
    }

    // Control Transfer: Class-Specific Request 0x00 (SEND_ENCAPSULATED_COMMAND)
    // BMRequestType=0x21 (Host to Interface), Request=0x00, Value=0, Index=0
    if !ctrl.control_transfer(slot, 0x21, 0x00, 0, 0, msg as *mut u8, 24) {
        crate::serial_println!("[RNDIS] Failed to send INIT message");
        return false;
    }
    
    crate::serial_println!("[RNDIS] INIT message sent successfully");
    true
}


pub fn usb_net_tx(_data: &[u8]) -> Result<(), &'static str> {
    // Placeholder for now
    Err("RNDIS TX not yet implemented")
}

pub fn usb_net_poll() -> Option<alloc::vec::Vec<u8>> {
    // Placeholder for now
    None
}

