// mesa_kernel/src/drivers/serial.rs

#[cfg(target_arch = "x86_64")]
use uart_16550::SerialPort;
use spin::Mutex;
use core::fmt;

#[cfg(target_arch = "x86_64")]
static SERIAL1: Mutex<Option<SerialPort>> = Mutex::new(None);

#[cfg(target_arch = "aarch64")]
static SERIAL1: Mutex<Option<()>> = Mutex::new(None); // Placeholder

pub fn init() {
    #[cfg(target_arch = "x86_64")]
    {
        let mut serial = unsafe { SerialPort::new(0x3F8) };
        serial.init();
        *SERIAL1.lock() = Some(serial);
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    
    crate::curr_arch::disable_interrupts();
    
    #[cfg(target_arch = "x86_64")]
    {
        if let Some(ref mut serial) = *SERIAL1.lock() {
            let _ = serial.write_fmt(args);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // TODO: AArch64 PL011 write
    }
    
    crate::curr_arch::enable_interrupts();
}

// Macros para serial
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::drivers::serial::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}

// Macros unificadas que escriben a AMBOS (serial + shell/redirección)
#[macro_export]
macro_rules! mesa_print {
    ($($arg:tt)*) => {{
        $crate::drivers::serial::_print(format_args!($($arg)*));
        $crate::shell_stdout(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! mesa_println {
    () => ($crate::mesa_print!("\n"));
    ($($arg:tt)*) => ($crate::mesa_print!("{}\n", format_args!($($arg)*)));
}