use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{compiler_fence, Ordering};
use spin::Mutex;
use crate::drivers::block::{BlockDevice, SECTOR_SIZE};
use crate::memory::{pmm, vmm};
use crate::pci;

const NVME_CLASS: u8 = 0x01;
const NVME_SUBCLASS: u8 = 0x08;

// MMIO Offsets
const CAP: usize = 0x0000;  // Controller Capabilities
const CC: usize = 0x0014;   // Controller Configuration
const CSTS: usize = 0x001C; // Controller Status
const AQA: usize = 0x0024;  // Admin Queue Attributes
const ASQ: usize = 0x0028;  // Admin Submission Queue Base Address
const ACQ: usize = 0x0030;  // Admin Completion Queue Base Address
const SQ0_TDBL: usize = 0x1000; // Submission Queue 0 Tail Doorbell
const CQ0_HDBL: usize = 0x1004; // Completion Queue 0 Head Doorbell

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct SqEntry {
    pub cdw0: u32,
    pub nsid: u32,
    pub rsvd2: u64,
    pub mptr: u64,
    pub prp1: u64,
    pub prp2: u64,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct CqEntry {
    pub cdw0: u32,
    pub rsvd1: u32,
    pub sq_head: u16,
    pub sq_id: u16,
    pub cid: u16,
    pub status: u16,
}

pub struct NvmeQueue {
    pub sq_virt: *mut SqEntry,
    pub sq_phys: u64,
    pub cq_virt: *mut CqEntry,
    pub cq_phys: u64,
    pub sq_tail: u16,
    pub cq_head: u16,
    pub phase: u16,
    pub size: u16,
    pub qid: u16,
    pub db_stride: usize,
    pub bar_virt: usize,
}

unsafe impl Send for NvmeQueue {}
unsafe impl Sync for NvmeQueue {}

impl NvmeQueue {
    pub fn new(bar_virt: usize, qid: u16, size: u16, db_stride: usize) -> Option<Self> {
        let (sq_phys, sq_virt) = match pmm::alloc_frames(1) {
            Some(p) => (p, vmm::phys_to_virt(p) as *mut SqEntry),
            None => return None,
        };
        let (cq_phys, cq_virt) = match pmm::alloc_frames(1) {
            Some(p) => (p, vmm::phys_to_virt(p) as *mut CqEntry),
            None => return None,
        };

        unsafe {
            core::ptr::write_bytes(sq_virt as *mut u8, 0, 4096);
            core::ptr::write_bytes(cq_virt as *mut u8, 0, 4096);
        }

        Some(Self {
            sq_virt,
            sq_phys,
            cq_virt,
            cq_phys,
            sq_tail: 0,
            cq_head: 0,
            phase: 1,
            size,
            qid,
            db_stride,
            bar_virt,
        })
    }

    pub fn submit(&mut self, mut entry: SqEntry) -> u16 {
        let cid = self.sq_tail;
        entry.cdw0 = (entry.cdw0 & !0xFFFF_0000) | ((cid as u32) << 16);
        
        unsafe {
            core::ptr::write_volatile(self.sq_virt.add(self.sq_tail as usize), entry);
        }
        
        self.sq_tail += 1;
        if self.sq_tail >= self.size {
            self.sq_tail = 0;
        }

        compiler_fence(Ordering::SeqCst);

        // Ring SQ Tail Doorbell
        let offset = 0x1000 + ((self.qid as usize * 2) * (4 << self.db_stride));
        unsafe {
            core::ptr::write_volatile((self.bar_virt + offset) as *mut u32, self.sq_tail as u32);
        }

        cid
    }

    pub fn poll(&mut self, cid: u16) -> Result<CqEntry, &'static str> {
        let mut timeout = 0;
        loop {
            let entry = unsafe { core::ptr::read_volatile(self.cq_virt.add(self.cq_head as usize)) };
            let p = (entry.status & 1) != 0;
            
            if p == (self.phase != 0) {
                compiler_fence(Ordering::Acquire);
                
                self.cq_head += 1;
                if self.cq_head >= self.size {
                    self.cq_head = 0;
                    self.phase ^= 1;
                }

                // Ring CQ Head Doorbell
                let offset = 0x1000 + (((self.qid as usize * 2) + 1) * (4 << self.db_stride));
                unsafe {
                    core::ptr::write_volatile((self.bar_virt + offset) as *mut u32, self.cq_head as u32);
                }

                if entry.cid == cid {
                    let st = entry.status >> 1;
                    if st == 0 {
                        return Ok(entry);
                    } else {
                        crate::serial_println!("[NVMe] Error en cola {}: status={:#x}", self.qid, st);
                        return Err("NVMe command failed");
                    }
                }
            }

            timeout += 1;
            if timeout > 100_000_000 {
                return Err("NVMe CQ poll timeout");
            }
            core::hint::spin_loop();
        }
    }
}

pub struct NvmeDriver {
    pub bar_virt: usize,
    pub db_stride: usize,
    pub admin_queue: Option<NvmeQueue>,
    pub io_queue: Option<NvmeQueue>,
    pub ns_size_sectors: u64,
    pub ns_id: u32,
    pub max_prp_pages: usize,
}

unsafe impl Send for NvmeDriver {}
unsafe impl Sync for NvmeDriver {}

pub static NVME: Mutex<Option<NvmeDriver>> = Mutex::new(None);

impl NvmeDriver {
    fn read_reg32(&self, offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((self.bar_virt + offset) as *const u32) }
    }
    
    fn write_reg32(&mut self, offset: usize, value: u32) {
        unsafe { core::ptr::write_volatile((self.bar_virt + offset) as *mut u32, value) }
    }

    fn read_reg64(&self, offset: usize) -> u64 {
        unsafe { core::ptr::read_volatile((self.bar_virt + offset) as *const u64) }
    }

    fn write_reg64(&mut self, offset: usize, value: u64) {
        unsafe { core::ptr::write_volatile((self.bar_virt + offset) as *mut u64, value) }
    }

    pub fn init() -> Result<(), &'static str> {
        // NVMe disabled for safety as requested by user
        Err("NVMe disabled for safety")
    }
}

pub struct NvmeBlockDevice;

impl BlockDevice for NvmeBlockDevice {
    fn read(&self, _lba: u64, _count: u16, _buffer: &mut [u8]) -> Result<(), &'static str> {
        Err("NVMe disabled")
    }

    fn write(&self, _lba: u64, _count: u16, _buffer: &[u8]) -> Result<(), &'static str> {
        Err("NVMe disabled")
    }

    fn capacity(&self) -> u64 {
        0
    }
}

pub fn init() -> Result<(), &'static str> {
    Err("NVMe disabled for safety")
}
