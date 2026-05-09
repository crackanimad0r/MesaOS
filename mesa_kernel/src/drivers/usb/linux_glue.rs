// src/drivers/usb/linux_glue.rs
use crate::linux::*;
use crate::drivers::usb::xhci::XhciDriver as XhciController;

#[repr(C)]
pub struct usb_device {
    pub slot_id: u8,
    pub speed: u8,
    pub bus: *mut XhciController,
    pub children: list_head,
    // ...
}

#[repr(C)]
pub struct urb {
    pub dev: *mut usb_device,
    pub pipe: u32,
    pub status: i32,
    pub transfer_buffer: *mut u8,
    pub transfer_buffer_length: u32,
    pub actual_length: u32,
    pub setup_packet: *mut u8,
    pub complete: Option<extern "C" fn(*mut urb)>,
    pub context: *mut core::ffi::c_void,
}

impl urb {
    pub fn new() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

pub unsafe fn usb_alloc_urb(_iso_packets: i32, _mem_flags: u32) -> *mut urb {
    let size = core::mem::size_of::<urb>();
    kzalloc(size, _mem_flags) as *mut urb
}

pub unsafe fn usb_control_msg(
    dev: *mut usb_device,
    pipe: u32,
    request: u8,
    requesttype: u8,
    value: u16,
    index: u16,
    data: *mut u8,
    size: u16,
    _timeout: i32
) -> i32 {
    let xhci = &mut *(*dev).bus;
    let success = xhci.control_transfer((*dev).slot_id, requesttype, request, value, index, data, size as usize);
    if success { size as i32 } else { -110 } // -ETIMEDOUT
}

