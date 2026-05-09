use xhci::ring::trb::Link;

pub struct CommandRing {
    pub ring_virt: *mut [u32; 4],
    pub ring_phys: u64,
    pub index: usize,
    pub cycle: bool,
    pub len: usize,
}

impl CommandRing {
    pub fn new(ring_virt: *mut u8, ring_phys: u64, len: usize) -> Self {
        Self {
            ring_virt: ring_virt as *mut [u32; 4],
            ring_phys,
            index: 0,
            cycle: true, // initial cycle is 1
            len,
        }
    }

    pub fn push_raw(&mut self, mut raw: [u32; 4]) {
        // Set cycle bit on raw TRB
        if self.cycle {
            raw[3] |= 1;
        } else {
            raw[3] &= !1;
        }

        unsafe { core::ptr::write_volatile(self.ring_virt.add(self.index), raw); }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);

        self.index += 1;
        if self.index == self.len - 1 {
            // Write Link TRB at the end
            let mut link = Link::new();
            link.set_ring_segment_pointer(self.ring_phys);
            link.set_toggle_cycle();
            
            let mut raw_link = link.into_raw();
            if self.cycle {
                raw_link[3] |= 1;
            } else {
                raw_link[3] &= !1;
            }
            
            unsafe { core::ptr::write_volatile(self.ring_virt.add(self.index), raw_link); }
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
            
            self.cycle = !self.cycle;
            self.index = 0;
        }
    }
}

pub struct EventRing {
    pub ring_virt: *mut [u32; 4],
    pub ring_phys: u64,
    pub index: usize,
    pub cycle: bool,
    pub len: usize,
}

impl EventRing {
    pub fn new(ring_virt: *mut u8, ring_phys: u64, len: usize) -> Self {
        Self {
            ring_virt: ring_virt as *mut [u32; 4],
            ring_phys,
            index: 0,
            cycle: true,
            len,
        }
    }

    pub fn poll(&mut self) -> Option<[u32; 4]> {
        unsafe {
            let trb = core::ptr::read_volatile(self.ring_virt.add(self.index));
            let cycle_bit = (trb[3] & 1) != 0;
            if cycle_bit == self.cycle {
                core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Acquire);
                self.index += 1;
                if self.index == self.len {
                    self.cycle = !self.cycle;
                    self.index = 0;
                }
                Some(trb)
            } else {
                None
            }
        }
    }
}

pub struct TransferRing {
    pub ring_virt: *mut [u32; 4],
    pub ring_phys: u64,
    pub index: usize,
    pub cycle: bool,
    pub len: usize,
}

impl TransferRing {
    pub fn new(ring_virt: *mut u8, ring_phys: u64, len: usize) -> Self {
        Self {
            ring_virt: ring_virt as *mut [u32; 4],
            ring_phys,
            index: 0,
            cycle: true,
            len, // Generally 256 for EP0
        }
    }

    pub fn push_raw(&mut self, mut raw: [u32; 4]) {
        // Enforce cycle bit
        if self.cycle {
            raw[3] |= 1;
        } else {
            raw[3] &= !1;
        }
        unsafe { core::ptr::write_volatile(self.ring_virt.add(self.index), raw); }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);

        self.index += 1;
        if self.index == self.len - 1 {
            // Write Link TRB
            let mut link = Link::new();
            link.set_ring_segment_pointer(self.ring_phys);
            link.set_toggle_cycle();
            
            let mut raw_link = link.into_raw();
            if self.cycle {
                raw_link[3] |= 1;
            } else {
                raw_link[3] &= !1;
            }
            
            unsafe { core::ptr::write_volatile(self.ring_virt.add(self.index), raw_link); }
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
            
            self.cycle = !self.cycle;
            self.index = 0;
        }
    }
}
