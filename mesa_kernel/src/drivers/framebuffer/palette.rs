// mesa_kernel/src/drivers/framebuffer/palette.rs

//! Paleta de colores Rosé Pine
//! https://rosepinetheme.com/

// ══════════════════════════════════════════════════════════════════════════════
// BASE COLORS
// ══════════════════════════════════════════════════════════════════════════════

pub const BASE: u32      = 0x191724;
pub const SURFACE: u32   = 0x1f1d2e;
pub const OVERLAY: u32   = 0x26233a;

// ══════════════════════════════════════════════════════════════════════════════
// TEXT COLORS
// ══════════════════════════════════════════════════════════════════════════════

pub const TEXT: u32      = 0xe0def4;
pub const SUBTLE: u32    = 0x908caa;
pub const MUTED: u32     = 0x6e6a86;

// ══════════════════════════════════════════════════════════════════════════════
// ACCENT COLORS
// ══════════════════════════════════════════════════════════════════════════════

pub const LOVE: u32      = 0xeb6f92;  // Rosa/Rojo
pub const GOLD: u32      = 0xf6c177;  // Amarillo/Naranja
pub const ROSE: u32      = 0xebbcba;  // Rosa claro
pub const PINE: u32      = 0x31748f;  // Azul oscuro
pub const FOAM: u32      = 0x9ccfd8;  // Cyan/Turquesa
pub const IRIS: u32      = 0xc4a7e7;  // Púrpura

// ══════════════════════════════════════════════════════════════════════════════
// SEMANTIC COLORS
// ══════════════════════════════════════════════════════════════════════════════

pub const SUCCESS: u32   = 0x9ccf9c;  // Verde
pub const WARNING: u32   = 0xf6c177;  // Igual que GOLD
pub const ERROR: u32     = 0xeb6f92;  // Igual que LOVE
pub const INFO: u32      = 0x9ccfd8;  // Igual que FOAM

// ══════════════════════════════════════════════════════════════════════════════
// HELPERS
// ══════════════════════════════════════════════════════════════════════════════

/// Extrae componentes RGB de un color
pub fn rgb(color: u32) -> (u8, u8, u8) {
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    (r, g, b)
}

/// Crea un color desde componentes RGB
pub fn from_rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Interpola entre dos colores (t: 0.0 - 1.0)
pub fn lerp(color1: u32, color2: u32, t: f32) -> u32 {
    let (r1, g1, b1) = rgb(color1);
    let (r2, g2, b2) = rgb(color2);
    
    let t = t.clamp(0.0, 1.0);
    
    let r = (r1 as f32 + (r2 as f32 - r1 as f32) * t) as u8;
    let g = (g1 as f32 + (g2 as f32 - g1 as f32) * t) as u8;
    let b = (b1 as f32 + (b2 as f32 - b1 as f32) * t) as u8;
    
    from_rgb(r, g, b)
}

/// Oscurece un color
pub fn darken(color: u32, factor: f32) -> u32 {
    let (r, g, b) = rgb(color);
    let factor = (1.0 - factor).clamp(0.0, 1.0);
    from_rgb(
        (r as f32 * factor) as u8,
        (g as f32 * factor) as u8,
        (b as f32 * factor) as u8,
    )
}

/// Aclara un color
pub fn lighten(color: u32, factor: f32) -> u32 {
    let (r, g, b) = rgb(color);
    let factor = factor.clamp(0.0, 1.0);
    from_rgb(
        (r as f32 + (255.0 - r as f32) * factor) as u8,
        (g as f32 + (255.0 - g as f32) * factor) as u8,
        (b as f32 + (255.0 - b as f32) * factor) as u8,
    )
}