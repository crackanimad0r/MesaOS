// mesa_kernel/src/config.rs
use spin::Mutex;
use alloc::string::String;
use crate::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KbdLayout {
    US,
    ES,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    EN,
    ES,
}

pub struct MesaConfig {
    pub kbd_layout: KbdLayout,
    pub tz_offset: i8,
    pub hostname: String,
    pub lang: Lang,
}

impl MesaConfig {
    pub const fn default() -> Self {
        Self {
            kbd_layout: KbdLayout::US,
            tz_offset: 1, // CET por defecto
            hostname: String::new(), // Se llenará en init
            lang: Lang::EN,
        }
    }
}

pub static CONFIG: Mutex<MesaConfig> = Mutex::new(MesaConfig::default());

pub fn init() {
    let mut cfg = CONFIG.lock();
    cfg.hostname = String::from("mesa");
    
    // Intentar cargar desde /etc/mesa.conf
    if let Ok(content) = fs::read_to_string("/etc/mesa.conf") {
        for line in content.lines() {
            let parts: alloc::vec::Vec<&str> = line.split('=').collect();
            if parts.len() == 2 {
                let key = parts[0].trim();
                let val = parts[1].trim();
                
                match key {
                    "kbd" => {
                        if val == "es" { cfg.kbd_layout = KbdLayout::ES; }
                        else { cfg.kbd_layout = KbdLayout::US; }
                    },
                    "tz" => {
                        if let Ok(offset) = val.parse::<i8>() {
                            cfg.tz_offset = offset;
                        }
                    },
                    "host" => {
                        cfg.hostname = String::from(val);
                    },
                    "lang" => {
                        if val == "es" { cfg.lang = Lang::ES; }
                        else { cfg.lang = Lang::EN; }
                    },
                    _ => {}
                }
            }
        }
    }
}

pub fn save() {
    let content = {
        let cfg = CONFIG.lock();
        let mut c = String::new();
        
        c.push_str(&alloc::format!("kbd={}\n", if cfg.kbd_layout == KbdLayout::ES { "es" } else { "us" }));
        c.push_str(&alloc::format!("tz={}\n", cfg.tz_offset));
        c.push_str(&alloc::format!("host={}\n", cfg.hostname));
        c.push_str(&alloc::format!("lang={}\n", if cfg.lang == Lang::ES { "es" } else { "en" }));
        c
    };
    
    // Asegurar que /etc existe
    let _ = fs::mkdir("/etc");
    let _ = fs::write("/etc/mesa.conf", content.as_bytes());
}

pub fn get_hostname() -> String {
    CONFIG.lock().hostname.clone()
}

pub fn set_hostname(name: &str) {
    CONFIG.lock().hostname = String::from(name);
    save();
}

pub fn get_tz_offset() -> i8 {
    CONFIG.lock().tz_offset
}

pub fn set_tz_offset(offset: i8) {
    CONFIG.lock().tz_offset = offset;
    save();
}

pub fn get_kbd_layout() -> KbdLayout {
    CONFIG.lock().kbd_layout
}

pub fn set_kbd_layout(layout: KbdLayout) {
    CONFIG.lock().kbd_layout = layout;
    save();
}

pub fn get_lang() -> Lang {
    CONFIG.lock().lang
}

pub fn set_lang(lang: Lang) {
    CONFIG.lock().lang = lang;
    save();
}
