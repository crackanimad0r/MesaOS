// mesa_kernel/src/drivers/keyboard/mod.rs

pub mod scancode;

use alloc::collections::VecDeque;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Char(char),
    Special(SpecialKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    Enter,
    Backspace,
    Escape,
    Tab,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Insert,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    /// Ctrl+C (SIGINT)
    CtrlC,
    /// Ctrl+S (Save)
    CtrlS,
    /// Ctrl+X (Exit)
    CtrlX,
}

static EVENT_BUFFER: Mutex<VecDeque<KeyEvent>> = Mutex::new(VecDeque::new());

const BUFFER_CAPACITY: usize = 64;

pub fn init() {
    #[cfg(target_arch = "x86_64")]
    {
        use x86_64::instructions::port::Port;
        
        let mut status_port: Port<u8> = Port::new(0x64);
        let mut command_port: Port<u8> = Port::new(0x64);
        let mut data_port: Port<u8> = Port::new(0x60);
        
        const PS2_TIMEOUT: u32 = 1_000_000;

        macro_rules! wait_write {
            () => {{
                let mut timeout = PS2_TIMEOUT;
                while (unsafe { status_port.read() } & 0x02) != 0 && timeout > 0 {
                    timeout -= 1;
                    core::hint::spin_loop();
                }
                timeout > 0
            }};
        }

        macro_rules! wait_read {
            () => {{
                let mut timeout = PS2_TIMEOUT;
                while (unsafe { status_port.read() } & 0x01) == 0 && timeout > 0 {
                    timeout -= 1;
                    core::hint::spin_loop();
                }
                timeout > 0
            }};
        }

        unsafe {
            crate::serial_println!("[KEYBOARD] Iniciando inicializacion robusta...");
            crate::mesa_print!("[BOOT] Teclado: ");

            // Verificar si ACPI dice que existe el 8042
            if let Some(info) = crate::acpi::get_info() {
                if !info.has_8042 {
                    crate::serial_println!("[KEYBOARD] WARNING: ACPI indica que no hay controlador 8042");
                    crate::mesa_print!("(No detectado por ACPI) ");
                }
            }

            // 1. Limpiar buffer de salida
            while (status_port.read() & 0x01) != 0 {
                data_port.read();
            }

            // 2. Deshabilitar puertos (Kbd y Mouse)
            if wait_write!() { command_port.write(0xAD); }
            if wait_write!() { command_port.write(0xA7); }

            // 3. Flush final
            while (status_port.read() & 0x01) != 0 {
                data_port.read();
            }

            // 4. Controller Self-Test
            if wait_write!() {
                command_port.write(0xAA);
                if wait_read!() {
                    let res = data_port.read();
                    if res != 0x55 {
                        crate::serial_println!("[KEYBOARD] WARNING: Self-test falló ({:#x})", res);
                    } else {
                        crate::serial_println!("[KEYBOARD] Controller Self-test: OK");
                    }
                } else {
                    crate::serial_println!("[KEYBOARD] ERROR: Timeout en self-test");
                }
            }

            // 5. Configurar CCB (Controller Configuration Byte)
            let mut final_ccb = 0;
            if wait_write!() {
                command_port.write(0x20); // Read CCB
                if wait_read!() {
                    let mut ccb = data_port.read();
                    crate::serial_println!("[KEYBOARD] CCB original: {:#x}", ccb);
                    
                    ccb |= 0x01; // Enable IRQ1
                    ccb |= 0x40; // ENABLE Translation (Set 2 -> Set 1 conversion)
                    ccb &= !0x10; // Ensure keyboard port is enabled
                    
                    if wait_write!() {
                        command_port.write(0x60); // Write CCB
                        if wait_write!() {
                            data_port.write(ccb);
                            
                            // Verificar que se guardó
                            if wait_write!() {
                                command_port.write(0x20);
                                if wait_read!() {
                                    final_ccb = data_port.read();
                                    crate::serial_println!("[KEYBOARD] CCB verificado (Traduccion ON): {:#x}", final_ccb);
                                }
                            }
                        }
                    }
                }
            }

            // 6. Habilitar puerto del teclado
            if wait_write!() {
                command_port.write(0xAE);
            }

            // 7. Forzar Set 2 en el hardware (con reintentos)
            for i in 0..3 {
                crate::serial_println!("[KEYBOARD] Intento {} de forzar Set 2...", i + 1);
                if wait_write!() {
                    data_port.write(0xF0); // Set Scancode Set
                    if wait_write!() {
                        data_port.write(0x02); // Set 2
                        if wait_read!() {
                            let res = data_port.read();
                            if res == 0xFA {
                                crate::serial_println!("[KEYBOARD] Hardware aceptó Set 2 (ACK)");
                                break;
                            } else {
                                crate::serial_println!("[KEYBOARD] Hardware respondió {:#x} al Set 2", res);
                            }
                        }
                    }
                }
            }
        }

        scancode::init();
        crate::serial_println!("[KEYBOARD] Inicialización finalizada.");
        crate::mesa_println!("[OK]");
    }
    
    #[cfg(target_arch = "aarch64")]
    {
        // TODO: USB Keyboard for RPi
    }
}

pub fn handle_interrupt_simple(scancode: u8) {
    // VISUAL FEEDBACK: Removed for clean input
    // crate::mesa_print!("*"); 
    
    if let Some(event) = scancode::decode(scancode) {
        if let Some(mut buffer) = EVENT_BUFFER.try_lock() {
            if buffer.len() < BUFFER_CAPACITY {
                buffer.push_back(event);
            }
        }
    }
}

pub fn read_event() -> Option<KeyEvent> {
    EVENT_BUFFER.lock().pop_front()
}

pub fn has_events() -> bool {
    !EVENT_BUFFER.lock().is_empty()
}

pub fn clear_buffer() {
    EVENT_BUFFER.lock().clear();
}