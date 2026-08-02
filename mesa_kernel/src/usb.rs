// mesa_kernel/src/usb.rs - USB subsystem: device discovery + lsusb
//
// Traduce la informacion del shim a nombres legibles:
//   - VID:PID
//   - speed string
//   - manufacturer/product strings (si estan disponibles)
//   - USB class
//
// License: MIT

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

static USB_DEVICES: Mutex<Vec<UsbDevice>> = Mutex::new(Vec::new());

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub bcd_usb: u16,
    pub b_device_class: u8,
    pub b_device_subclass: u8,
    pub b_device_protocol: u8,
    pub b_max_packet_size0: u8,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u16,
    pub i_manufacturer: u8,
    pub i_product: u8,
    pub i_serial_number: u8,
    pub b_num_configurations: u8,
}

#[derive(Debug, Clone)]
pub struct UsbDevice {
    pub slot_id: u8,
    pub port: u8,
    pub speed_id: u8,
    pub speed_str: &'static str,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_class: u8,
    pub class_str: &'static str,
    pub manufacturer: String,
    pub product: String,
    pub serial: String,
    pub configured: bool,
}

impl UsbDevice {
    pub fn from_slice(slot_id: u8, data: &[u8]) -> Option<Self> {
        if data.len() < core::mem::size_of::<UsbDeviceDescriptor>() {
            return None;
        }
        let desc = unsafe { &*(data.as_ptr() as *const UsbDeviceDescriptor) };

        let (speed_str, class_str) = match desc.b_device_class {
            0x01 => ("?", "Audio"),
            0x02 => ("?", "Communications"),
            0x03 => ("?", "HID"),
            0x05 => ("?", "Physical"),
            0x06 => ("?", "Image"),
            0x07 => ("?", "Printer"),
            0x08 => ("?", "Mass Storage"),
            0x09 => ("?", "Hub"),
            0x0A => ("?", "CDC-Data"),
            0x0B => ("?", "SmartCard"),
            0x0E => ("?", "Video"),
            0xEF => ("?", "Vendor-specific"),
            0xFE => ("?", "Wireless"),
            0xFF => ("?", "Vendor-specific (FF)"),
            _ => ("?", "Per-Interface"),
        };

        let mut manufacturer = String::new();
        let mut product = String::new();
        let mut serial = String::new();

        /* Intentar parsear string descriptors despues del device descriptor */
        let mut offset = desc.b_length as usize;
        while offset + 2 <= data.len() {
            let str_len = data[offset] as usize;
            if str_len == 0 || offset + str_len > data.len() {
                break;
            }
            let str_type = data[offset + 1];
            let raw = &data[offset + 2..offset + str_len];
            let s = parse_utf16le_safe(raw);
            match str_type {
                0x02 if desc.i_manufacturer != 0 && manufacturer.is_empty() => {
                    manufacturer = s;
                }
                0x01 if desc.i_product != 0 && product.is_empty() => {
                    product = s;
                }
                0x03 if desc.i_serial_number != 0 && serial.is_empty() => {
                    serial = s;
                }
                _ => {}
            }
            offset += str_len;
        }

        let speed_str = match desc.b_device_class {
            _ => "?",
        };

        Some(UsbDevice {
            slot_id,
            port: 0,
            speed_id: 0,
            speed_str: "?",
            vendor_id: desc.id_vendor,
            product_id: desc.id_product,
            device_class: desc.b_device_class,
            class_str,
            manufacturer,
            product,
            serial,
            configured: false,
        })
    }
}

fn parse_utf16le_safe(data: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i + 1 < data.len() {
        let code_unit = u16::from_le_bytes([data[i], data[i + 1]]);
        let ch = if code_unit >= 0x20 && code_unit <= 0x7E {
            core::char::from_u32(code_unit as u32).unwrap_or('?')
        } else {
            '?'
        };
        out.push(ch);
        i += 2;
    }
    out
}

pub fn refresh_devices() {
    /* En esta iteracion, la deteccion real se hace desde el shim.
    Aqui solo dejamos la estructura lista para popularse */
    let mut list = USB_DEVICES.lock();
    list.clear();
}

pub fn list_devices() -> Vec<UsbDevice> {
    USB_DEVICES.lock().clone()
}

pub fn add_device(dev: UsbDevice) {
    USB_DEVICES.lock().push(dev);
}
