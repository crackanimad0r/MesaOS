use super::console::Color;
use super::ui::UiRenderer;

/// 16x16 mouse cursor bitmap (1 = foreground, 0 = transparent)
/// Simple arrow pointing up-left
const CURSOR_BITMAP: [[u8; 2]; 16] = [
    [0b10000000, 0b00000000], // █
    [0b11000000, 0b00000000], // ██
    [0b11100000, 0b00000000], // ███
    [0b11110000, 0b00000000], // ████
    [0b11111000, 0b00000000], // █████
    [0b11111100, 0b00000000], // ██████
    [0b11111110, 0b00000000], // ███████
    [0b11111111, 0b00000000], // ████████
    [0b11111111, 0b10000000], // █████████
    [0b11111111, 0b11000000], // ██████████
    [0b11110000, 0b00000000], // ████
    [0b11110000, 0b00000000], // ████
    [0b10110000, 0b00000000], // █ ██
    [0b00110000, 0b00000000], //   ██
    [0b00011000, 0b00000000], //    ██
    [0b00011000, 0b00000000], //    ██
];

const CURSOR_W: usize = 16;
const CURSOR_H: usize = 16;
const CURSOR_FG: Color = Color::new(224, 222, 244);
const CURSOR_OUTLINE: Color = Color::new(25, 23, 36);

pub struct Cursor {
    x: i32,
    y: i32,
    width: usize,
    height: usize,
}

impl Cursor {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            x: (width / 2) as i32,
            y: (height / 2) as i32,
            width,
            height,
        }
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn move_by(&mut self, dx: i32, dy: i32) {
        let new_x = self.x + dx;
        let new_y = self.y + dy;
        self.x = new_x.clamp(0, (self.width - 1) as i32);
        self.y = new_y.clamp(0, (self.height - 1) as i32);
    }

    pub fn draw(&self, ui: &UiRenderer) {
        // Debug: draw a 3x3 red square at cursor hotspot so it's always visible
        for dy in 0..3 {
            for dx in 0..3 {
                let px = self.x as usize + dx;
                let py = self.y as usize + dy;
                if px < self.width && py < self.height {
                    ui.put_pixel(px, py, Color::new(255, 0, 0));
                }
            }
        }
        for row in 0..CURSOR_H {
            let px = self.x as usize;
            let py = self.y as usize + row;
            if py >= self.height {
                continue;
            }

            let bits_hi = CURSOR_BITMAP[row][0];
            let bits_lo = CURSOR_BITMAP[row][1];

            for col in 0..8 {
                let cx = px + col;
                if cx >= self.width {
                    break;
                }
                if (bits_hi >> (7 - col)) & 1 == 1 {
                    // Draw outline (1px shift)
                    if col > 0 && (bits_hi >> (8 - col)) & 1 == 0 {
                        ui.put_pixel(cx - 1, py, CURSOR_OUTLINE);
                    }
                    if row > 0 && (CURSOR_BITMAP[row - 1][0] >> (7 - col)) & 1 == 0 {
                        ui.put_pixel(cx, py - 1, CURSOR_OUTLINE);
                    }
                    ui.put_pixel(cx, py, CURSOR_FG);
                }
            }
            for col in 0..8 {
                let cx = px + 8 + col;
                if cx >= self.width {
                    break;
                }
                if (bits_lo >> (7 - col)) & 1 == 1 {
                    if 8 + col > 0
                        && (if col == 0 {
                            bits_hi >> 7
                        } else {
                            bits_lo >> (8 - col)
                        }) & 1
                            == 0
                    {
                        ui.put_pixel(cx - 1, py, CURSOR_OUTLINE);
                    }
                    ui.put_pixel(cx, py, CURSOR_FG);
                }
            }
        }
    }

    pub fn erase(&self, ui: &UiRenderer) {
        for row in 0..CURSOR_H {
            let py = self.y as usize + row;
            if py >= self.height {
                continue;
            }
            for col in 0..CURSOR_W {
                let cx = self.x as usize + col;
                if cx >= self.width {
                    break;
                }
                ui.put_pixel(cx, py, Color::new(25, 23, 36));
            }
        }
    }
}
