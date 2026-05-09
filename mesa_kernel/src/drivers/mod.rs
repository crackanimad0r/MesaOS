// mesa_kernel/src/drivers/mod.rs

pub mod serial;
pub mod framebuffer;
pub mod keyboard;

#[cfg(target_arch = "x86_64")]
pub mod rtc;
#[cfg(target_arch = "x86_64")]
pub mod ata;
#[cfg(target_arch = "x86_64")]
pub mod net;
#[cfg(target_arch = "x86_64")]
pub mod audio;
#[cfg(target_arch = "x86_64")]
pub mod bios_analyzer;
pub mod usb;
pub mod block;
#[cfg(target_arch = "x86_64")]
pub mod nvme;
#[cfg(target_arch = "x86_64")]
pub mod battery;

pub fn init_serial() {
    serial::init();
}

pub fn init_framebuffer(
    fb_ptr: *mut u8,
    width: usize,
    height: usize,
    pitch: usize,
    bpp: usize,
) {
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