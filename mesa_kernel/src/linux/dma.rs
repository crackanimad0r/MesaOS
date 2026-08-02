use crate::memory::{pmm, vmm, PAGE_SIZE};
use core::ptr;

pub const DMA_BIDIRECTIONAL: i32 = 0;
pub const DMA_TO_DEVICE: i32 = 1;
pub const DMA_FROM_DEVICE: i32 = 2;
pub const DMA_NONE: i32 = 3;

pub unsafe fn dma_alloc_coherent(size: usize) -> (*mut u8, u64) {
    if size == 0 {
        return (ptr::null_mut(), 0);
    }
    let pages = (size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;
    let phys = match pmm::alloc_frames(pages) {
        Some(p) => p,
        None => return (ptr::null_mut(), 0),
    };
    let virt = vmm::phys_to_virt(phys);
    ptr::write_bytes(virt as *mut u8, 0, pages * PAGE_SIZE as usize);
    (virt as *mut u8, phys)
}

pub unsafe fn dma_free_coherent(phys: u64, size: usize) {
    if phys == 0 {
        return;
    }
    let pages = (size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;
    for i in 0..pages {
        pmm::free_frame(phys + (i as u64) * PAGE_SIZE);
    }
}

pub unsafe fn dma_map_single(phys: u64, _direction: i32) -> u64 {
    phys
}

pub unsafe fn dma_unmap_single(phys: u64, _direction: i32) {
    let _ = phys;
}

pub unsafe fn dma_sync_single_for_device(phys: u64) {
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}

pub unsafe fn dma_sync_single_for_cpu(phys: u64) {
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}
