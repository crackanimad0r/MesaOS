use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub struct wait_queue_entry {
    pub flags: u32,
    pub private: u64,
    pub func: Option<unsafe extern "C" fn(*mut wait_queue_entry, u32, u64, *mut u8) -> i32>,
    pub entry: crate::linux::list_head,
}

#[derive(Debug)]
pub struct wait_queue_head {
    pub head: crate::linux::list_head,
    pub lock: u64,
}

impl wait_queue_head {
    pub const fn new() -> Self {
        Self {
            head: crate::linux::list_head::new(),
            lock: 0,
        }
    }

    pub unsafe fn init_waitqueue_head(&mut self) {
        self.head.init();
    }
}

pub unsafe fn init_waitqueue_entry(entry: *mut wait_queue_entry, flags: u32, private: u64) {
    (*entry).flags = flags;
    (*entry).private = private;
    (*entry).entry = crate::linux::list_head::new();
    (*entry).entry.init();
}

pub unsafe fn add_wait_queue(wq: *mut wait_queue_head, entry: *mut wait_queue_entry) {
    (*wq).head.add(&mut (*entry).entry);
}

pub unsafe fn remove_wait_queue(wq: *mut wait_queue_head, entry: *mut wait_queue_entry) {
    (*entry).entry.del();
}

pub unsafe fn wake_up(wq: *mut wait_queue_head) -> i32 {
    let mut count = 0;
    let mut curr = (*wq).head.next;
    while curr != &mut (*wq).head as *mut _ {
        let entry = curr as *mut wait_queue_entry;
        if let Some(func) = (*entry).func {
            let _ = func(entry, 0, 0, core::ptr::null_mut());
        }
        curr = (*curr).next;
        count += 1;
    }
    count
}

pub unsafe fn wait_event_interruptible(wq: *mut wait_queue_head, condition: bool) -> i32 {
    while !condition {
        add_wait_queue(wq, core::ptr::null_mut());
        crate::scheduler::yield_now();
        remove_wait_queue(wq, core::ptr::null_mut());
    }
    0
}
