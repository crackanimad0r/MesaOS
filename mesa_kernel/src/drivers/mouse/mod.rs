use alloc::collections::VecDeque;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEvent {
    Move(i32, i32),
    ButtonDown(MouseButton),
    ButtonUp(MouseButton),
    Scroll(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

static EVENT_BUFFER: Mutex<VecDeque<MouseEvent>> = Mutex::new(VecDeque::new());

const BUFFER_CAPACITY: usize = 64;

struct MouseState {
    buttons: u8,
    packet_buf: [u8; 4],
    packet_idx: u8,
    initialized: bool,
}

static STATE: Mutex<MouseState> = Mutex::new(MouseState {
    buttons: 0,
    packet_buf: [0u8; 4],
    packet_idx: 0,
    initialized: false,
});

pub fn init() {
    #[cfg(target_arch = "x86_64")]
    {
        if STATE.lock().initialized {
            return;
        }

        const PS2_TIMEOUT: u32 = 1_000_000;

        macro_rules! wait_write {
            ($sp:expr) => {{
                let mut timeout = PS2_TIMEOUT;
                while (unsafe { ($sp).read() } & 0x02) != 0 && timeout > 0 {
                    timeout -= 1;
                    core::hint::spin_loop();
                }
                timeout > 0
            }};
        }

        macro_rules! wait_read {
            ($sp:expr) => {{
                let mut timeout = PS2_TIMEOUT;
                while (unsafe { ($sp).read() } & 0x01) == 0 && timeout > 0 {
                    timeout -= 1;
                    core::hint::spin_loop();
                }
                timeout > 0
            }};
        }

        unsafe {
            crate::serial_println!("[MOUSE] Inicializando mouse PS/2...");

            // Helper to send command to mouse via PS/2 auxiliary channel
            unsafe fn mouse_cmd(
                cp: &mut x86_64::instructions::port::Port<u8>,
                dp: &mut x86_64::instructions::port::Port<u8>,
                sp: &mut x86_64::instructions::port::Port<u8>,
                cmd: u8,
            ) -> Option<u8> {
                if !wait_write!(sp) {
                    return None;
                }
                cp.write(0xD4);
                if !wait_write!(sp) {
                    return None;
                }
                dp.write(cmd);
                if !wait_read!(sp) {
                    return None;
                }
                Some(dp.read())
            }

            let mut sp: x86_64::instructions::port::Port<u8> =
                x86_64::instructions::port::Port::new(0x64);
            let mut cp: x86_64::instructions::port::Port<u8> =
                x86_64::instructions::port::Port::new(0x64);
            let mut dp: x86_64::instructions::port::Port<u8> =
                x86_64::instructions::port::Port::new(0x60);

            // 1. Enable auxiliary device
            if wait_write!(sp) {
                cp.write(0xA8);
                crate::serial_println!("[MOUSE] Auxiliary device enabled");
            }

            // 2. Read and modify CCB: enable IRQ12 (bit 1), mouse clock (bit 5)
            if wait_write!(sp) {
                cp.write(0x20);
                if wait_read!(sp) {
                    let mut ccb = dp.read();
                    ccb |= 0x02;
                    ccb |= 0x20;
                    if wait_write!(sp) {
                        cp.write(0x60);
                        if wait_write!(sp) {
                            dp.write(ccb);
                            crate::serial_println!("[MOUSE] CCB updated: {:#x}", ccb);
                        }
                    }
                }
            }

            // 4. Reset mouse
            crate::serial_println!("[MOUSE] Resetting mouse...");
            if let Some(ack) = mouse_cmd(&mut cp, &mut dp, &mut sp, 0xFF) {
                if ack == 0xFA {
                    if wait_read!(sp) {
                        let self_test = dp.read();
                        crate::serial_println!("[MOUSE] Self-test: {:#x}", self_test);
                    }
                    if wait_read!(sp) {
                        let dev_id = dp.read();
                        crate::serial_println!("[MOUSE] Device ID: {:#x}", dev_id);
                    }
                } else {
                    crate::serial_println!("[MOUSE] Reset ACK failed: {:#x}", ack);
                }
            }

            // 5. Set sample rate to 100Hz
            mouse_cmd(&mut cp, &mut dp, &mut sp, 0xF3);
            if wait_write!(sp) {
                cp.write(0xD4);
                if wait_write!(sp) {
                    dp.write(100);
                }
            }

            // 6. Enable data reporting
            crate::serial_println!("[MOUSE] Enabling data reporting...");
            if let Some(ack) = mouse_cmd(&mut cp, &mut dp, &mut sp, 0xF4) {
                if ack == 0xFA {
                    crate::serial_println!("[MOUSE] Data reporting enabled (ACK)");
                } else {
                    crate::serial_println!("[MOUSE] Enable ACK failed: {:#x}", ack);
                }
            }

            // 7. Set default resolution (4 counts/mm)
            mouse_cmd(&mut cp, &mut dp, &mut sp, 0xE8);
            if wait_write!(sp) {
                cp.write(0xD4);
                if wait_write!(sp) {
                    dp.write(2);
                }
            }

            // Flush any stale data
            while (sp.read() & 0x01) != 0 {
                dp.read();
            }

            STATE.lock().initialized = true;
            crate::serial_println!("[MOUSE] Inicializacion completada.");
        }
    }
}

pub fn handle_data(data: u8) {
    let mut state = STATE.lock();
    if !state.initialized {
        return;
    }

    let idx = state.packet_idx as usize;
    state.packet_buf[idx] = data;
    state.packet_idx += 1;

    // 3-byte packets (standard PS/2 mouse)
    if state.packet_idx >= 3 {
        let b0 = state.packet_buf[0];
        let dx = state.packet_buf[1] as i32;
        let dy = state.packet_buf[2] as i32;

        // Sign extension for 9-bit signed values
        let dx = if (b0 & 0x10) != 0 { dx | !0xFF } else { dx };
        let dy = if (b0 & 0x20) != 0 { dy | !0xFF } else { dy };

        let new_buttons = b0 & 0x07;

        // Detect button changes
        let old = state.buttons;
        if (new_buttons & 1) != (old & 1) {
            push_event(if (new_buttons & 1) != 0 {
                MouseEvent::ButtonDown(MouseButton::Left)
            } else {
                MouseEvent::ButtonUp(MouseButton::Left)
            });
        }
        if (new_buttons & 2) != (old & 2) {
            push_event(if (new_buttons & 2) != 0 {
                MouseEvent::ButtonDown(MouseButton::Right)
            } else {
                MouseEvent::ButtonUp(MouseButton::Right)
            });
        }
        if (new_buttons & 4) != (old & 4) {
            push_event(if (new_buttons & 4) != 0 {
                MouseEvent::ButtonDown(MouseButton::Middle)
            } else {
                MouseEvent::ButtonUp(MouseButton::Middle)
            });
        }

        // Mouse Y is inverted in PS/2 convention
        if dx != 0 || dy != 0 {
            push_event(MouseEvent::Move(dx, -dy));
        }

        state.buttons = new_buttons;
        state.packet_idx = 0;
    }
}

fn push_event(event: MouseEvent) {
    if let Some(mut buf) = EVENT_BUFFER.try_lock() {
        if buf.len() < BUFFER_CAPACITY {
            buf.push_back(event);
        }
    }
}

pub fn read_event() -> Option<MouseEvent> {
    EVENT_BUFFER.lock().pop_front()
}

pub fn has_events() -> bool {
    !EVENT_BUFFER.lock().is_empty()
}

pub fn clear_buffer() {
    EVENT_BUFFER.lock().clear();
}

/// Allows the touchpad driver to inject mouse events (button clicks)
pub fn inject_event(event: MouseEvent) {
    push_event(event);
}

pub fn is_initialized() -> bool {
    STATE.lock().initialized
}
