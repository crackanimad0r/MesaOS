use core::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug)]
pub struct completion {
    done: AtomicU32,
}

impl completion {
    pub const fn new() -> Self {
        Self {
            done: AtomicU32::new(0),
        }
    }

    pub fn wait_for_completion(&self) {
        while self.done.load(Ordering::Acquire) == 0 {
            crate::scheduler::yield_now();
        }
    }

    pub fn complete(&self) {
        self.done.store(1, Ordering::Release);
    }

    pub fn reinit_completion(&self) {
        self.done.store(0, Ordering::Release);
    }
}

pub unsafe fn wait_for_completion(c: &completion) {
    c.wait_for_completion();
}

pub unsafe fn complete(c: &completion) {
    c.complete();
}

pub unsafe fn reinit_completion(c: &completion) {
    c.reinit_completion();
}
