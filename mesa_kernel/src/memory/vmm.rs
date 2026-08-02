// mesa_kernel/src/memory/vmm.rs

pub fn init(hhdm_offset: u64) -> Result<(), &'static str> {
    HHDM_OFFSET.store(hhdm_offset, Relaxed);
    // Capturar el CR3 del kernel una vez, antes de crear cualquier proceso de usuario.
    #[cfg(target_arch = "x86_64")]
    {
        let cr3: u64;
        unsafe { core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack)); }
        KERNEL_CR3.store(cr3 & !0xFFF, Relaxed);
    }
    Ok(())
}

use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

static HHDM_OFFSET: AtomicU64 = AtomicU64::new(0);
static KERNEL_CR3: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn hhdm_offset() -> u64 {
    HHDM_OFFSET.load(Relaxed)
}

/// Devuelve el CR3 del kernel (PML4 del espacio de direcciones del kernel).
/// Se establece la primera vez que se llama a `init()`.
#[cfg(target_arch = "x86_64")]
#[inline]
pub fn kernel_cr3() -> u64 {
    KERNEL_CR3.load(Relaxed)
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
        // KERNER_RW | NO_CACHE | WRITE_THROUGH = UC (Uncacheable), required for proper MMIO
        let flags = crate::memory::page_flags::KERNEL_RW
            | crate::memory::page_flags::NO_CACHE
            | crate::memory::page_flags::WRITE_THROUGH;
        kernel_as.map_page(virt + offset, phys + offset, flags)?;
    }

    Ok(virt)
}
