use super::addr::PhysAddr;
use core::ptr::{read_volatile, write_volatile};
use crate::memory::{pmm, vmm};

pub trait XhciMemory {
    fn alloc_64byte(&mut self, size_bytes: usize) -> Option<PhysAddr>;
    fn virt_from_phys(&self, phys: PhysAddr) -> *mut u8;
}

impl XhciMemory for PmmXhciAdapter {
    fn alloc_64byte(&mut self, size_bytes: usize) -> Option<PhysAddr> {
        let pages = (size_bytes + 4095) / 4096;
        if let Some(phys) = pmm::alloc_frames(pages as usize) {
            Some(PhysAddr::new(phys))
        } else {
            None
        }
    }

    fn virt_from_phys(&self, phys: PhysAddr) -> *mut u8 {
        vmm::phys_to_virt(phys.as_u64()) as *mut u8
    }
}

pub struct PmmXhciAdapter;

pub use xhci::accessor::Mapper as XhciMapper;

pub unsafe trait MmioOps {
    unsafe fn read32(&self, offset: usize) -> u32;
    unsafe fn write32(&self, offset: usize, val: u32);
}

#[repr(transparent)]
pub struct MmioPtr(*mut u8);

impl MmioPtr {
    pub const fn new(ptr: *mut u8) -> Self { Self(ptr) }
}

unsafe impl MmioOps for MmioPtr {
    #[inline]
    unsafe fn read32(&self, offset: usize) -> u32 {
        read_volatile((self.0 as usize + offset) as *const u32)
    }
    
    #[inline]
    unsafe fn write32(&self, offset: usize, val: u32) {
        write_volatile((self.0 as usize + offset) as *mut u32, val)
    }
}

