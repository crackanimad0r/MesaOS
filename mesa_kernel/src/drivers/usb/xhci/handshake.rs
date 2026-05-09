use crate::serial_println;

pub fn perform_bios_handoff(base_virt: u64) {
    let mut offset = unsafe { core::ptr::read_volatile((base_virt + 0x10) as *const u32) };
    let xecp = (offset >> 16) & 0xFFFF;
    if xecp == 0 {
        serial_println!("[XHCI] Handshake: No Extended Capabilities found.");
        return;
    }

    let mut current = (xecp as u64) << 2;
    loop {
        let cap_base = base_virt + current;
        let cap_val = unsafe { core::ptr::read_volatile(cap_base as *const u32) };
        let cap_id = cap_val & 0xFF;
        let next = (cap_val >> 8) & 0xFF;

        if cap_id == 1 {
            let mut legsup = cap_val;
            if (legsup & (1 << 16)) != 0 {
                serial_println!("[XHCI] Handshake: BIOS owns the controller. Requesting OS ownership...");
                legsup |= 1 << 24;
                unsafe { core::ptr::write_volatile(cap_base as *mut u32, legsup); }

                let start_tick = crate::curr_arch::get_ticks();
                loop {
                    let curr = unsafe { core::ptr::read_volatile(cap_base as *const u32) };
                    if (curr & (1 << 16)) == 0 && (curr & (1 << 24)) != 0 {
                        serial_println!("[XHCI] Handshake: Success! OS took Ownership!");
                        break;
                    }
                    if crate::curr_arch::get_ticks().wrapping_sub(start_tick) > 20 { // ~1 second
                        serial_println!("[XHCI] Handshake: Timeout. Continuing anyway...");
                        break;
                    }
                    core::hint::spin_loop();
                }
            } else {
                serial_println!("[XHCI] Handshake: OS already owns the controller.");
                legsup |= 1 << 24;
                unsafe { core::ptr::write_volatile(cap_base as *mut u32, legsup); }
            }

            let ctl_base = cap_base + 4;
            let ctl_val = unsafe { core::ptr::read_volatile(ctl_base as *const u32) };
            unsafe { core::ptr::write_volatile(ctl_base as *mut u32, ctl_val & 0x1FFFFF); }
            break;
        }

        if next == 0 {
            serial_println!("[XHCI] Handshake: USB Legacy Support Capability not found.");
            break;
        }
        current += (next as u64) << 2;
    }
}
