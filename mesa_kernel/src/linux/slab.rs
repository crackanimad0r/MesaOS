// src/linux/slab.rs
use core::alloc::Layout;


pub const GFP_KERNEL: u32 = 0;
pub const GFP_ATOMIC: u32 = 1;

pub unsafe fn kmalloc(size: usize, _flags: u32) -> *mut u8 {
    if size == 0 { return core::ptr::null_mut(); }
    
    let layout = Layout::from_size_align(size, 8).unwrap();
    alloc::alloc::alloc(layout)
}

pub unsafe fn kfree(ptr: *mut u8) {
    if ptr.is_null() { return; }
    // Note: Rust's dealloc requires layout (size/align).
    // This is a major difference from C kmalloc.
    // For a better shim, we'd need a header tracking sizes.
    // For now, we'll have to rely on callers knowing the size or 
    // provide a wrapper that stores it.
}


pub unsafe fn kzalloc(size: usize, flags: u32) -> *mut u8 {
    let ptr = kmalloc(size, flags);
    if !ptr.is_null() {
        core::ptr::write_bytes(ptr, 0, size);
    }
    ptr
}
