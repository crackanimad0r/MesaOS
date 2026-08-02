// mesa_kernel/src/drivers/mod.rs

pub mod framebuffer;
pub mod keyboard;
pub mod serial;

#[cfg(target_arch = "x86_64")]
pub mod ata;
#[cfg(target_arch = "x86_64")]
pub mod audio;
#[cfg(target_arch = "x86_64")]
pub mod battery;
#[cfg(target_arch = "x86_64")]
pub mod bios_analyzer;
pub mod block;
#[cfg(target_arch = "x86_64")]
pub mod hda;
#[cfg(target_arch = "x86_64")]
pub mod mouse;
#[cfg(target_arch = "x86_64")]
pub mod net;
#[cfg(target_arch = "x86_64")]
pub mod nvme;
#[cfg(target_arch = "x86_64")]
pub mod rtc;
#[cfg(target_arch = "x86_64")]
pub mod touchpad;
pub mod usb;

pub fn init_serial() {
    serial::init();
}

pub fn init_framebuffer(fb_ptr: *mut u8, width: usize, height: usize, pitch: usize, bpp: usize) {
    framebuffer::init(fb_ptr, width, height, pitch, bpp);
}

pub fn init_keyboard() {
    keyboard::init();
}

pub fn init_rtc() {
    #[cfg(target_arch = "x86_64")]
    rtc::init();
}

pub fn init_battery() {
    #[cfg(target_arch = "x86_64")]
    battery::init();
}

pub fn init_audio() {
    #[cfg(target_arch = "x86_64")]
    {
        audio::init();
        if let Err(e) = hda::init() {
            crate::serial_println!("[HDA] Error inicializando audio: {}", e);
        }
    }
}
