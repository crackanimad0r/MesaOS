use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub struct mutex {
    locked: AtomicBool,
}

impl mutex {
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) {
        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
    }

    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }

    pub fn try_lock(&self) -> bool {
        !self.locked.swap(true, Ordering::Acquire)
    }
}

pub unsafe fn mutex_lock(m: &mut mutex) {
    m.lock();
}

pub unsafe fn mutex_unlock(m: &mut mutex) {
    m.unlock();
}

pub unsafe fn mutex_trylock(m: &mut mutex) -> bool {
    m.try_lock()
}
