use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;

const CONFIG_DIR: &str = "/etc/net";
const DEFAULT_CONFIG: &str = "/etc/net.conf";

pub fn save_profile(name: &str, method: &str) -> Result<(), String> {
    if !crate::fs::is_persistent() {
        return Err(String::from("Filesystem not persistent (RamFS)"));
    }
    
    // Ensure directory exists
    let _ = crate::fs::mkdir(CONFIG_DIR);
    
    let path = if name == "default" {
        String::from(DEFAULT_CONFIG)
    } else {
        format!("{}/{}.conf", CONFIG_DIR, name)
    };
    
    let content = match method {
        "dhcp" => String::from("dhcp"),
        "static" => {
            let ip = crate::net::get_ip();
            let mask = crate::net::get_netmask();
            let gw = crate::net::get_gateway();
            format!("static {}.{}.{}.{} {}.{}.{}.{} {}.{}.{}.{}",
                ip[0], ip[1], ip[2], ip[3],
                mask[0], mask[1], mask[2], mask[3],
                gw[0], gw[1], gw[2], gw[3])
        }
        _ => return Err(String::from("Invalid method (use 'dhcp' or 'static')")),
    };
    
    if let Err(e) = crate::fs::write(&path, content.as_bytes()) {
        return Err(format!("Write error: {}", e.as_str()));
    }
    
    Ok(())
}

pub fn load_profile(name: &str) -> Result<(), String> {
    let path = if name == "default" {
        String::from(DEFAULT_CONFIG)
    } else {
        format!("{}/{}.conf", CONFIG_DIR, name)
    };
    
    let config = crate::fs::read_to_string(&path)
        .map_err(|e| format!("Read error: {}", e.as_str()))?;
        
    apply_config(&config)
}

fn apply_config(config: &str) -> Result<(), String> {
    let config = config.trim();
    if config == "dhcp" {
        crate::drivers::framebuffer::set_color(crate::drivers::framebuffer::palette::FOAM);
        crate::mesa_println!("  [NET] Solicitando DHCP...");
        crate::drivers::framebuffer::set_color(crate::drivers::framebuffer::palette::TEXT);
        
        crate::net::dhcp::send_discover()
            .map_err(|e| format!("DHCP error: {}", e))
    } else if config.starts_with("static ") {
        let parts: Vec<&str> = config.split_whitespace().collect();
        if parts.len() == 4 {
            let parse_ip = |s: &str| -> Option<[u8; 4]> {
                let p: Vec<&str> = s.split('.').collect();
                if p.len() != 4 { return None; }
                Some([
                    p[0].parse().ok()?,
                    p[1].parse().ok()?,
                    p[2].parse().ok()?,
                    p[3].parse().ok()?,
                ])
            };
            
            if let (Some(ip), Some(mask), Some(gw)) = (parse_ip(parts[1]), parse_ip(parts[2]), parse_ip(parts[3])) {
                crate::net::configure(ip, mask, gw);
                crate::mesa_println!("  [NET] Config static: {}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
                Ok(())
            } else {
                Err(String::from("Invalid IP format in config"))
            }
        } else {
            Err(String::from("Invalid static config format"))
        }
    } else {
        Err(String::from("Unknown config method"))
    }
}

pub fn list_profiles() -> Vec<String> {
    let mut profiles = Vec::new();
    
    // Check default
    if crate::fs::exists(DEFAULT_CONFIG) {
        profiles.push(String::from("default"));
    }
    
    // Check dir
    if let Ok(entries) = crate::fs::readdir(CONFIG_DIR) {
        for entry in entries {
            if entry.name.ends_with(".conf") {
                profiles.push(entry.name.replace(".conf", ""));
            }
        }
    }
    
    profiles
}

pub fn auto_configure() {
    if let Err(e) = load_profile("default") {
        crate::serial_println!("[NET] No default config: {}", e);
    }
}
