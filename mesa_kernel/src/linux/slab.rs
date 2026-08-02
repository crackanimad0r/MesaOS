use alloc::vec::Vec;
use core::alloc::Layout;
use spin::Mutex;

pub const GFP_KERNEL: u32 = 0;
pub const GFP_ATOMIC: u32 = 1;
pub const GFP_DMA: u32 = 2;

struct AllocHeader {
    size: usize,
    magic: u32,
}

const HEADER_MAGIC: u32 = 0xDEADBEEF;

static ALLOC_TRACKER: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

pub unsafe fn kmalloc(size: usize, _flags: u32) -> *mut u8 {
    if size == 0 {
        return core::ptr::null_mut();
    }
    let total = size + core::mem::size_of::<AllocHeader>();
    let layout = Layout::from_size_align(total, 16).unwrap();
    let ptr = alloc::alloc::alloc(layout);
    if ptr.is_null() {
        return core::ptr::null_mut();
    }
    let header = &mut *(ptr as *mut AllocHeader);
    header.size = size;
    header.magic = HEADER_MAGIC;
    let data = ptr.add(core::mem::size_of::<AllocHeader>());
    ALLOC_TRACKER.lock().push((data as usize, size));
    data
}

pub unsafe fn kfree(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let header_ptr = ptr.sub(core::mem::size_of::<AllocHeader>());
    let header = &*(header_ptr as *const AllocHeader);
    if header.magic != HEADER_MAGIC {
        return;
    }
    let total = header.size + core::mem::size_of::<AllocHeader>();
    let layout = Layout::from_size_align(total, 16).unwrap();
    ALLOC_TRACKER.lock().retain(|&(a, _)| a != ptr as usize);
    alloc::alloc::dealloc(header_ptr, layout);
}

pub unsafe fn kzalloc(size: usize, flags: u32) -> *mut u8 {
    let ptr = kmalloc(size, flags);
    if !ptr.is_null() {
        core::ptr::write_bytes(ptr, 0, size);
    }
    ptr
}

pub unsafe fn krealloc(ptr: *mut u8, new_size: usize, flags: u32) -> *mut u8 {
    if ptr.is_null() {
        return kmalloc(new_size, flags);
    }
    if new_size == 0 {
        kfree(ptr);
        return core::ptr::null_mut();
    }
    let new_ptr = kmalloc(new_size, flags);
    if new_ptr.is_null() {
        return core::ptr::null_mut();
    }
    let header_ptr = ptr.sub(core::mem::size_of::<AllocHeader>());
    let header = &*(header_ptr as *const AllocHeader);
    let old_size = header.size;
    let copy_size = if old_size < new_size {
        old_size
    } else {
        new_size
    };
    core::ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
    kfree(ptr);
    new_ptr
}

pub unsafe fn vmalloc(size: usize) -> *mut u8 {
    kmalloc(size, GFP_KERNEL)
}

pub unsafe fn vfree(ptr: *mut u8) {
    kfree(ptr);
}

pub unsafe fn kcalloc(n: usize, size: usize, flags: u32) -> *mut u8 {
    kzalloc(n * size, flags)
}

pub fn alloc_tracker_stats() -> (usize, usize) {
    let tracker = ALLOC_TRACKER.lock();
    let count = tracker.len();
    let total: usize = tracker.iter().map(|&(_, s)| s).sum();
    (count, total)
}
