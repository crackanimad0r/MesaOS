// mesa_kernel/src/drivers/keyboard/scancode.rs

use super::{KeyEvent, SpecialKey};
use core::sync::atomic::{AtomicBool, Ordering};

static EXTENDED_MODE: AtomicBool = AtomicBool::new(false);
static SHIFT_PRESSED: AtomicBool = AtomicBool::new(false);
static CAPS_LOCK: AtomicBool = AtomicBool::new(false);
static CTRL_PRESSED: AtomicBool = AtomicBool::new(false);
static ALT_PRESSED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    EXTENDED_MODE.store(false, Ordering::Relaxed);
    SHIFT_PRESSED.store(false, Ordering::Relaxed);
    CAPS_LOCK.store(false, Ordering::Relaxed);
    CTRL_PRESSED.store(false, Ordering::Relaxed);
    ALT_PRESSED.store(false, Ordering::Relaxed);
}

pub fn decode(scancode: u8) -> Option<KeyEvent> {
    if scancode == 0xE0 {
        EXTENDED_MODE.store(true, Ordering::Relaxed);
        return None;
    }
    
    let is_extended = EXTENDED_MODE.load(Ordering::Relaxed);
    EXTENDED_MODE.store(false, Ordering::Relaxed);
    
    let is_release = scancode & 0x80 != 0;
    let code = scancode & 0x7F;
    
    if is_extended {
        return decode_extended(code, is_release);
    }
    
    match code {
        0x2A | 0x36 => {
            SHIFT_PRESSED.store(!is_release, Ordering::Relaxed);
            return None;
        }
        0x1D => {
            CTRL_PRESSED.store(!is_release, Ordering::Relaxed);
            return None;
        }
        0x38 => {
            ALT_PRESSED.store(!is_release, Ordering::Relaxed);
            return None;
        }
        0x3A if !is_release => {
            let current = CAPS_LOCK.load(Ordering::Relaxed);
            CAPS_LOCK.store(!current, Ordering::Relaxed);
            return None;
        }
        _ => {}
    }
    
    if is_release {
        return None;
    }
    
    let layout = crate::config::get_kbd_layout();
    if layout == crate::config::KbdLayout::ES {
        decode_es(code)
    } else {
        decode_us(code)
    }
}

fn decode_extended(code: u8, is_release: bool) -> Option<KeyEvent> {
    if is_release {
        return None;
    }
    
    match code {
        0x48 => Some(KeyEvent::Special(SpecialKey::ArrowUp)),
        0x50 => Some(KeyEvent::Special(SpecialKey::ArrowDown)),
        0x4B => Some(KeyEvent::Special(SpecialKey::ArrowLeft)),
        0x4D => Some(KeyEvent::Special(SpecialKey::ArrowRight)),
        0x47 => Some(KeyEvent::Special(SpecialKey::Home)),
        0x4F => Some(KeyEvent::Special(SpecialKey::End)),
        0x49 => Some(KeyEvent::Special(SpecialKey::PageUp)),
        0x51 => Some(KeyEvent::Special(SpecialKey::PageDown)),
        0x52 => Some(KeyEvent::Special(SpecialKey::Insert)),
        0x53 => Some(KeyEvent::Special(SpecialKey::Delete)),
        _ => None,
    }
}

