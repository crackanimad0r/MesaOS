// mesa_kernel/src/drivers/framebuffer/console.rs

use spin::Mutex;
use core::fmt;
use alloc::format;
use super::font::{FONT_WIDTH, FONT_HEIGHT, get_char};
use super::ui::{UiRenderer, palette};

#[derive(Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

pub struct Console {
    fb_ptr: *mut u8,
    width: usize,
    height: usize,
    pitch: usize,
    bpp: usize,
    col: usize,
    row: usize,
    max_cols: usize,
    max_rows: usize,
    fg_color: Color,
    text_x: usize,
    text_y: usize,
    text_w: usize,
    text_h: usize,
    initialized: bool,
    locked: bool,
}

unsafe impl Send for Console {}
unsafe impl Sync for Console {}

const TOP_BAR_HEIGHT: usize = 28;
const PADDING: usize = 20;
const CARD_HEADER: usize = 44;
const LINE_SPACING: usize = 4;

impl Console {
    const fn new() -> Self {
        Self {
            fb_ptr: core::ptr::null_mut(),
            width: 0,
            height: 0,
            pitch: 0,
            bpp: 4,
            col: 0,
            row: 0,
            max_cols: 0,
            max_rows: 0,
            fg_color: palette::TEXT,
            text_x: 0,
            text_y: 0,
            text_w: 0,
            text_h: 0,
            initialized: false,
            locked: false,
        }
    }
    
    pub fn init(&mut self, fb_ptr: *mut u8, width: usize, height: usize, pitch: usize, bpp: usize) {
        self.fb_ptr = fb_ptr;
        self.width = width;
        self.height = height;
        self.pitch = pitch;
        self.bpp = bpp;
        
        self.text_x = PADDING + 16;
        self.text_y = TOP_BAR_HEIGHT + PADDING + CARD_HEADER + 8;
        self.text_w = width - 2 * (PADDING + 16);
        self.text_h = height - TOP_BAR_HEIGHT - 2 * PADDING - CARD_HEADER - 24;
        
        self.max_cols = self.text_w / FONT_WIDTH;
        self.max_rows = self.text_h / (FONT_HEIGHT + LINE_SPACING);
        self.col = 0;
        self.row = 0;
        self.fg_color = palette::TEXT;
        self.initialized = true;
        self.locked = false;
        
        self.redraw_full();
    }
    
    pub fn redraw_full(&self) {
        let ui = UiRenderer::new(self.fb_ptr, self.width, self.height, self.pitch, self.bpp);
        
        ui.draw_background();
        ui.draw_top_bar("Mesa OS");
        
        let card_x = PADDING;
        let card_y = TOP_BAR_HEIGHT + PADDING;
        let card_w = self.width - 2 * PADDING;
        let card_h = self.height - TOP_BAR_HEIGHT - 2 * PADDING;
        
        ui.draw_card(card_x, card_y, card_w, card_h, Some("Terminal"));
    }
    
    pub fn clear(&mut self) {
        if !self.initialized {
            return;
        }
        
        for y in 0..self.text_h {
            for x in 0..self.text_w {
                self.put_pixel(self.text_x + x, self.text_y + y, palette::SURFACE);
            }
        }
        
        self.col = 0;
        self.row = 0;
    }
    
    #[inline]
    fn put_pixel(&self, x: usize, y: usize, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = y * self.pitch + x * self.bpp;
        unsafe {
            let pixel = self.fb_ptr.add(offset);
            *pixel = color.b;
            *pixel.add(1) = color.g;
            *pixel.add(2) = color.r;
            if self.bpp >= 4 {
                *pixel.add(3) = 255;
            }
        }
    }
    
    fn put_char(&mut self, c: char) {
        if !self.initialized || self.locked {
            return;
        }
        
        match c {
            '\n' => {
                self.col = 0;
                self.row += 1;
            }
            '\r' => {
                self.col = 0;
            }
            '\t' => {
                let spaces = 4 - (self.col % 4);
                for _ in 0..spaces {
                    if self.col < self.max_cols {
                        self.col += 1;
                    }
                }
            }
            '\x08' => {
                if self.col > 0 {
                    self.col -= 1;
                } else if self.row > 0 {
                    self.row -= 1;
                    self.col = self.max_cols - 1;
                }
            }
            _ => {
                let glyph = get_char(c);
                let x = self.text_x + self.col * FONT_WIDTH;
                let y = self.text_y + self.row * (FONT_HEIGHT + LINE_SPACING);
                
                if x + FONT_WIDTH <= self.text_x + self.text_w 
                   && y + FONT_HEIGHT <= self.text_y + self.text_h {
                    for row in 0..FONT_HEIGHT {
                        let bits = glyph[row];
                        for col in 0..FONT_WIDTH {
                            let color = if (bits >> (7 - col)) & 1 == 1 {
                                self.fg_color
                            } else {
                                palette::SURFACE
                            };
                            self.put_pixel(x + col, y + row, color);
                        }
                    }
                }
                
                self.col += 1;
            }
        }
        
        if self.col >= self.max_cols {
            self.col = 0;
            self.row += 1;
        }
        
        if self.row >= self.max_rows {
            self.scroll();
        }
    }
    
