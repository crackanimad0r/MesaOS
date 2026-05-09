use super::ui::{UiRenderer, palette};
use super::console::Color;
use crate::fs;
use alloc::string::String;
use alloc::vec::Vec;

pub fn render_html(path: &str) {
    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            crate::serial_println!("[HTML] Error reading file: {}", e.as_str());
            return;
        }
    };

    // Obtenemos una instancia del renderer. 
    // Como save_state/restore_state no está implementado, simplemente limpiaremos y dibujaremos.
    let (fb_ptr, width, height, pitch, bpp) = super::get_info();
    let ui = UiRenderer::new(fb_ptr, width, height, pitch, bpp);
    
    // Limpiar pantalla
    ui.draw_background();
    ui.draw_top_bar(&alloc::format!("Mesa Browser - {}", path));
    
    let mut x = 20;
    let mut y = 50;
    let line_height = 20;
    
    let mut in_tag = false;
    let mut current_tag = String::new();
    let mut text_color = palette::TEXT;
    let mut is_h1 = false;
    let mut is_bold = false;

    // Parser ultra-simple
    let mut i = 0;
    let chars: Vec<char> = content.chars().collect();
    
    while i < chars.len() {
        let c = chars[i];
        
        if c == '<' {
            in_tag = true;
            current_tag.clear();
        } else if c == '>' {
            in_tag = false;
            let tag = current_tag.to_lowercase();
            
            if tag == "h1" {
                is_h1 = true;
                text_color = palette::IRIS;
                y += 10;
            } else if tag == "/h1" {
                is_h1 = false;
                text_color = palette::TEXT;
                y += line_height + 10;
                x = 20;
            } else if tag == "p" {
                y += line_height;
                x = 20;
            } else if tag == "b" || tag == "strong" {
                is_bold = true;
                text_color = palette::GOLD;
            } else if tag == "/b" || tag == "/strong" {
                is_bold = false;
                text_color = palette::TEXT;
            } else if tag == "br" {
                y += line_height;
                x = 20;
            } else if tag == "hr" {
                y += line_height / 2;
                ui.fill_rect(20, y, width - 40, 2, palette::SUBTLE);
                y += line_height;
                x = 20;
            }
        } else if in_tag {
            current_tag.push(c);
        } else {
            // Render text
            if c == '\n' {
                y += line_height;
                x = 20;
            } else {
                let s = String::from(c);
                ui.draw_text(x, y, &s, text_color);
                x += 8; // Ancho fijo por ahora
                
                if x > width - 40 {
                    x = 20;
                    y += line_height;
                }
            }
        }
        i += 1;
    }
    
    // Esperar una tecla
    crate::serial_println!("[HTML] Render finished. Press any key to return.");
    // En una implementación real aquí esperaríamos a que el usuario presione algo
}
