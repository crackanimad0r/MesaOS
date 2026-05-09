//! Shell de usuario en Ring 3

use crate::scheduler::TaskId;

/// Lanza el shell de usuario en Ring 3
pub fn launch_userland_shell() -> Result<TaskId, &'static str> {
    crate::serial_println!("[SHELL] Launching userland shell in Ring 3...");
    
    if let Some(shell_code) = super::programs::get_program("shell") {
        match crate::scheduler::spawn_user("shell", shell_code) {
            Ok(id) => {
                crate::klog_info!("Userland shell started (PID {})", id);
                Ok(id)
            }
            Err(e) => {
                crate::serial_println!("[SHELL] Failed to launch shell: {}", e);
                Err(e)
            }
        }
    } else {
        Err("Shell program not found")
    }
}

/// Muestra información sobre el shell
pub fn shell_info() {
    use crate::drivers::framebuffer::ui::palette;
    
    crate::drivers::framebuffer::set_color(palette::FOAM);
    crate::mesa_println!("\n╭─────────────────────────────────────╮");
    crate::mesa_println!("│     Mesa Shell v1.0 (Ring 3)       │");
    crate::mesa_println!("├─────────────────────────────────────┤");
    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_println!("│  Comandos disponibles:              │");
    crate::mesa_println!("│    help   - Muestra ayuda           │");
    crate::mesa_println!("│    ls     - Lista archivos          │");
    crate::mesa_println!("│    echo   - Imprime texto           │");
    crate::mesa_println!("│    hello  - Hello World             │");
    crate::mesa_println!("│    counter- Contador demo           │");
    crate::mesa_println!("│    exit   - Salir del shell         │");
    crate::drivers::framebuffer::set_color(palette::FOAM);
    crate::mesa_println!("╰─────────────────────────────────────╯\n");
    crate::drivers::framebuffer::set_color(palette::TEXT);
}