    fn scroll(&mut self) {
        if !self.initialized {
            return;
        }
        
        let line_height = FONT_HEIGHT + LINE_SPACING;
        
        for row in 1..self.max_rows {
            let src_y = self.text_y + row * line_height;
            let dst_y = self.text_y + (row - 1) * line_height;
            
            // Optimización: Copiar scanlines enteras usando ptr::copy (memmove)
            // Esto es mucho más rápido que leer/escribir pixel a pixel en memoria de video
            for dy in 0..line_height {
                 // Calcular punteros de inicio de línea para el área de texto
                let src_offset = (src_y + dy) * self.pitch + self.text_x * self.bpp;
                let dst_offset = (dst_y + dy) * self.pitch + self.text_x * self.bpp;
                
                unsafe {
                    let src_ptr = self.fb_ptr.add(src_offset);
                    let dst_ptr = self.fb_ptr.add(dst_offset);
                    // Copiamos el ancho del texto en bytes
                    core::ptr::copy(src_ptr, dst_ptr, self.text_w * self.bpp);
                }
            }
        }
        
        let last_y = self.text_y + (self.max_rows - 1) * line_height;
        for dy in 0..line_height {
            for x in 0..self.text_w {
                self.put_pixel(self.text_x + x, last_y + dy, palette::SURFACE);
            }
        }
        
        self.row = self.max_rows - 1;
    }
    
    pub fn write_str(&mut self, s: &str) {
        for c in s.chars() {
            self.put_char(c);
        }
    }
    
    pub fn set_fg_color(&mut self, color: Color) {
        self.fg_color = color;
    }
    
    /// Dibuja/actualiza la status bar con hora real
    pub fn update_status_bar(
        &self, 
        used_mb: u64, 
        total_mb: u64, 
        cpu_count: usize,
        hour: u8,
        minute: u8,
        second: u8,
        disk_used: u64,
        disk_total: u64,
        bat_percent: u8,
        bat_charging: bool,
    ) {
        if !self.initialized {
            return;
        }
        
        let charging_str = if bat_charging { "+" } else { "" };
        // Formato: BAT: XX% | RAM: XX/YYY MB | DISK: AA/BB MB | CPUs: N | HH:MM:SS
        let status_text = format!(
            "BAT: {}{}% | RAM: {}/{} MB | DISK: {}/{} MB | CPUs: {} | {:02}:{:02}:{:02}",
            bat_percent, charging_str, used_mb, total_mb, disk_used, disk_total, cpu_count, hour, minute, second
        );
        
        let text_len = status_text.len();
        let bar_x = self.width - PADDING - 16 - (text_len * FONT_WIDTH);
        let bar_y = TOP_BAR_HEIGHT + PADDING + 14;
        
        // Limpiar área anterior (un poco más ancha)
        let clear_width = (text_len + 5) * FONT_WIDTH;
        for dy in 0..FONT_HEIGHT {
            for dx in 0..clear_width {
                let px = bar_x.saturating_sub(20) + dx;
                if px < self.width && px >= PADDING + 100 {
                    self.put_pixel(px, bar_y + dy, palette::SURFACE);
                }
            }
        }
        
        // Dibujar texto
        for (i, c) in status_text.chars().enumerate() {
            let glyph = get_char(c);
            let char_x = bar_x + i * FONT_WIDTH;
            
            for row in 0..FONT_HEIGHT {
                let bits = glyph[row];
                for col in 0..FONT_WIDTH {
                    if (bits >> (7 - col)) & 1 == 1 {
                        self.put_pixel(char_x + col, bar_y + row, palette::SUBTLE);
                    }
                }
            }
        }
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Console::write_str(self, s);
        Ok(())
    }
}

pub static CONSOLE: Mutex<Console> = Mutex::new(Console::new());

pub fn init(fb_ptr: *mut u8, width: usize, height: usize, pitch: usize, bpp: usize) {
    CONSOLE.lock().init(fb_ptr, width, height, pitch, bpp);
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    
    crate::curr_arch::disable_interrupts();
    let _ = CONSOLE.lock().write_fmt(args);
    crate::curr_arch::enable_interrupts();
}

pub fn clear() {
    CONSOLE.lock().clear();
}

pub fn set_color(color: Color) {
    CONSOLE.lock().set_fg_color(color);
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
    bat_percent: u8,
    bat_charging: bool,
) {
    crate::curr_arch::disable_interrupts();
    CONSOLE.lock().update_status_bar(used_mb, total_mb, cpu_count, hour, minute, second, disk_used, disk_total, bat_percent, bat_charging);
    crate::curr_arch::enable_interrupts();
}

pub fn get_info() -> (*mut u8, usize, usize, usize, usize) {
    let c = CONSOLE.lock();
    (c.fb_ptr, c.width, c.height, c.pitch, c.bpp)
}

pub fn lock() {
    CONSOLE.lock().locked = true;
}

pub fn unlock() {
    CONSOLE.lock().locked = false;
}
pub fn redraw_full() {
    CONSOLE.lock().redraw_full();
}

pub fn write_str_direct(s: &str) {
    CONSOLE.lock().write_str(s);
}
