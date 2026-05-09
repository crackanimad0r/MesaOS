// src/linux/list.rs
// Simple circular doubly linked list in the style of Linux list_head

#[repr(C)]
#[derive(Copy, Clone)]
pub struct list_head {
    pub next: *mut list_head,
    pub prev: *mut list_head,
}

impl list_head {
    pub fn new() -> Self {
        Self {
            next: core::ptr::null_mut(),
            prev: core::ptr::null_mut(),
        }
    }

    pub unsafe fn init(&mut self) {
        self.next = self as *mut _;
        self.prev = self as *mut _;
    }

    pub unsafe fn add(&mut self, new: *mut list_head) {
        let prev = self as *mut _;
        let next = self.next;
        (*new).next = next;
        (*new).prev = prev;
        (*next).prev = new;
        (*prev).next = new;
    }

    pub unsafe fn del(&mut self) {
        (*self.prev).next = self.next;
        (*self.next).prev = self.prev;
        self.next = core::ptr::null_mut();
        self.prev = core::ptr::null_mut();
    }
}
