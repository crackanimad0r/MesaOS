// mesa_kernel/src/drivers/framebuffer/mod.rs

pub mod font;
pub mod console;
pub mod ui;

pub use console::Color;
pub use ui::palette;
pub mod html;

pub fn init(fb_ptr: *mut u8, width: usize, height: usize, pitch: usize, bpp: usize) {
    console::init(fb_ptr, width, height, pitch, bpp);
}

pub fn clear() {
    console::clear();
}

pub fn set_color(color: Color) {
    console::set_color(color);
}

pub fn update_status_bar(
    used_mb: u64, 
    total_mb: u64, 
    cpu_count: usize,
    hour: u8,
    minute: u8,
    second: u8,
    disk_used: u64,
    disk_total: u64,
) {
    console::update_status_bar(used_mb, total_mb, cpu_count, hour, minute, second, disk_used, disk_total);
}

pub fn get_info() -> (*mut u8, usize, usize, usize, usize) {
    console::get_info()
}

pub fn lock() {
    console::lock();
}

pub fn unlock() {
    console::unlock();
}
pub fn redraw_full() {
    console::redraw_full();
}
