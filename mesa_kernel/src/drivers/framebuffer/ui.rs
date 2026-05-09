use super::console::Color;
use libm::{sqrtf, sinf, cosf};

pub mod palette {
    use super::Color;
    
    pub const BASE: Color = Color::new(25, 23, 36);
    pub const SURFACE: Color = Color::new(31, 29, 46);
    pub const OVERLAY: Color = Color::new(38, 35, 58);
    
    pub const TEXT: Color = Color::new(224, 222, 244);
    pub const SUBTLE: Color = Color::new(144, 140, 170);
    pub const MUTED: Color = Color::new(110, 106, 134);
    
    pub const ROSE: Color = Color::new(235, 188, 186);
    pub const PINE: Color = Color::new(49, 116, 143);
    pub const FOAM: Color = Color::new(156, 207, 216);
    pub const IRIS: Color = Color::new(196, 167, 231);
    pub const GOLD: Color = Color::new(246, 193, 119);
    pub const LOVE: Color = Color::new(235, 111, 146);
    
    pub const SUCCESS: Color = Color::new(156, 207, 156);
    pub const WARNING: Color = Color::new(246, 193, 119);
    pub const ERROR: Color = Color::new(235, 111, 146);
    pub const INFO: Color = Color::new(156, 207, 216);
    
    pub const BAR_BG: Color = Color::new(20, 18, 30);
    pub const BAR_TEXT: Color = Color::new(200, 197, 220);
}

pub struct UiRenderer {
    fb_ptr: *mut u8,
    width: usize,
    height: usize,
    pitch: usize,
    bpp: usize,
}