fn decode_us(code: u8) -> Option<KeyEvent> {
    let shift = SHIFT_PRESSED.load(Ordering::Relaxed);
    let caps = CAPS_LOCK.load(Ordering::Relaxed);
    let ctrl = CTRL_PRESSED.load(Ordering::Relaxed);
    let upper = shift ^ caps;
    
    // Ctrl+C → SIGINT
    if code == 0x2E && ctrl {
        return Some(KeyEvent::Special(SpecialKey::CtrlC));
    }
    // Ctrl+S → Save
    if code == 0x1F && ctrl {
        return Some(KeyEvent::Special(SpecialKey::CtrlS));
    }
    // Ctrl+X → Exit
    if code == 0x2D && ctrl {
        return Some(KeyEvent::Special(SpecialKey::CtrlX));
    }
    
    match code {
        0x01 => Some(KeyEvent::Special(SpecialKey::Escape)),
        0x0E => Some(KeyEvent::Special(SpecialKey::Backspace)),
        0x0F => Some(KeyEvent::Special(SpecialKey::Tab)),
        0x1C => Some(KeyEvent::Special(SpecialKey::Enter)),
        
        0x3B => Some(KeyEvent::Special(SpecialKey::F1)),
        0x3C => Some(KeyEvent::Special(SpecialKey::F2)),
        0x3D => Some(KeyEvent::Special(SpecialKey::F3)),
        0x3E => Some(KeyEvent::Special(SpecialKey::F4)),
        0x3F => Some(KeyEvent::Special(SpecialKey::F5)),
        0x40 => Some(KeyEvent::Special(SpecialKey::F6)),
        0x41 => Some(KeyEvent::Special(SpecialKey::F7)),
        0x42 => Some(KeyEvent::Special(SpecialKey::F8)),
        0x43 => Some(KeyEvent::Special(SpecialKey::F9)),
        0x44 => Some(KeyEvent::Special(SpecialKey::F10)),
        0x57 => Some(KeyEvent::Special(SpecialKey::F11)),
        0x58 => Some(KeyEvent::Special(SpecialKey::F12)),
        
        0x02 => Some(KeyEvent::Char(if shift { '!' } else { '1' })),
        0x03 => Some(KeyEvent::Char(if shift { '@' } else { '2' })),
        0x04 => Some(KeyEvent::Char(if shift { '#' } else { '3' })),
        0x05 => Some(KeyEvent::Char(if shift { '$' } else { '4' })),
        0x06 => Some(KeyEvent::Char(if shift { '%' } else { '5' })),
        0x07 => Some(KeyEvent::Char(if shift { '^' } else { '6' })),
        0x08 => Some(KeyEvent::Char(if shift { '&' } else { '7' })),
        0x09 => Some(KeyEvent::Char(if shift { '*' } else { '8' })),
        0x0A => Some(KeyEvent::Char(if shift { '(' } else { '9' })),
        0x0B => Some(KeyEvent::Char(if shift { ')' } else { '0' })),
        0x0C => Some(KeyEvent::Char(if shift { '_' } else { '-' })),
        0x0D => Some(KeyEvent::Char(if shift { '+' } else { '=' })),
        
        0x10 => Some(KeyEvent::Char(if upper { 'Q' } else { 'q' })),
        0x11 => Some(KeyEvent::Char(if upper { 'W' } else { 'w' })),
        0x12 => Some(KeyEvent::Char(if upper { 'E' } else { 'e' })),
        0x13 => Some(KeyEvent::Char(if upper { 'R' } else { 'r' })),
        0x14 => Some(KeyEvent::Char(if upper { 'T' } else { 't' })),
        0x15 => Some(KeyEvent::Char(if upper { 'Y' } else { 'y' })),
        0x16 => Some(KeyEvent::Char(if upper { 'U' } else { 'u' })),
        0x17 => Some(KeyEvent::Char(if upper { 'I' } else { 'i' })),
        0x18 => Some(KeyEvent::Char(if upper { 'O' } else { 'o' })),
        0x19 => Some(KeyEvent::Char(if upper { 'P' } else { 'p' })),
        0x1A => Some(KeyEvent::Char(if shift { '{' } else { '[' })),
        0x1B => Some(KeyEvent::Char(if shift { '}' } else { ']' })),
        0x2B => Some(KeyEvent::Char(if shift { '|' } else { '\\' })),
        
        0x1E => Some(KeyEvent::Char(if upper { 'A' } else { 'a' })),
        0x1F => Some(KeyEvent::Char(if upper { 'S' } else { 's' })),
        0x20 => Some(KeyEvent::Char(if upper { 'D' } else { 'd' })),
        0x21 => Some(KeyEvent::Char(if upper { 'F' } else { 'f' })),
        0x22 => Some(KeyEvent::Char(if upper { 'G' } else { 'g' })),
        0x23 => Some(KeyEvent::Char(if upper { 'H' } else { 'h' })),
        0x24 => Some(KeyEvent::Char(if upper { 'J' } else { 'j' })),
        0x25 => Some(KeyEvent::Char(if upper { 'K' } else { 'k' })),
        0x26 => Some(KeyEvent::Char(if upper { 'L' } else { 'l' })),
        0x27 => Some(KeyEvent::Char(if shift { ':' } else { ';' })),
        0x28 => Some(KeyEvent::Char(if shift { '"' } else { '\'' })),
        0x29 => Some(KeyEvent::Char(if shift { '~' } else { '`' })),
        
        0x2C => Some(KeyEvent::Char(if upper { 'Z' } else { 'z' })),
        0x2D => Some(KeyEvent::Char(if upper { 'X' } else { 'x' })),
        0x2E => Some(KeyEvent::Char(if upper { 'C' } else { 'c' })),
        0x2F => Some(KeyEvent::Char(if upper { 'V' } else { 'v' })),
        0x30 => Some(KeyEvent::Char(if upper { 'B' } else { 'b' })),
        0x31 => Some(KeyEvent::Char(if upper { 'N' } else { 'n' })),
        0x32 => Some(KeyEvent::Char(if upper { 'M' } else { 'm' })),
        0x33 => Some(KeyEvent::Char(if shift { '<' } else { ',' })),
        0x34 => Some(KeyEvent::Char(if shift { '>' } else { '.' })),
        0x35 => Some(KeyEvent::Char(if shift { '?' } else { '/' })),
        
        0x39 => Some(KeyEvent::Char(' ')),
        
        _ => None,
    }
}

