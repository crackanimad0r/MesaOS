// mesa_kernel/src/memory/vmm.rs

pub fn init(hhdm_offset: u64) -> Result<(), &'static str> {
    HHDM_OFFSET.store(hhdm_offset, Relaxed);
    Ok(())
}

use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

static HHDM_OFFSET: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn hhdm_offset() -> u64 {
    HHDM_OFFSET.load(Relaxed)
}

#[inline]
pub fn phys_to_virt(phys: u64) -> u64 {
    hhdm_offset() + phys
}

#[inline]
pub fn virt_to_phys(virt: u64) -> u64 {
    virt - hhdm_offset()
}

pub fn map_mmio(phys: u64, size: u64) -> Result<u64, &'static str> {
    let virt = phys_to_virt(phys);
    let mut kernel_as = crate::memory::AddressSpace::kernel();
    let pages = (size + 4095) / 4096;
    
    for i in 0..pages {
        let offset = i * 4096;
        // KERNEL_RW | NO_CACHE | WRITE_THROUGH (usually best for MMIO)
        let flags = crate::memory::page_flags::KERNEL_RW | crate::memory::page_flags::NO_CACHE;
        kernel_as.map_page(virt + offset, phys + offset, flags)?;
    }
    
    Ok(virt)
}