impl UiRenderer {
    pub fn new(fb_ptr: *mut u8, width: usize, height: usize, pitch: usize, bpp: usize) -> Self {
        Self { fb_ptr, width, height, pitch, bpp }
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
    
    pub fn fill_rect(&self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        for dy in 0..h {
            for dx in 0..w {
                self.put_pixel(x + dx, y + dy, color);
            }
        }
    }
    
    pub fn fill_rounded_rect(&self, x: usize, y: usize, w: usize, h: usize, radius: usize, color: Color) {
        if radius == 0 || w < radius * 2 || h < radius * 2 {
            self.fill_rect(x, y, w, h, color);
            return;
        }
        
        self.fill_rect(x + radius, y, w - 2 * radius, h, color);
        self.fill_rect(x, y + radius, w, h - 2 * radius, color);
        
        self.fill_circle_quarter(x + radius, y + radius, radius, 0, color);
        self.fill_circle_quarter(x + w - radius - 1, y + radius, radius, 1, color);
        self.fill_circle_quarter(x + radius, y + h - radius - 1, radius, 2, color);
        self.fill_circle_quarter(x + w - radius - 1, y + h - radius - 1, radius, 3, color);
    }
    
    fn fill_circle_quarter(&self, cx: usize, cy: usize, r: usize, quarter: u8, color: Color) {
        for dy in 0..=r {
            for dx in 0..=r {
                if dx * dx + dy * dy <= r * r {
                    let (px, py) = match quarter {
                        0 => (cx.saturating_sub(dx), cy.saturating_sub(dy)),
                        1 => (cx + dx, cy.saturating_sub(dy)),
                        2 => (cx.saturating_sub(dx), cy + dy),
                        _ => (cx + dx, cy + dy),
                    };
                    self.put_pixel(px, py, color);
                }
            }
        }
    }
    
    pub fn fill_circle(&self, cx: usize, cy: usize, r: usize, color: Color) {
        for dy in 0..=r * 2 {
            for dx in 0..=r * 2 {
                let dist_x = (dx as i32 - r as i32).unsigned_abs() as usize;
                let dist_y = (dy as i32 - r as i32).unsigned_abs() as usize;
                if dist_x * dist_x + dist_y * dist_y <= r * r {
                    self.put_pixel(cx.saturating_sub(r) + dx, cy.saturating_sub(r) + dy, color);
                }
            }
        }
    }
    
    pub fn gradient_vertical(&self, x: usize, y: usize, w: usize, h: usize, from: Color, to: Color) {
        for dy in 0..h {
            let t = dy as f32 / h as f32;
            let color = Color::new(
                (from.r as f32 + (to.r as f32 - from.r as f32) * t) as u8,
                (from.g as f32 + (to.g as f32 - from.g as f32) * t) as u8,
                (from.b as f32 + (to.b as f32 - from.b as f32) * t) as u8,
            );
            for dx in 0..w {
                self.put_pixel(x + dx, y + dy, color);
            }
        }
    }
    
    pub fn gradient_radial(&self, cx: usize, cy: usize, r: usize, center_color: Color, edge_color: Color) {
        for dy in 0..r * 2 {
            for dx in 0..r * 2 {
                let dist_x = (dx as i32 - r as i32) as f32;
                let dist_y = (dy as i32 - r as i32) as f32;
                let dist = sqrtf(dist_x * dist_x + dist_y * dist_y);
                
                if dist <= r as f32 {
                    let t = dist / r as f32;
                    let color = Color::new(
                        (center_color.r as f32 + (edge_color.r as f32 - center_color.r as f32) * t) as u8,
                        (center_color.g as f32 + (edge_color.g as f32 - center_color.g as f32) * t) as u8,
                        (center_color.b as f32 + (edge_color.b as f32 - center_color.b as f32) * t) as u8,
                    );
                    let px = cx.saturating_sub(r) + dx;
                    let py = cy.saturating_sub(r) + dy;
                    self.put_pixel(px, py, color);
                }
            }
        }
    }
    
    pub fn draw_top_bar(&self, title: &str) {
        let bar_height = 28;
        
        let bar_top = Color::new(35, 32, 52);
        let bar_bottom = Color::new(25, 23, 36);
        self.gradient_vertical(0, 0, self.width, bar_height, bar_top, bar_bottom);
        
        for x in 0..self.width {
            self.put_pixel(x, bar_height - 1, Color::new(45, 42, 65));
        }
        
        let btn_y = bar_height / 2;
        let btn_radius = 6;
        let btn_spacing = 20;
        let btn_start = 12;
        
        self.fill_circle(btn_start, btn_y, btn_radius, Color::new(255, 95, 87));
        self.fill_circle(btn_start + btn_spacing, btn_y, btn_radius, Color::new(255, 189, 46));
        self.fill_circle(btn_start + 2 * btn_spacing, btn_y, btn_radius, Color::new(39, 201, 63));
        
        let title_x = (self.width - title.len() * 8) / 2;
        self.draw_text(title_x, 8, title, palette::BAR_TEXT);
    }
    
    pub fn draw_background(&self) {
        for y in 0..self.height {
            for x in 0..self.width {
                let t = x as f32 / self.width as f32 * 0.3 + y as f32 / self.height as f32 * 0.7;
                
                let r = (palette::BASE.r as f32 + (palette::SURFACE.r as f32 - palette::BASE.r as f32) * t * 0.5) as u8;
                let g = (palette::BASE.g as f32 + (palette::SURFACE.g as f32 - palette::BASE.g as f32) * t * 0.5) as u8;
                let b = (palette::BASE.b as f32 + (palette::SURFACE.b as f32 - palette::BASE.b as f32) * t * 0.5) as u8;
                
                self.put_pixel(x, y, Color::new(r, g, b));
            }
        }
        
        self.gradient_radial(0, 0, 300, Color::new(45, 40, 70), palette::BASE);
        self.gradient_radial(self.width, self.height, 400, Color::new(35, 30, 55), palette::BASE);
    }
    
    pub fn draw_text(&self, x: usize, y: usize, text: &str, color: Color) {
        use super::font::{FONT_WIDTH, FONT_HEIGHT, get_char};
        
        for (i, c) in text.chars().enumerate() {
            let glyph = get_char(c);
            let char_x = x + i * FONT_WIDTH;
            
            for row in 0..FONT_HEIGHT {
                let bits = glyph[row];
                for col in 0..FONT_WIDTH {
                    if (bits >> (7 - col)) & 1 == 1 {
                        self.put_pixel(char_x + col, y + row, color);
                    }
                }
            }
        }
    }
    
    pub fn draw_card(&self, x: usize, y: usize, w: usize, h: usize, title: Option<&str>) {
        for offset in (1..=8).rev() {
            let shadow_color = Color::new(10, 8, 15);
            for dy in offset..h {
                for dx in offset..w {
                    let px = x + dx;
                    let py = y + dy;
                    if px < self.width && py < self.height {
                        self.put_pixel(px, py, shadow_color);
                    }
                }
            }
        }
        
        self.fill_rounded_rect(x, y, w, h, 12, palette::SURFACE);
        
        if let Some(title) = title {
            self.draw_text(x + 16, y + 14, title, palette::TEXT);
            for dx in 12..w.saturating_sub(12) {
                self.put_pixel(x + dx, y + 36, Color::new(50, 47, 75));
            }
        }
    }
}