fn decode_es(code: u8) -> Option<KeyEvent> {
    let shift = SHIFT_PRESSED.load(Ordering::Relaxed);
    let caps = CAPS_LOCK.load(Ordering::Relaxed);
    let ctrl = CTRL_PRESSED.load(Ordering::Relaxed);
    let upper = shift ^ caps;
    
    // Ctrl+C, Ctrl+S, Ctrl+X
    if code == 0x2E && ctrl { return Some(KeyEvent::Special(SpecialKey::CtrlC)); }
    if code == 0x1F && ctrl { return Some(KeyEvent::Special(SpecialKey::CtrlS)); }
    if code == 0x2D && ctrl { return Some(KeyEvent::Special(SpecialKey::CtrlX)); }
    
    match code {
        0x01 => Some(KeyEvent::Special(SpecialKey::Escape)),
        0x0E => Some(KeyEvent::Special(SpecialKey::Backspace)),
        0x0F => Some(KeyEvent::Special(SpecialKey::Tab)),
        0x1C => Some(KeyEvent::Special(SpecialKey::Enter)),
        
        0x02 => Some(KeyEvent::Char(if shift { '!' } else { '1' })),
        0x03 => Some(KeyEvent::Char(if shift { '"' } else { '2' })),
        0x04 => Some(KeyEvent::Char(if shift { '·' } else { '3' })),
        0x05 => Some(KeyEvent::Char(if shift { '$' } else { '4' })),
        0x06 => Some(KeyEvent::Char(if shift { '%' } else { '5' })),
        0x07 => Some(KeyEvent::Char(if shift { '&' } else { '6' })),
        0x08 => Some(KeyEvent::Char(if shift { '/' } else { '7' })),
        0x09 => Some(KeyEvent::Char(if shift { '(' } else { '8' })),
        0x0A => Some(KeyEvent::Char(if shift { ')' } else { '9' })),
        0x0B => Some(KeyEvent::Char(if shift { '=' } else { '0' })),
        0x0C => Some(KeyEvent::Char(if shift { '?' } else { '\'' })),
        0x0D => Some(KeyEvent::Char(if shift { '¿' } else { '¡' })),
        
        0x10 => Some(KeyEvent::Char(if upper { 'Q' } else { 'q' })),
        0x11 => Some(KeyEvent::Char(if upper { 'W' } else { 'w' })),
        0x12 => Some(KeyEvent::Char(if upper { 'E' } else { 'e' })),
        0x13 => Some(KeyEvent::Char(if upper { 'R' } else { 'r' })),
        0x14 => Some(KeyEvent::Char(if upper { 'T' } else { 't' })),
        0x15 => Some(KeyEvent::Char(if upper { 'Y' } else { 'y' })),
        0x16 => Some(KeyEvent::Char(if upper { 'U' } else { 'u' })),
        0x17 => Some(KeyEvent::Char(if upper { 'I' } else { 'i' })),
        0x18 => Some(KeyEvent::Char(if upper { 'O' } else { 'o' })),
        0x19 => Some(KeyEvent::Char(if upper { 'P' } else { 'p' })),
        0x1A => Some(KeyEvent::Char(if shift { '^' } else { '`' })),
        0x1B => Some(KeyEvent::Char(if shift { '*' } else { '+' })),
        0x2B => Some(KeyEvent::Char(if shift { 'Ç' } else { 'ç' })),
        
        0x1E => Some(KeyEvent::Char(if upper { 'A' } else { 'a' })),
        0x1F => Some(KeyEvent::Char(if upper { 'S' } else { 's' })),
        0x20 => Some(KeyEvent::Char(if upper { 'D' } else { 'd' })),
        0x21 => Some(KeyEvent::Char(if upper { 'F' } else { 'f' })),
        0x22 => Some(KeyEvent::Char(if upper { 'G' } else { 'g' })),
        0x23 => Some(KeyEvent::Char(if upper { 'H' } else { 'h' })),
        0x24 => Some(KeyEvent::Char(if upper { 'J' } else { 'j' })),
        0x25 => Some(KeyEvent::Char(if upper { 'K' } else { 'k' })),
        0x26 => Some(KeyEvent::Char(if upper { 'L' } else { 'l' })),
        0x27 => Some(KeyEvent::Char(if upper { 'Ñ' } else { 'ñ' })),
        0x28 => Some(KeyEvent::Char(if shift { '¨' } else { '´' })),
        0x29 => Some(KeyEvent::Char(if shift { 'ª' } else { 'º' })),
        
        0x2C => Some(KeyEvent::Char(if upper { 'Z' } else { 'z' })),
        0x2D => Some(KeyEvent::Char(if upper { 'X' } else { 'x' })),
        0x2E => Some(KeyEvent::Char(if upper { 'C' } else { 'c' })),
        0x2F => Some(KeyEvent::Char(if upper { 'V' } else { 'v' })),
        0x30 => Some(KeyEvent::Char(if upper { 'B' } else { 'b' })),
        0x31 => Some(KeyEvent::Char(if upper { 'N' } else { 'n' })),
        0x32 => Some(KeyEvent::Char(if upper { 'M' } else { 'm' })),
        0x33 => Some(KeyEvent::Char(if shift { ';' } else { ',' })),
        0x34 => Some(KeyEvent::Char(if shift { ':' } else { '.' })),
        0x35 => Some(KeyEvent::Char(if shift { '_' } else { '-' })),
        
        0x39 => Some(KeyEvent::Char(' ')),
        _ => None,
    }
}

pub fn is_ctrl_pressed() -> bool {
    CTRL_PRESSED.load(Ordering::Relaxed)
}

pub fn is_alt_pressed() -> bool {
    ALT_PRESSED.load(Ordering::Relaxed)
}

pub fn is_shift_pressed() -> bool {
    SHIFT_PRESSED.load(Ordering::Relaxed)
}