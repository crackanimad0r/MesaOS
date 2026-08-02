// src/drivers/usb/linux_glue.rs
use crate::linux::*;

#[repr(C)]
pub struct usb_device {
    pub slot_id: u8,
    pub speed: u8,
    pub bus: *mut core::ffi::c_void,
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

#[no_mangle]
pub unsafe extern "C" fn usb_alloc_urb(_iso_packets: i32, _mem_flags: u32) -> *mut urb {
    let size = core::mem::size_of::<urb>();
    kzalloc(size, _mem_flags) as *mut urb
}

#[no_mangle]
pub unsafe extern "C" fn usb_control_msg(
    _dev: *mut usb_device,
    _pipe: u32,
    _request: u8,
    _requesttype: u8,
    _value: u16,
    _index: u16,
    _data: *mut u8,
    _size: u16,
    _timeout: i32,
) -> i32 {
    // TODO: Implement using Linux shim USB core or let the Linux driver handle it.
    -110 // -ETIMEDOUT
}
