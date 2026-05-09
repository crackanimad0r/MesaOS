//! Sistema de memoria del kernel

pub mod addr;
pub mod pmm;
pub mod heap;
pub mod paging;
pub mod vmm;
pub mod address_space;
pub mod xhci_access;

pub use xhci_access::{XhciMemory, PmmXhciAdapter};

#[cfg(target_arch = "x86_64")]
use crate::arch::x86_64::limine_req;

pub use paging::PAGE_SIZE;
pub use vmm::{phys_to_virt, virt_to_phys};
pub use address_space::{AddressSpace, flags as page_flags, layout as user_layout};

pub struct MemoryInfo {
    pub usable_memory: u64,
    pub total_memory: u64,
}

pub fn init() -> Result<MemoryInfo, &'static str> {
    #[cfg(target_arch = "x86_64")]
    {
        let hhdm_offset = limine_req::hhdm_offset().ok_or("No HHDM")?;
        let entries = limine_req::memory_map_entries().ok_or("No memory map")?;
        
        pmm::init(entries, hhdm_offset)?;
        vmm::init(hhdm_offset)?;
        heap::init()?;
        
        let (_free_frames, total_frames) = pmm::stats();
        let total_memory = total_frames * PAGE_SIZE;
        
        Ok(MemoryInfo {
            usable_memory: total_memory,
            total_memory,
        })
    }
    #[cfg(target_arch = "aarch64")]
    {
        // RPi stub: physical memory is fixed or probed via Devicetree
        // For now, assume a 512MB RAM and 0 hhdm_offset if not provided by bootloader
        heap::init()?;
        vmm::init(0)?; // TODO: actual offset
        Ok(MemoryInfo { 
            usable_memory: 512 * 1024 * 1024, 
            total_memory: 512 * 1024 * 1024 
        })
    }
}