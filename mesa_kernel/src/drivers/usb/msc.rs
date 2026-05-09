// mesa_kernel/src/drivers/usb/msc.rs
use crate::drivers::usb::xhci::XhciDriver;
use crate::drivers::usb::descriptors::{InterfaceDescriptor, EndpointDescriptor};
use alloc::vec::Vec;
use spin::Mutex;

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct CommandBlockWrapper {
    signature: u32,
    tag: u32,
    transfer_length: u32,
    flags: u8,
    lun: u8,
    cb_length: u8,
    cb: [u8; 16],
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct CommandStatusWrapper {
    signature: u32,
    tag: u32,
    residue: u32,
    status: u8,
}

pub struct MscDevice {
    pub slot: u8,
    pub xhci_idx: usize,
    pub bulk_in_dci: u8,
    pub bulk_out_dci: u8,
    pub max_packet_size: u16,
    pub capacity_blocks: u64,
    pub block_size: u32,
}

impl MscDevice {
    pub fn new(xhci_idx: usize, slot: u8, bulk_in: u8, bulk_out: u8, mps: u16) -> Self {
        Self {
            slot,
            xhci_idx,
            bulk_in_dci: bulk_in,
            bulk_out_dci: bulk_out,
            max_packet_size: mps,
            capacity_blocks: 0,
            block_size: 0,
        }
    }

    fn send_cbw(&self, tag: u32, len: u32, flags: u8, cb: &[u8], explicit_ctrl: Option<&mut XhciDriver>) -> bool {
        let mut cbw = CommandBlockWrapper {
            signature: 0x43425355, // "USBC"
            tag,
            transfer_length: len,
            flags,
            lun: 0,
            cb_length: cb.len() as u8,
            cb: [0; 16],
        };
        cbw.cb[..cb.len()].copy_from_slice(cb);

        if let Some(ctrl) = explicit_ctrl {
            ctrl.bulk_transfer(self.slot, self.bulk_out_dci, &mut cbw as *mut _ as *mut u8, 31, false)
        } else {
            let mut controllers = crate::drivers::usb::XHCI_CONTROLLERS.lock();
            if let Some(ctrl) = controllers.get_mut(self.xhci_idx) {
                ctrl.bulk_transfer(self.slot, self.bulk_out_dci, &mut cbw as *mut _ as *mut u8, 31, false)
            } else {
                false
            }
        }
    }

    fn receive_csw(&self, tag: u32, explicit_ctrl: Option<&mut XhciDriver>) -> Option<CommandStatusWrapper> {
        let mut csw = CommandStatusWrapper {
            signature: 0,
            tag: 0,
            residue: 0,
            status: 0,
        };

        let success = if let Some(ctrl) = explicit_ctrl {
            ctrl.bulk_transfer(self.slot, self.bulk_in_dci, &mut csw as *mut _ as *mut u8, 13, true)
        } else {
            let mut controllers = crate::drivers::usb::XHCI_CONTROLLERS.lock();
            if let Some(ctrl) = controllers.get_mut(self.xhci_idx) {
                ctrl.bulk_transfer(self.slot, self.bulk_in_dci, &mut csw as *mut _ as *mut u8, 13, true)
            } else {
                false
            }
        };

        if success && csw.signature == 0x53425355 && csw.tag == tag {
            Some(csw)
        } else {
            None
        }
    }

    pub fn init(&mut self, ctrl: &mut XhciDriver) -> Result<(), &'static str> {
        // 0. Test Unit Ready (wait up to 5 seconds)
        crate::serial_println!("[MSC] Waiting for device ready...");
        let mut ready = false;
        for _ in 0..100 {
            if self.scsi_command(0x00, 0, &mut [], Some(&mut *ctrl)) {
                ready = true;
                break;
            }
            crate::drivers::usb::xhci::delay_ms(50);
        }
        if !ready {
            crate::serial_println!("[MSC] Device not ready after timeout");
            // Continuar de todos modos, algunos dispositivos son raros
        }

        // 1. Inquiry
        let mut inquiry_data = [0u8; 36];
        if !self.scsi_command(0x12, 36, &mut inquiry_data, Some(&mut *ctrl)) {
            return Err("SCSI Inquiry failed");
        }
        crate::serial_println!("[MSC] Inquiry OK: {:x?}", &inquiry_data[8..36]);

        // 2. Read Capacity
        let mut cap_data = [0u8; 8];
        if !self.scsi_command(0x25, 8, &mut cap_data, Some(&mut *ctrl)) {
            return Err("SCSI Read Capacity failed");
        }
        
        let last_lba = u32::from_be_bytes([cap_data[0], cap_data[1], cap_data[2], cap_data[3]]);
        let block_len = u32::from_be_bytes([cap_data[4], cap_data[5], cap_data[6], cap_data[7]]);
        
        self.capacity_blocks = (last_lba as u64) + 1;
        self.block_size = block_len;

        crate::serial_println!("[MSC] Capacity: {} blocks of {} bytes ({} MB)", 
            self.capacity_blocks, self.block_size, (self.capacity_blocks * self.block_size as u64) / 1024 / 1024);

        Ok(())
    }

    pub fn read_blocks(&self, _lba: u64, _count: u16, _buffer: &mut [u8]) -> bool {
        false
    }

    pub fn write_blocks(&self, _lba: u64, _count: u16, _buffer: &[u8]) -> bool {
        false
    }

    fn scsi_command(&self, cmd: u8, len: u32, data: &mut [u8], mut explicit_ctrl: Option<&mut XhciDriver>) -> bool {
        let tag = 0x99999999;
        let mut cb = [0u8; 16];
        cb[0] = cmd;
        
        let cb_len = if cmd == 0x12 {
            cb[4] = len as u8; // Inquiry allocation length
            6
        } else if cmd == 0x25 {
            10 // Read Capacity (10)
        } else {
            6
        };

        crate::serial_println!("[MSC] Sending CBW for cmd {:x}", cmd);
        if !self.send_cbw(tag, len, 0x80, &cb[..cb_len], explicit_ctrl.as_deref_mut()) {
            crate::serial_println!("[MSC] send_cbw failed");
            return false;
        }

        if len > 0 {
            crate::serial_println!("[MSC] Sending Data Phase for cmd {:x}", cmd);
            let success = if let Some(ctrl) = explicit_ctrl.as_deref_mut() {
                ctrl.bulk_transfer(self.slot, self.bulk_in_dci, data.as_mut_ptr(), len as usize, true)
            } else {
                let mut controllers = crate::drivers::usb::XHCI_CONTROLLERS.lock();
                if let Some(ctrl) = controllers.get_mut(self.xhci_idx) {
                    ctrl.bulk_transfer(self.slot, self.bulk_in_dci, data.as_mut_ptr(), len as usize, true)
                } else {
                    false
                }
            };
            if !success {
                crate::serial_println!("[MSC] bulk_transfer for Data Phase failed");
                return false;
            }
        }

        crate::serial_println!("[MSC] Receiving CSW for cmd {:x}", cmd);
        if let Some(csw) = self.receive_csw(tag, explicit_ctrl.as_deref_mut()) {
            if csw.status == 0 {
                return true;
            } else {
                crate::serial_println!("[MSC] CSW status non-zero: {}", csw.status);
                return false;
            }
        }
        crate::serial_println!("[MSC] receive_csw failed");
        false
    }
}

impl crate::drivers::block::BlockDevice for MscDevice {
    fn read(&self, lba: u64, count: u16, buffer: &mut [u8]) -> Result<(), &'static str> {
        if self.read_blocks(lba, count, buffer) {
            Ok(())
        } else {
            Err("MSC read failed")
        }
    }
    fn write(&self, lba: u64, count: u16, buffer: &[u8]) -> Result<(), &'static str> {
        if self.write_blocks(lba, count, buffer) {
            Ok(())
        } else {
            Err("MSC write failed")
        }
    }
    fn capacity(&self) -> u64 {
        self.capacity_blocks
    }
}

pub static MSC_DEVICES: Mutex<Vec<alloc::sync::Arc<MscDevice>>> = Mutex::new(Vec::new());

pub fn register(dev: MscDevice) {
    MSC_DEVICES.lock().push(alloc::sync::Arc::new(dev));
}
