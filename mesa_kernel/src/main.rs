#![no_std]
#![no_main]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![feature(alloc_error_handler)]

extern crate alloc;

#[cfg(target_arch = "x86_64")]
mod acpi;
mod arch;
mod drivers;
mod log;
mod memory;
mod pci;
mod scheduler;
#[macro_use]
mod linux;
mod audio_player;
mod config;
mod elf;
mod fs;
mod linux_compat;
#[cfg(target_arch = "x86_64")]
mod net;
mod pipe;
mod security;
mod shim;
mod smp;
mod syscall;
mod usb;
mod userland;
mod users;

// Architecture alias for simplicity
#[cfg(target_arch = "aarch64")]
pub use arch::aarch64 as curr_arch;
#[cfg(target_arch = "x86_64")]
pub use arch::x86_64 as curr_arch;

pub use curr_arch::limine_req;

use alloc::{format, string::String, vec::Vec};
use core::panic::PanicInfo;
use drivers::framebuffer::ui::palette;
use drivers::framebuffer::Color;
use drivers::keyboard::{KeyEvent, SpecialKey};
use limine::memory_map::EntryType;
use spin::Mutex;

/// Redirección de salida a pipe (para cmd1 | cmd2)
static PIPE_STDOUT: Mutex<Option<i32>> = Mutex::new(None);
/// Redirección de entrada desde pipe (para cmd1 | cmd2)
static PIPE_STDIN: Mutex<Option<i32>> = Mutex::new(None);

pub const OS_VERSION: &str = "v129-TASKLET";

const COMMANDS: &[&str] = &[
    "help",
    "clear",
    "info",
    "mem",
    "echo",
    "time",
    "uptime",
    "date",
    "neofetch",
    "panic",
    "test",
    "history",
    "reboot",
    "halt",
    "shutdown",
    "cpuinfo",
    "colors",
    "logo",
    "dmesg",
    "hexdump",
    "lspci",
    "vmm",
    "tasks",
    "acpi",
    "hda",
    "spawn",
    "kill",
    "ps",
    "yield",
    "sched",
    "whoami",
    "id",
    "users",
    "su",
    "logout",
    "ring3",
    "passwd",
    "exec",
    "userland", // AGREGAR
    "ls",
    "cd",
    "pwd",
    "cat",
    "mkdir",
    "touch",
    "rm",
    "rmdir",
    "mv",
    "tree",
    "write", // Escribir contenido a archivo
    "beep",
    "ifconfig",
    "ping",
    "curl",
    "arp",
    "net",
    "usb",
    "usbstatus",
    "usb_net",
    "ip",
    "nano",
    "config",
    "useradd",
    "userdel",
    "chmod",
    "run",
    // UEFI NVRAM commands
    "nvram_set",
    "nvram_get",
    "nvram_del",
    "nvram_list",
    "nvram_test",
    // Battery report (estilo Linux: upower -i / acpi -V)
    "battery-report",
    // Audio player (WAV/MP3 info, play, etc.)
    "play",
    "audio-info",
    "audio-list",
];

// ══════════════════════════════════════════════════════════════════════════════
// LOCALIZACIÓN
// ══════════════════════════════════════════════════════════════════════════════

fn t(es: &'static str, en: &'static str) -> &'static str {
    if config::get_lang() == config::Lang::ES {
        es
    } else {
        en
    }
}

fn cmd_config(args: &[&str]) {
    if args.is_empty() {
        mesa_println!(
            "{}",
            t(
                "Uso: config <opcion> <valor>",
                "Usage: config <option> <value>"
            )
        );
        mesa_println!("{}", t("Opciones:", "Options:"));
        mesa_println!(
            "  kbd  <es|us>     - {}",
            t("Cambiar teclado", "Change keyboard")
        );
        mesa_println!(
            "  tz   <offset>    - {}",
            t("Zona horaria (ej: +2, -5)", "Timezone (eg: +2, -5)")
        );
        mesa_println!(
            "  host <nombre>    - {}",
            t("Cambiar hostname", "Change hostname")
        );
        mesa_println!(
            "  lang <es|en>     - {}",
            t("Cambiar idioma", "Change language")
        );
        return;
    }

    let opt = args[0];
    if args.len() < 2 {
        mesa_println!("Falta el valor para '{}'", opt);
        return;
    }
    let val = args[1];

    match opt {
        "kbd" => {
            if val == "es" {
                config::set_kbd_layout(config::KbdLayout::ES);
                mesa_println!("[CONFIG] Teclado cambiado a ESPAÑOL");
            } else if val == "us" {
                config::set_kbd_layout(config::KbdLayout::US);
                mesa_println!("[CONFIG] Teclado cambiado a US (Inglés)");
            } else {
                mesa_println!("Layout no reconocido: {}", val);
            }
        }
        "tz" => {
            if let Ok(offset) = val.parse::<i8>() {
                config::set_tz_offset(offset);
                mesa_println!("[CONFIG] Zona horaria ajustada a UTC{:+.0}", offset);
            } else {
                mesa_println!("Offset inválido: {}", val);
            }
        }
        "host" => {
            config::set_hostname(val);
            mesa_println!("[CONFIG] Hostname cambiado a: {}", val);
        }
        "lang" => {
            if val == "es" {
                config::set_lang(config::Lang::ES);
                mesa_println!("[CONFIG] Idioma cambiado a ESPAÑOL");
            } else if val == "en" {
                config::set_lang(config::Lang::EN);
                mesa_println!("[CONFIG] Language changed to ENGLISH");
            } else {
                mesa_println!("Idioma no soportado: {}", val);
            }
        }
        _ => mesa_println!("Opción desconocida: {}", opt),
    }
}

#[no_mangle]
extern "C" fn kernel_start() -> ! {
    #[cfg(target_arch = "x86_64")]
    {
        if !arch::x86_64::limine_req::is_supported() {
            loop {
                core::hint::spin_loop();
            }
        }
    }

    drivers::init_serial();
    serial_println!("[BOOT] Mesa OS iniciando...");

    if let Some(fb_response) = limine_req::framebuffer_response() {
        if let Some(fb) = fb_response.framebuffers().next() {
            drivers::init_framebuffer(
                fb.addr(),
                fb.width() as usize,
                fb.height() as usize,
                fb.pitch() as usize,
                (fb.bpp() / 8) as usize,
            );
            serial_println!("[BOOT] Framebuffer inicializado");
        }
    }

    drivers::framebuffer::set_color(palette::IRIS);
    mesa_println!("Mesa OS v0.1.0-{}", OS_VERSION);
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("\"Minimalismo con Libertad\"\n");

    print_system_info();

    // Detección específica HP Laptop 15s-eq2xxx
    #[cfg(target_arch = "x86_64")]
    detect_hp_laptop_15s();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("[CPU]");
    drivers::framebuffer::set_color(palette::TEXT);

    #[cfg(target_arch = "x86_64")]
    {
        serial_println!("[BOOT] Inicializando arquitectura x86_64...");
        arch::x86_64::init();
        print_ok("GDT + IDT");
    }

    #[cfg(target_arch = "aarch64")]
    {
        serial_println!("[BOOT] Inicializando arquitectura aarch64 (RPi)...");
        arch::aarch64::init();
        print_ok("Exceptions/MMU Stub");
    }

    // No inicializamos PIC aquí, lo haremos con APIC después de ACPI

    serial_println!("[BOOT] Inicializando teclado...");
    drivers::init_keyboard();
    print_ok("Keyboard");

    serial_println!("[BOOT] Inicializando RTC...");
    drivers::init_rtc();
    print_ok("RTC");

    serial_println!("[BOOT] Inicializando Batería...");
    drivers::init_battery();
    print_ok("Battery Manager");

    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("[Memoria]");
    drivers::framebuffer::set_color(palette::TEXT);

    serial_println!("[BOOT] Inicializando sistema de memoria...");
    let mem_info = match memory::init() {
        Ok(info) => {
            print_ok(&format!(
                "PMM+VMM (~{} MB)",
                info.usable_memory / 1024 / 1024
            ));
            print_ok("Heap");
            info
        }
        Err(e) => {
            print_err(e);
            serial_println!("[ERROR] Fallo al inicializar memoria: {}", e);
            halt();
        }
    };
    mesa_println!();

    // Initialize UEFI NVRAM for persistent storage
    #[cfg(target_arch = "x86_64")]
    {
        serial_println!("[BOOT] Inicializando UEFI NVRAM...");
        if let Some(rt_services) = arch::x86_64::limine_req::uefi_runtime_services() {
            unsafe {
                arch::x86_64::uefi_nvram::init(rt_services as *mut core::ffi::c_void);
            }
            print_ok("UEFI NVRAM");
            klog_info!("UEFI NVRAM initialized");
        } else {
            drivers::framebuffer::set_color(palette::GOLD);
            mesa_print!("  [--] ");
            drivers::framebuffer::set_color(palette::TEXT);
            mesa_println!("UEFI NVRAM: No disponible (no UEFI boot o sin RuntimeServices)");
            klog_warn!("UEFI NVRAM not available");
        }
    }

    log::init();
    klog_info!("Mesa OS v0.1.0 booted");
    klog_info!("Usable memory ~{} MB", mem_info.usable_memory / 1024 / 1024);

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("[Hardware]");
    drivers::framebuffer::set_color(palette::TEXT);

    #[cfg(target_arch = "x86_64")]
    {
        serial_println!("[BOOT] Parseando tablas ACPI...");
        match acpi::init() {
            Ok(()) => {
                print_ok("ACPI");
                klog_info!("ACPI initialized");
            }
            Err(e) => {
                drivers::framebuffer::set_color(palette::GOLD);
                mesa_print!("  [--] ");
                drivers::framebuffer::set_color(palette::TEXT);
                mesa_println!("ACPI: {}", e);
                klog_warn!("ACPI init failed: {}", e);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        serial_println!("[BOOT] Inicializando sistema de interrupciones (APIC)...");
        unsafe {
            match arch::x86_64::interrupts::apic::init_apic() {
                Ok(()) => {
                    print_ok("APIC (Modern Mode)");
                    klog_info!("APIC initialized");
                }
                Err(e) => {
                    serial_println!("[BOOT] APIC Falló, usando PIC como fallback: {}", e);
                    arch::x86_64::interrupts::init_pic();
                    print_ok("PIC (Legacy Mode)");
                    klog_warn!("Fallback to PIC: {}", e);
                }
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        serial_println!("[BOOT] Escaneando bus PCI...");
        pci::init();
        print_ok(&format!("PCI ({} dispositivos)", pci::device_count()));

        // Inicializar touchpad (necesita PCI para encontrar SMBus)
        let (_, fb_w, fb_h, _, _) = drivers::framebuffer::get_info();
        drivers::touchpad::init(fb_w as u16, fb_h as u16);

        drivers::usb::init();

        serial_println!("[BOOT] Inicializando red...");
        drivers::net::init();
        let nic_name = if net::is_virtio() {
            "VirtIO-Net"
        } else {
            "RTL8139"
        };
        print_ok(&format!("Networking ({})", nic_name));
    }

    #[cfg(target_arch = "x86_64")]
    {
        // Initialize network stack
        net::init();
        // Default QEMU user network config
        net::configure([10, 0, 2, 15], [255, 255, 255, 0], [10, 0, 2, 2]);
        print_ok("IP: 10.0.2.15/24");

        serial_println!("[BOOT] Inicializando audio...");
        drivers::init_audio();
        print_ok("Audio (HDA + PC Speaker)");
    }

    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("[Kernel]");
    drivers::framebuffer::set_color(palette::TEXT);

    serial_println!("[BOOT] Inicializando scheduler...");
    scheduler::init();
    print_ok("Scheduler");

    #[cfg(target_arch = "x86_64")]
    {
        serial_println!("[BOOT] Inicializando syscalls...");
        syscall::init();
        print_ok("Syscalls");

        serial_println!("[BOOT] Inicializando SMP (multi-CPU)...");
        smp::init();
        print_ok("SMP");
    }

    let disk_available = false;

    serial_println!("[BOOT] Inicializando filesystem...");
    match fs::init() {
        fs::InitResult::RamFs => {
            drivers::framebuffer::set_color(palette::SUCCESS);
            mesa_print!("  [OK] ");
            drivers::framebuffer::set_color(palette::TEXT);
            mesa_println!("Filesystem (RamFS - volátil)");
        }
    }

    serial_println!("[BOOT] Inicializando sistema de usuarios...");
    users::init();
    print_ok("Users");

    config::init();
    print_ok("Config");

    serial_println!("[BOOT] Inicializando módulo de seguridad...");
    security::init();
    print_ok("Security");

    // Registrar evento de auditoría del arranque
    security::audit_log(
        security::AuditSeverity::Info,
        "Kernel boot completed successfully",
    );

    // Inicializar capa de compatibilidad Linux
    linux_compat::init();

    // Inicializar capa de drivers Linux (shim)
    shim::init();

    serial_println!("[BOOT] Habilitando interrupciones...");
    curr_arch::enable_interrupts();
    klog_info!("Interrupts enabled");

    // Start xHCI polling kernel task (needs interrupts enabled for sleep to work)
    // shim::start_xhci_polling(); // DISABLED — USB driver causing issues

    drivers::framebuffer::set_color(palette::SUCCESS);
    mesa_println!("Sistema listo.\n");
    drivers::framebuffer::set_color(palette::TEXT);

    serial_println!("[BOOT] Mesa OS completamente inicializado");

    // Pantalla de login
    login_screen();

    // Auto-configurar red si hay perfil guardado
    #[cfg(target_arch = "x86_64")]
    net::config::auto_configure();

    serial_println!("[BOOT] Iniciando Shell Loop...");
    // Shell principal
    shell_loop();
}

// ══════════════════════════════════════════════════════════════════════════════
// PANTALLA DE LOGIN
// ══════════════════════════════════════════════════════════════════════════════

fn login_screen() {
    loop {
        drivers::framebuffer::clear();
        update_status();

        // Logo
        mesa_println!();
        mesa_println!();
        drivers::framebuffer::set_color(palette::IRIS);
        mesa_println!("      ##m   m##  ######  ######  #####");
        mesa_println!("      ###m m###  #       #       #   #");
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_println!("      ## ### ##  ####    #####   #####");
        mesa_println!("      ## '#' ##  #           #   #   #");
        drivers::framebuffer::set_color(palette::GOLD);
        mesa_println!("      ##     ##  ######  #####   #   #");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();

        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("                O P E R A T I N G   S Y S T E M");
        mesa_println!("                       Version 0.1.0");
        mesa_println!();
        drivers::framebuffer::set_color(palette::TEXT);

        // Caja de login
        mesa_println!();
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_println!("      ┌─────────────────────────────────┐");
        mesa_println!("      │            L O G I N            │");
        mesa_println!("      └─────────────────────────────────┘");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();

        // Usuarios disponibles
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("      {}", t("Usuarios disponibles:", "Available users:"));
        mesa_println!("        • root  ({})", t("sin contraseña", "no password"));
        mesa_println!("        • guest ({}: guest)", t("contraseña", "password"));
        mesa_println!("        • mesa  ({}: mesa)", t("contraseña", "password"));
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();

        // Input de usuario
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_print!("      {}: ", t("Usuario", "User"));
        drivers::framebuffer::set_color(palette::TEXT);

        let mut username = String::new();
        serial_println!("[LOGIN] Esperando nombre de usuario...");
        read_line_simple(&mut username);
        let username = username.trim();
        serial_println!("[LOGIN] Usuario introducido: {}", username);

        if username.is_empty() {
            continue;
        }

        // Input de contraseña
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_print!("      {}: ", t("Contraseña", "Password"));
        drivers::framebuffer::set_color(palette::TEXT);

        let mut password = String::new();
        read_line_password(&mut password);
        let password = password.trim();

        mesa_println!();

        // Intentar login
        match users::login(username, password) {
            Ok(()) => {
                drivers::framebuffer::set_color(palette::SUCCESS);
                mesa_println!();
                mesa_println!("      ¡{}, {}!", t("Bienvenido", "Welcome"), username);

                klog_info!("User '{}' logged in successfully", username);

                for _ in 0..10000000 {
                    core::hint::spin_loop();
                }

                drivers::framebuffer::clear();
                return;
            }
            Err(e) => {
                drivers::framebuffer::set_color(palette::ERROR);
                mesa_println!();
                mesa_println!("      Error: {}", e);
                drivers::framebuffer::set_color(palette::TEXT);
                mesa_println!();
                mesa_println!("      Presiona Enter para reintentar...");

                klog_warn!("Failed login attempt for user '{}'", username);

                let mut dummy = String::new();
                read_line_simple(&mut dummy);
            }
        }
    }
}

fn read_line_simple(buffer: &mut String) {
    loop {
        crate::scheduler::yield_now();
        curr_arch::halt();

        while let Some(event) = drivers::keyboard::read_event() {
            match event {
                KeyEvent::Char(c) => {
                    if buffer.len() < 64 {
                        buffer.push(c);
                        mesa_print!("{}", c);
                    }
                }
                KeyEvent::Special(SpecialKey::Enter) => {
                    mesa_println!();
                    return;
                }
                KeyEvent::Special(SpecialKey::Backspace) => {
                    if !buffer.is_empty() {
                        buffer.pop();
                        mesa_print!("\x08 \x08");
                    }
                }
                _ => {}
            }
        }
    }
}

fn read_line_password(buffer: &mut String) {
    loop {
        crate::scheduler::yield_now();
        curr_arch::halt();

        while let Some(event) = drivers::keyboard::read_event() {
            match event {
                KeyEvent::Char(c) => {
                    if buffer.len() < 64 {
                        buffer.push(c);
                        mesa_print!("*");
                    }
                }
                KeyEvent::Special(SpecialKey::Enter) => {
                    mesa_println!();
                    return;
                }
                KeyEvent::Special(SpecialKey::Backspace) => {
                    if !buffer.is_empty() {
                        buffer.pop();
                        mesa_print!("\x08 \x08");
                    }
                }
                _ => {}
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// STATUS BAR
// ══════════════════════════════════════════════════════════════════════════════

fn update_status() {
    let (free, total) = memory::pmm::stats();
    let used_mb = ((total - free) * memory::PAGE_SIZE) / 1024 / 1024;
    let total_mb = (total * memory::PAGE_SIZE) / 1024 / 1024;

    let cpu_count = limine_req::cpu_count();

    // RTC info (x86_64 only for now)
    struct TimeInfo {
        hour: u8,
        minute: u8,
        second: u8,
    }

    #[cfg(target_arch = "x86_64")]
    let dt = {
        let r = drivers::rtc::read();
        TimeInfo {
            hour: r.hour,
            minute: r.minute,
            second: r.second,
        }
    };
    #[cfg(target_arch = "aarch64")]
    let dt = TimeInfo {
        hour: 0,
        minute: 0,
        second: 0,
    };

    let (disk_used, disk_total) = fs::stats();
    // Convertir bloques a MB (asumiendo bloque de 1KB)
    let disk_used_mb = disk_used / 1024;
    let disk_total_mb = disk_total / 1024;

    #[cfg(target_arch = "x86_64")]
    let (bat_pct, bat_charging, bat_present) = {
        let st = drivers::battery::read_status();
        (st.percentage, st.is_charging, st.present)
    };
    #[cfg(not(target_arch = "x86_64"))]
    let (bat_pct, bat_charging, bat_present) = (100, false, true);

    drivers::framebuffer::update_status_bar(
        used_mb,
        total_mb,
        cpu_count,
        dt.hour,
        dt.minute,
        dt.second,
        disk_used_mb,
        disk_total_mb,
        bat_pct,
        bat_charging,
        bat_present,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// SHELL
// ══════════════════════════════════════════════════════════════════════════════

fn shell_loop() -> ! {
    // Auto-test: run modload.elf at startup (USB modules disabled)
    // serial_println!("[TEST] Ejecutando modload.elf...");
    // execute_command("exec /inyect/bin/modload.elf");
    // serial_println!("[TEST] modload.elf completado");

    let mut input = String::new();
    let mut history: Vec<String> = Vec::new();
    let mut last_status_update: u64 = 0;

    mesa_println!(
        "{}\n",
        t(
            "Escribe 'help' para ver los comandos disponibles.",
            "Type 'help' to see available commands."
        )
    );
    serial_println!("[SHELL] Shell iniciado");
    klog_info!("Shell started");

    update_status();

    loop {
        crate::scheduler::yield_now();
        let current_tick = curr_arch::get_ticks();
        if current_tick.wrapping_sub(last_status_update) >= 18 {
            update_status();
            last_status_update = current_tick;
        }

        // Prompt con usuario y hostname
        let username = users::current_username();
        let is_root = users::is_root();
        let hostname = config::get_hostname();
        let cwd = fs::cwd();

        drivers::framebuffer::set_color(if is_root {
            palette::LOVE
        } else {
            palette::FOAM
        });
        mesa_print!("{}", username);
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_print!("@");
        drivers::framebuffer::set_color(palette::IRIS);
        mesa_print!("{} ", hostname);
        drivers::framebuffer::set_color(palette::PINE);
        mesa_print!("{} ", cwd);
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_print!("{} ", if is_root { "#" } else { "$" });
        drivers::framebuffer::set_color(palette::TEXT);

        input.clear();
        let mut history_index = history.len();
        read_line_with_history(&mut input, &history, &mut history_index);

        let cmd = input.trim();
        if !cmd.is_empty() {
            if history.last().map(|s| s.as_str()) != Some(cmd) {
                history.push(String::from(cmd));
                if history.len() > 50 {
                    history.remove(0);
                }
            }
            // Soporte para redirección: comando > archivo o comando >> archivo
            let (final_cmd, redirect_file, append) = if cmd.contains(">>") {
                let parts: Vec<&str> = cmd.split(">>").map(|s| s.trim()).collect();
                if parts.len() == 2 {
                    (parts[0], Some(parts[1]), true)
                } else {
                    (cmd, None, false)
                }
            } else if cmd.contains('>') {
                let parts: Vec<&str> = cmd.split('>').map(|s| s.trim()).collect();
                if parts.len() == 2 {
                    (parts[0], Some(parts[1]), false)
                } else {
                    (cmd, None, false)
                }
            } else {
                (cmd, None, false)
            };

            // Activar redirección si es necesario
            if let Some(filename) = redirect_file {
                REDIRECT_BUFFER.lock().clear();
                *REDIRECT_ACTIVE.lock() = true;

                // Ejecutar el comando (que puede tener pipes internos)
                execute_maybe_piped(final_cmd);

                *REDIRECT_ACTIVE.lock() = false;
                let content = REDIRECT_BUFFER.lock();

                // Guardar al archivo
                if append {
                    let mut full_content = fs::read_to_string(filename).unwrap_or_default();
                    full_content.push_str(&content);
                    if let Err(e) = fs::write(filename, full_content.as_bytes()) {
                        print_error(&format!("Error redireccionando (>>): {}", e.as_str()));
                    }
                } else {
                    if let Err(e) = fs::write(filename, content.as_bytes()) {
                        print_error(&format!("Error redireccionando (>): {}", e.as_str()));
                    }
                }
            } else {
                execute_maybe_piped(cmd);
            }
        }
    }
}

fn read_line_with_history(buffer: &mut String, history: &[String], history_index: &mut usize) {
    let mut cursor_visible = false;
    let mut last_toggle_tick: u64 = 0;
    let mut last_status_tick: u64 = 0;
    let mut current_line = String::new();
    let mut saved_current = false;

    loop {
        crate::net::poll(); // Procesar red de fondo
        crate::scheduler::yield_now();
        // TEMP DEBUG: heartbeat del shell (input loop)
        {
            let t = curr_arch::get_ticks();
            if t % 150 == 0 {
                crate::serial_println!("[TEMP] shell-input tick={}", t);
            }
        }
        // Solo halt si no hay actividad de red pendiente (opcional, por ahora halt simple)
        curr_arch::halt();

        let current_tick = curr_arch::get_ticks();

        if current_tick.wrapping_sub(last_status_tick) >= 18 {
            update_status();
            last_status_tick = current_tick;
        }

        if current_tick.wrapping_sub(last_toggle_tick) >= 50 {
            if cursor_visible {
                mesa_print!("\x08 \x08");
            }
            cursor_visible = !cursor_visible;
            if cursor_visible {
                drivers::framebuffer::set_color(palette::SUBTLE);
                mesa_print!("_");
                drivers::framebuffer::set_color(palette::TEXT);
            }
            last_toggle_tick = current_tick;
        }

        while let Some(event) = drivers::keyboard::read_event() {
            if cursor_visible {
                mesa_print!("\x08 \x08");
                cursor_visible = false;
            }

            match event {
                KeyEvent::Char(c) => {
                    if buffer.len() < 256 {
                        buffer.push(c);
                        mesa_print!("{}", c);
                    }
                }
                KeyEvent::Special(SpecialKey::Enter) => {
                    mesa_println!();
                    return;
                }
                KeyEvent::Special(SpecialKey::Backspace) => {
                    if !buffer.is_empty() {
                        buffer.pop();
                        mesa_print!("\x08 \x08");
                    }
                }
                KeyEvent::Special(SpecialKey::Tab) => {
                    if let Some(completion) = autocomplete(buffer) {
                        for _ in 0..buffer.len() {
                            mesa_print!("\x08 \x08");
                        }
                        buffer.clear();
                        buffer.push_str(&completion);
                        mesa_print!("{}", buffer);
                    }
                }
                KeyEvent::Special(SpecialKey::ArrowUp) => {
                    if !history.is_empty() && *history_index > 0 {
                        if !saved_current {
                            current_line = buffer.clone();
                            saved_current = true;
                        }
                        *history_index -= 1;
                        replace_line(buffer, &history[*history_index]);
                    }
                }
                KeyEvent::Special(SpecialKey::ArrowDown) => {
                    if !history.is_empty() {
                        if *history_index < history.len() - 1 {
                            *history_index += 1;
                            replace_line(buffer, &history[*history_index]);
                        } else if *history_index == history.len() - 1 && saved_current {
                            *history_index = history.len();
                            replace_line(buffer, &current_line);
                            saved_current = false;
                        }
                    }
                }
                KeyEvent::Special(SpecialKey::CtrlC) => {
                    // SIGINT: cancelar línea actual, imprimir ^C y nueva línea
                    buffer.clear();
                    drivers::framebuffer::set_color(palette::LOVE);
                    mesa_print!("^C");
                    drivers::framebuffer::set_color(palette::TEXT);
                    mesa_println!();
                    return;
                }
                _ => {}
            }

            last_toggle_tick = current_tick;
        }
    }
}

fn autocomplete(input: &str) -> Option<String> {
    if input.is_empty() {
        return None;
    }

    let matches: Vec<&str> = COMMANDS
        .iter()
        .filter(|cmd| cmd.starts_with(input))
        .copied()
        .collect();

    if matches.len() == 1 {
        Some(String::from(matches[0]))
    } else if matches.len() > 1 {
        mesa_println!();
        drivers::framebuffer::set_color(palette::SUBTLE);
        for cmd in &matches {
            mesa_print!("{}  ", cmd);
        }
        mesa_println!();
        drivers::framebuffer::set_color(palette::TEXT);

        let first = matches[0];
        let mut common_len = input.len();

        for i in input.len()..first.len() {
            let c = first.chars().nth(i);
            if matches.iter().all(|m| m.chars().nth(i) == c) {
                common_len = i + 1;
            } else {
                break;
            }
        }

        if common_len > input.len() {
            Some(String::from(&first[..common_len]))
        } else {
            Some(String::from(input))
        }
    } else {
        None
    }
}

fn replace_line(buffer: &mut String, new_content: &str) {
    for _ in 0..buffer.len() {
        mesa_print!("\x08 \x08");
    }
    buffer.clear();
    buffer.push_str(new_content);
    mesa_print!("{}", buffer);
}

// ══════════════════════════════════════════════════════════════════════════════
// PIPE HELPERS (para cmd1 | cmd2)
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// PIPE & REDIRECTION HELPERS
// ══════════════════════════════════════════════════════════════════════════════

static REDIRECT_BUFFER: Mutex<String> = Mutex::new(String::new());
static REDIRECT_ACTIVE: Mutex<bool> = Mutex::new(false);

/// Escribe a stdout: si hay redirección a archivo, guarda en buffer;
/// si hay pipe, escribe al pipe; si no, a consola.
/// Escribe a stdout de forma eficiente usando fmt::Arguments
pub fn shell_stdout(args: core::fmt::Arguments) {
    use core::fmt::Write;

    if *REDIRECT_ACTIVE.lock() {
        let mut buf = REDIRECT_BUFFER.lock();
        let _ = buf.write_fmt(args);
    } else if let Some(fd) = *PIPE_STDOUT.lock() {
        // Para pipes, sí necesitamos un string temporal por ahora (o buffer intermedio)
        let s = alloc::format!("{}", args);
        let _ = pipe::pipe_write(fd, s.as_bytes());
    } else {
        let mut console = drivers::framebuffer::console::CONSOLE.lock();
        let _ = console.write_fmt(args);
    }
}

pub fn shell_stdout_ln(args: core::fmt::Arguments) {
    shell_stdout(args);
    shell_stdout(format_args!("\n"));
}

/// Lee desde stdin del pipe (cuando cat lee de pipe). Retorna bytes leídos en buf.
fn shell_stdin_read(buf: &mut [u8]) -> usize {
    if let Some(fd) = *PIPE_STDIN.lock() {
        pipe::pipe_read(fd, buf).unwrap_or(0)
    } else {
        0
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// EXECUTE COMMAND
// ══════════════════════════════════════════════════════════════════════════════

fn execute_maybe_piped(cmd: &str) {
    // Soporte para pipe: cmd1 | cmd2
    if cmd.contains('|') {
        let parts: Vec<&str> = cmd.split('|').map(|s| s.trim()).collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            if let Ok((read_fd, write_fd)) = pipe::create_pipe() {
                *PIPE_STDOUT.lock() = Some(write_fd);
                execute_command(parts[0]);
                *PIPE_STDOUT.lock() = None;
                let _ = pipe::pipe_close_write(write_fd);
                *PIPE_STDIN.lock() = Some(read_fd);
                execute_command(parts[1]);
                *PIPE_STDIN.lock() = None;
            } else {
                print_error("Demasiados pipes abiertos");
            }
        } else {
            print_error("Uso: comando1 | comando2");
        }
    } else {
        execute_command(cmd);
    }
}

/// Parsea argumentos soportando comillas dobles (para paths con espacios)
fn parse_args(input: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut current_start = None;
    let mut in_quotes = false;
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        if c == '"' {
            in_quotes = !in_quotes;
            i += 1;
            continue;
        }
        if c == ' ' && !in_quotes {
            if let Some(start) = current_start {
                args.push(&input[start..i]);
                current_start = None;
            }
        } else if current_start.is_none() {
            current_start = Some(i);
        }
        i += 1;
    }
    if let Some(start) = current_start {
        args.push(&input[start..]);
    }

    args
}

fn execute_command(cmd: &str) {
    let parts = parse_args(cmd);
    let command = parts.first().copied().unwrap_or("");
    let args = if parts.len() > 1 { &parts[1..] } else { &[] };

    match command {
        "help" => cmd_help(),
        "clear" => cmd_clear(),
        "info" => cmd_info(),
        "mem" => cmd_mem(),
        "hda" => cmd_hda(args),
        "usb" => cmd_usb(),
        "xhci_flush" => cmd_xhci_flush(),
        "echo" => cmd_echo(args),
        "time" | "uptime" => cmd_time(),
        #[cfg(target_arch = "x86_64")]
        "date" => cmd_date(),
        "neofetch" => cmd_neofetch(),
        "panic" => cmd_panic(),
        "test" => cmd_test(),
        "history" => cmd_history(),
        #[cfg(target_arch = "x86_64")]
        "reboot" => cmd_reboot(),
        #[cfg(target_arch = "x86_64")]
        "halt" | "shutdown" => cmd_halt(),
        #[cfg(target_arch = "x86_64")]
        "cpuinfo" => cmd_cpuinfo(),
        #[cfg(target_arch = "x86_64")]
        "smp" => cmd_smp(),
        #[cfg(target_arch = "x86_64")]
        "net" => cmd_net(args),
        #[cfg(target_arch = "x86_64")]
        "ifconfig" => cmd_ifconfig(args),
        #[cfg(target_arch = "x86_64")]
        "ping" => cmd_ping(args),
        "curl" => cmd_curl(args),
        "ip" => cmd_ip(),
        "nano" => cmd_nano(args),
        "config" => cmd_config(args),
        #[cfg(target_arch = "x86_64")]
        "arp" => cmd_arp(),
        #[cfg(target_arch = "x86_64")]
        "scan" => cmd_scan(),
        "html" => cmd_html(args),
        #[cfg(target_arch = "x86_64")]
        "beep" => drivers::audio::PcSpeaker::beep(),
        "colors" => cmd_colors(),
        "logo" => cmd_logo(),
        "dmesg" => cmd_dmesg(args),
        "hexdump" => cmd_hexdump(args),
        #[cfg(target_arch = "x86_64")]
        "lspci" => cmd_lspci(),
        "vmm" => cmd_vmm(),
        "tasks" | "ps" => cmd_tasks(),
        #[cfg(target_arch = "x86_64")]
        "acpi" => cmd_acpi(),
        "spawn" => cmd_spawn(args),
        // "usb" => cmd_usb(),        // USB disabled
        // "usbstatus" => cmd_usbstatus(), // USB disabled
        // "lsusb" => cmd_usb(),      // USB disabled
        // "usb_net" => cmd_usb_net(), // USB disabled
        "log-ps2" => cmd_log_ps2(),
        "snake" => cmd_snake(),
        "calc" => cmd_calc(),
        "calendar" => cmd_calendar(),
        "clock" => cmd_clock(),
        "desktop" => cmd_desktop(),
        "ls" => cmd_ls(args),
        "cd" => cmd_cd(args),
        "pwd" => cmd_pwd(),
        "cat" => cmd_cat(args),
        "mkdir" => cmd_mkdir(args),
        "touch" => cmd_touch(args),
        "rm" => cmd_rm(args),
        "rmdir" => cmd_rmdir(args),
        "mv" => cmd_mv(args),
        "tree" => cmd_tree(args),
        "write" => cmd_write(args),
        "kill" => cmd_kill(args),
        "yield" => cmd_yield(),
        "sched" => cmd_sched(),
        "whoami" => cmd_whoami(),
        "id" => cmd_id(),
        "users" => cmd_users(),
        "test_fs_user" => {
            mesa_println!("Ejecutando FILE_TEST en Ring 3...");
            userland::exec::exec_user_code("file_test", &userland::exec::programs::FILE_TEST);
        }
        "su" => cmd_su(args),
        "logout" => cmd_logout(),
        "exec" => cmd_exec(args),
        "userland" => cmd_userland(),
        "ring3" => cmd_ring3(),
        "passwd" => cmd_passwd(args),
        "useradd" => cmd_useradd(args),
        "userdel" => cmd_userdel(args),
        // UEFI NVRAM commands
        #[cfg(target_arch = "x86_64")]
        "nvram_set" => arch::x86_64::uefi_nvram_cmd::cmd_nvram_set(args),
        #[cfg(target_arch = "x86_64")]
        "nvram_get" => arch::x86_64::uefi_nvram_cmd::cmd_nvram_get(args),
        #[cfg(target_arch = "x86_64")]
        "nvram_del" => arch::x86_64::uefi_nvram_cmd::cmd_nvram_del(args),
        #[cfg(target_arch = "x86_64")]
        "nvram_list" => arch::x86_64::uefi_nvram_cmd::cmd_nvram_list(args),
        #[cfg(target_arch = "x86_64")]
        "nvram_test" => arch::x86_64::uefi_nvram_cmd::cmd_nvram_test(args),
        // Battery report (estilo upower -i / acpi -V)
        "battery-report" => cmd_battery_report(args),
        "chmod" => cmd_chmod(args),
        "run" => cmd_run(args),
        // Audio player
        "play" => cmd_play(args),
        "audio-info" => cmd_audio_info(args),
        "audio-list" => cmd_audio_list(args),
        "" => {}
        _ => {
            print_error(&format!(
                "{}: {}",
                t("Comando no encontrado", "Command not found"),
                command
            ));
            drivers::framebuffer::set_color(palette::SUBTLE);
            mesa_println!(
                "{}\n",
                t(
                    "Escribe 'help' para ver los comandos.",
                    "Type 'help' to see commands."
                )
            );
            drivers::framebuffer::set_color(palette::TEXT);
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// COMANDOS DE FILESYSTEM
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_ls(args: &[&str]) {
    let path = args.first().copied().unwrap_or(".");

    match fs::readdir(path) {
        Ok(entries) => {
            if entries.is_empty() {
                drivers::framebuffer::set_color(palette::SUBTLE);
                mesa_println!("({})", t("directorio vacío", "empty directory"));
                drivers::framebuffer::set_color(palette::TEXT);
            } else {
                for entry in entries {
                    let type_char = match entry.node_type {
                        fs::NodeType::Directory => {
                            drivers::framebuffer::set_color(palette::FOAM);
                            'd'
                        }
                        fs::NodeType::File => {
                            drivers::framebuffer::set_color(palette::TEXT);
                            '-'
                        }
                        fs::NodeType::Symlink => {
                            drivers::framebuffer::set_color(palette::IRIS);
                            'l'
                        }
                        fs::NodeType::Device => {
                            drivers::framebuffer::set_color(palette::GOLD);
                            'c'
                        }
                    };

                    mesa_print!("{} ", type_char);

                    if entry.node_type == fs::NodeType::Directory {
                        drivers::framebuffer::set_color(palette::FOAM);
                    } else {
                        drivers::framebuffer::set_color(palette::TEXT);
                    }

                    mesa_print!("{:<20}", entry.name);

                    if entry.node_type == fs::NodeType::File {
                        drivers::framebuffer::set_color(palette::SUBTLE);
                        mesa_print!(" {:>8} bytes", entry.size);
                    }

                    mesa_println!();
                }
                drivers::framebuffer::set_color(palette::TEXT);
            }
        }
        Err(e) => {
            print_error(&format!("ls: {}: {}", path, e.as_str()));
        }
    }
    mesa_println!();
}

fn cmd_cd(args: &[&str]) {
    let path = args.first().copied().unwrap_or("/");

    match fs::chdir(path) {
        Ok(()) => {}
        Err(e) => {
            print_error(&format!("cd: {}: {}", path, e.as_str()));
        }
    }
}

fn cmd_pwd() {
    mesa_println!("{}", fs::cwd());
    mesa_println!();
}

fn cmd_cat(args: &[&str]) {
    if args.is_empty() {
        // cat sin argumentos: leer de stdin (pipe)
        if PIPE_STDIN.lock().is_some() {
            let mut buf = [0u8; 256];
            loop {
                let n = shell_stdin_read(&mut buf);
                if n == 0 {
                    break;
                }
                for i in 0..n {
                    let c = buf[i] as char;
                    mesa_print!("{}", c);
                }
            }
            mesa_println!();
            return;
        }
        print_error(&format!(
            "{}: cat <archivo>  {}  comando | cat",
            t("Uso", "Usage"),
            t("o", "or")
        ));
        mesa_println!();
        return;
    }

    for raw_path in args {
        // Procesar backslash escaping: \  -> espacio, \\ -> backslash
        let mut path_buf = String::with_capacity(raw_path.len());
        let mut chars = raw_path.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some(' ') => path_buf.push(' '),
                    Some('\\') => path_buf.push('\\'),
                    Some(next) => {
                        path_buf.push('\\');
                        path_buf.push(next);
                    }
                    None => path_buf.push('\\'),
                }
            } else {
                path_buf.push(c);
            }
        }
        let path = &*path_buf;
        if path.is_empty() {
            continue;
        }

        match fs::read_to_string(path) {
            Ok(content) => {
                mesa_print!("{}", content);
                if !content.ends_with('\n') {
                    mesa_println!();
                }
            }
            Err(e) => {
                print_error(&format!("cat: {}: {}", path, e.as_str()));
            }
        }
    }
    mesa_println!();
}

fn cmd_mkdir(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: mkdir <directorio>");
        mesa_println!();
        return;
    }

    for path in args {
        match fs::mkdir(path) {
            Ok(()) => {
                print_success(&format!("Directorio '{}' creado", path));
            }
            Err(e) => {
                print_error(&format!("mkdir: {}: {}", path, e.as_str()));
            }
        }
    }
    mesa_println!();
}

fn cmd_touch(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: touch <archivo>");
        mesa_println!();
        return;
    }

    for path in args {
        match fs::touch(path) {
            Ok(()) => {}
            Err(e) => {
                print_error(&format!("touch: {}: {}", path, e.as_str()));
            }
        }
    }
}

fn cmd_rm(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: rm <archivo>");
        mesa_println!();
        return;
    }

    for path in args {
        match fs::rm(path) {
            Ok(()) => {
                print_success(&format!("'{}' eliminado", path));
            }
            Err(e) => {
                print_error(&format!("rm: {}: {}", path, e.as_str()));
            }
        }
    }
    mesa_println!();
}

fn cmd_rmdir(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: rmdir <directorio>");
        mesa_println!();
        return;
    }

    for path in args {
        match fs::rmdir(path) {
            Ok(()) => {
                print_success(&format!("Directorio '{}' eliminado", path));
            }
            Err(e) => {
                print_error(&format!("rmdir: {}: {}", path, e.as_str()));
            }
        }
    }
    mesa_println!();
}

fn cmd_mv(args: &[&str]) {
    if args.len() != 2 {
        print_error("Uso: mv <origen> <destino>");
        mesa_println!();
        return;
    }

    match fs::mv(args[0], args[1]) {
        Ok(()) => {
            print_success(&format!("'{}' -> '{}'", args[0], args[1]));
        }
        Err(e) => {
            print_error(&format!("mv: {}", e.as_str()));
        }
    }
    mesa_println!();
}

fn cmd_tree(args: &[&str]) {
    let path = args.first().copied().unwrap_or("/");

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("{}", path);
    drivers::framebuffer::set_color(palette::TEXT);

    fn print_tree(path: &str, prefix: &str, _is_last: bool) {
        use alloc::string::String;

        match crate::fs::readdir(path) {
            Ok(mut entries) => {
                entries.sort_by(|a, b| a.name.cmp(&b.name));
                let count = entries.len();

                for (i, entry) in entries.iter().enumerate() {
                    let is_last_entry = i == count - 1;
                    let connector = if is_last_entry {
                        "└── "
                    } else {
                        "├── "
                    };

                    crate::drivers::framebuffer::set_color(
                        crate::drivers::framebuffer::ui::palette::SUBTLE,
                    );
                    mesa_print!("{}{}", prefix, connector);

                    if entry.node_type == crate::fs::NodeType::Directory {
                        crate::drivers::framebuffer::set_color(
                            crate::drivers::framebuffer::ui::palette::FOAM,
                        );
                        mesa_println!("{}/", entry.name);

                        let new_prefix = if is_last_entry {
                            format!("{}    ", prefix)
                        } else {
                            format!("{}│   ", prefix)
                        };

                        let child_path = if path == "/" {
                            format!("/{}", entry.name)
                        } else {
                            format!("{}/{}", path, entry.name)
                        };

                        print_tree(&child_path, &new_prefix, is_last_entry);
                    } else {
                        crate::drivers::framebuffer::set_color(
                            crate::drivers::framebuffer::ui::palette::TEXT,
                        );
                        mesa_println!("{}", entry.name);
                    }
                }
            }
            Err(_) => {}
        }
    }

    print_tree(path, "", true);
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
}

fn cmd_write(args: &[&str]) {
    if args.len() < 2 {
        print_error("Uso: write <archivo> <contenido...>");
        mesa_println!();
        return;
    }

    let path = args[0];
    let content = args[1..].join(" ");
    let content_with_newline = format!("{}\n", content);

    match fs::write(path, content_with_newline.as_bytes()) {
        Ok(()) => {
            print_success(&format!(
                "Escrito {} bytes a '{}'",
                content_with_newline.len(),
                path
            ));
        }
        Err(e) => {
            print_error(&format!("write: {}: {}", path, e.as_str()));
        }
    }
    mesa_println!();
}

// ══════════════════════════════════════════════════════════════════════════════
// COMANDOS PRINCIPALES
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_help() {
    print_section(t("Comandos Disponibles", "Available Commands"));
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  {}", t("Sistema:", "System:"));
    drivers::framebuffer::set_color(palette::TEXT);
    print_cmd_help("help", t("Muestra esta ayuda", "Show this help"));
    print_cmd_help("clear", t("Limpia la pantalla", "Clear screen"));
    print_cmd_help("info", t("Informacion del sistema", "System info"));
    print_cmd_help(
        "neofetch",
        t("Info del sistema con estilo", "Stylish system info"),
    );
    print_cmd_help("logo", t("Muestra el logo de Mesa OS", "Show Mesa OS logo"));
    print_cmd_help("dmesg [n]", t("Muestra log del kernel", "Show kernel logs"));
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  {}", t("Hardware:", "Hardware:"));
    drivers::framebuffer::set_color(palette::TEXT);
    print_cmd_help("mem", t("Estado de la memoria", "Memory status"));
    print_cmd_help("vmm", t("Estado del VMM", "VMM status"));
    print_cmd_help("cpuinfo", t("Informacion del CPU", "CPU info"));
    print_cmd_help("lspci", t("Lista dispositivos PCI", "List PCI devices"));
    // print_cmd_help("usb", t("Estado del controlador USB xHCI", "USB xHCI controller status"));
    // print_cmd_help("usbstatus", t("Diagnostico detallado USB/xHCI", "Detailed USB/xHCI diagnostic"));
    print_cmd_help(
        "log-ps2",
        t(
            "Estado del driver PS/2 y touchpad",
            "PS/2 mouse and touchpad status",
        ),
    );
    print_cmd_help("snake", t("Juego de la serpiente", "Snake game"));
    print_cmd_help(
        "calc",
        t("Calculadora grafica (BETA)", "Graphical calculator (BETA)"),
    );
    print_cmd_help("calendar", t("Calendario", "Calendar"));
    print_cmd_help("clock", t("Reloj mundial", "World clock"));
    print_cmd_help(
        "desktop",
        t(
            "Entorno grafico (BETA) con raton",
            "Graphical desktop (BETA) with mouse",
        ),
    );
    print_cmd_help(
        "hda",
        t("Estado de Intel HD Audio", "Intel HD Audio status"),
    );
    print_cmd_help("acpi", t("Informacion ACPI", "ACPI info"));
    print_cmd_help("time/uptime", t("Tiempo encendido", "Uptime"));
    print_cmd_help("date", t("Fecha y hora", "Date and time"));
    print_cmd_help("hexdump <addr>", t("Dump de memoria", "Memory dump"));
    print_cmd_help("beep", t("Emite un pitido", "Emit a beep"));
    print_cmd_help(
        "speak <texto>",
        t(
            "Sintetiza voz (experimental)",
            "Speech synthesis (experimental)",
        ),
    );
    print_cmd_help(
        "battery-report [-v|-j|-w]",
        t(
            "Reporte completo de batería (estilo Linux)",
            "Full battery report (Linux style)",
        ),
    );
    print_cmd_help(
        "play <archivo>",
        t(
            "Reproduce un archivo de audio (WAV)",
            "Play an audio file (WAV)",
        ),
    );
    print_cmd_help(
        "audio-info <arch>",
        t(
            "Muestra metadatos de un archivo de audio",
            "Show audio file metadata",
        ),
    );
    print_cmd_help(
        "audio-list",
        t(
            "Lista archivos de audio en /inyect/audio",
            "List audio files in /inyect/audio",
        ),
    );
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  {}", t("Red:", "Network:"));
    drivers::framebuffer::set_color(palette::TEXT);
    print_cmd_help("net", t("Informacion de red", "Network info"));
    print_cmd_help(
        "ifconfig [ip netmask gw]",
        t("Configurar red", "Configure network"),
    );
    print_cmd_help(
        "ping <host/ip>",
        t("Ping ICMP con soporte DNS", "ICMP Ping with DNS support"),
    );
    print_cmd_help(
        "curl <url> [file]",
        t(
            "Cliente HTTP (GET) con descarga",
            "HTTP client (GET) with download",
        ),
    );
    print_cmd_help(
        "ip",
        t("Ver configuracion de red", "View network configuration"),
    );
    print_cmd_help("nano <file>", t("Editor de texto", "Text editor"));
    print_cmd_help("arp", t("Ver cache ARP", "View ARP cache"));
    print_cmd_help(
        "scan",
        t("Escanear red local (ARP)", "Scan local network (ARP)"),
    );
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  {}", t("Multitarea:", "Multitasking:"));
    drivers::framebuffer::set_color(palette::TEXT);
    print_cmd_help("tasks / ps", t("Lista de tareas", "Task list"));
    print_cmd_help("sched", t("Estado del scheduler", "Scheduler status"));
    print_cmd_help(
        "spawn <task>",
        t("Crear tarea (counter/spinner/worker)", "Create task"),
    );
    print_cmd_help("kill <id>", t("Terminar tarea", "Terminate task"));
    print_cmd_help("yield", t("Ceder CPU", "Yield CPU"));
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  {}", t("Usuarios:", "Users:"));
    drivers::framebuffer::set_color(palette::TEXT);
    print_cmd_help("whoami", t("Usuario actual", "Current user"));
    print_cmd_help("id", t("UID y GID", "UID and GID"));
    print_cmd_help("users", t("Lista de usuarios", "User list"));
    print_cmd_help("su [user]", t("Cambiar usuario", "Switch user"));
    print_cmd_help("passwd [user]", t("Cambiar contraseña", "Change password"));
    print_cmd_help(
        "useradd <n> <p> [admin]",
        t("Crear usuario permanente", "Create persistent user"),
    );
    print_cmd_help(
        "userdel <n>",
        t("Borrar usuario permanente", "Delete persistent user"),
    );
    print_cmd_help("logout", t("Cerrar sesion", "Logout"));
    print_cmd_help(
        "test_fs_user",
        t("Test VFS syscalls (Ring 3)", "VFS test (Ring 3)"),
    );
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  {}", t("Filesystem:", "Filesystem:"));
    drivers::framebuffer::set_color(palette::TEXT);
    print_cmd_help("ls [dir]", t("Lista directorio", "List directory"));
    print_cmd_help("cd <dir>", t("Cambia directorio", "Change directory"));
    print_cmd_help("pwd", t("Directorio actual", "Current directory"));
    print_cmd_help("cat <archivo>", t("Muestra contenido", "Show content"));
    print_cmd_help("mkdir <dir>", t("Crea directorio", "Create directory"));
    print_cmd_help(
        "touch <archivo>",
        t("Crea archivo vacío", "Create empty file"),
    );
    print_cmd_help("rm <archivo>", t("Elimina archivo", "Delete file"));
    print_cmd_help(
        "rmdir <dir>",
        t("Elimina directorio vacío", "Delete empty directory"),
    );
    print_cmd_help("mv <src> <dst>", t("Mueve/renombra", "Move/rename"));
    print_cmd_help("tree [dir]", t("Árbol de directorios", "Directory tree"));
    print_cmd_help(
        "write <f> <txt>",
        t("Escribe texto a archivo", "Write text to file"),
    );
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  {}", t("Avanzado:", "Advanced:"));
    drivers::framebuffer::set_color(palette::TEXT);
    print_cmd_help(
        "ring3",
        t("Demo de Ring 3 (solo root)", "Ring 3 demo (root only)"),
    );
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  {}", t("Utilidades:", "Utilities:"));
    drivers::framebuffer::set_color(palette::TEXT);
    print_cmd_help("echo <texto>", t("Imprime texto", "Print text"));
    print_cmd_help("colors", t("Muestra la paleta", "Show palette"));
    print_cmd_help("history", t("Info del historial", "History info"));
    print_cmd_help("test", t("Prueba el teclado", "Test keyboard"));
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  {}", t("Control:", "Control:"));
    drivers::framebuffer::set_color(palette::TEXT);
    print_cmd_help("reboot", t("Reinicia", "Reboot"));
    print_cmd_help("halt", t("Apaga", "Shutdown"));
    print_cmd_help("panic", t("Kernel panic (debug)", "Kernel panic"));
    mesa_println!();

    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!(
        "  {}",
        t(
            "Tip: TAB autocompleta, flechas navegan historial",
            "Tip: TAB to autocomplete, arrows to navigate history"
        )
    );
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
}

fn print_cmd_help(cmd: &str, desc: &str) {
    drivers::framebuffer::set_color(palette::GOLD);
    mesa_print!("    {:<18}", cmd);
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}", desc);
}

fn cmd_clear() {
    drivers::framebuffer::clear();
    update_status();
}

fn cmd_exec(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: exec <programa>  o  exec <ruta/archivo.elf>");
        mesa_println!();
        print_info("Programas: hello, counter. O ruta a binario ELF64.");
        mesa_println!();
        return;
    }

    let program = args[0];
    let task_id = match program {
        "hello" => userland::exec::exec_user_from_slice(program, &userland::exec::programs::HELLO),
        "counter" => {
            userland::exec::exec_user_from_slice(program, &userland::exec::programs::COUNTER)
        }
        _ => match fs::read(program) {
            Ok(bytes) => {
                if bytes.len() >= 4 && bytes[0..4] == [0x7f, b'E', b'L', b'F'] {
                    userland::exec::exec_user_from_slice(program, &bytes)
                } else {
                    print_error(&format!("'{}' no es un binario ELF64 válido", program));
                    mesa_println!();
                    return;
                }
            }
            Err(e) => {
                print_error(&format!(
                    "Programa '{}' no encontrado: {}",
                    program,
                    e.as_str()
                ));
                mesa_println!();
                return;
            }
        },
    };

    if task_id > 0 {
        print_success(&format!(
            "Proceso '{}' iniciado con PID {} (Ring 3)",
            program, task_id
        ));
    } else {
        print_error("Error al crear proceso");
    }
    mesa_println!();
}

fn cmd_userland() {
    print_section("Programas en Userland (Ring 3)");
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  Programas disponibles:");
    drivers::framebuffer::set_color(palette::TEXT);

    print_cmd_help("hello", "Hello World en Ring 3");
    print_cmd_help("counter", "Contador en Ring 3");

    mesa_println!();

    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("  Uso: exec <programa>");
    mesa_println!();
    mesa_println!("  Los programas se ejecutan en Ring 3 (modo usuario)");
    mesa_println!("  con protección completa de memoria y privilegios.");
    drivers::framebuffer::set_color(palette::TEXT);

    mesa_println!();
}

fn cmd_info() {
    print_section(t("Sistema", "System"));
    mesa_println!();

    print_info_line("OS", "Mesa OS v0.1.0");
    print_info_line(t("Arquitectura", "Architecture"), "x86_64");
    print_info_line("Bootloader", "Limine");
    print_info_line("Kernel", t("Hibrido (Rust)", "Hybrid (Rust)"));

    if let Some((phys, virt)) = limine_req::kernel_address() {
        print_info_line(
            t("Direccion", "Address"),
            &format!("{:#x} -> {:#x}", phys, virt),
        );
    }

    print_info_line("CPUs", &format!("{}", limine_req::cpu_count()));
    print_info_line(t("Usuario", "User"), &users::current_username());
    #[cfg(target_arch = "x86_64")]
    print_info_line(t("Fecha/Hora", "Date/Time"), &drivers::rtc::get_datetime());
    mesa_println!();
}

// ══════════════════════════════════════════════════════════════════════════════
// COMANDO: battery-report
// Reporte completo de batería estilo Linux (upower -i / acpi -V)
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_battery_report(args: &[&str]) {
    // Parseo de flags
    let mut verbose = false;
    let mut watch = false;
    let mut brief = false;
    let mut show_help = false;
    for a in args {
        match *a {
            "-v" | "--verbose" => verbose = true,
            "-w" | "--watch" => watch = true,
            "-b" | "--brief" => brief = true,
            "-h" | "--help" | "help" => show_help = true,
            _ => {}
        }
    }

    if show_help {
        print_section("battery-report — Ayuda");
        mesa_println!();
        print_info_line("Uso", "battery-report [flags]");
        mesa_println!();
        print_info_line(
            "-v, --verbose",
            "Muestra metadatos extendidos (OEM, ciclos, etc.)",
        );
        print_info_line(
            "-w, --watch",
            "Refresca la salida cada 2 segundos (Ctrl+C para salir)",
        );
        print_info_line("-b, --brief", "Una sola línea con los datos clave");
        print_info_line("-h, --help", "Muestra esta ayuda");
        mesa_println!();
        print_info("El reporte incluye: porcentaje, estado, energía (Wh/Ah),");
        print_info("tiempo estimado, salud (SOH), ciclos, voltaje, tasa, etc.");
        mesa_println!();
        return;
    }

    if brief {
        drivers::battery::print_report_brief();
        return;
    }

    if watch {
        // Modo monitor: refrescar cada 2 segundos
        loop {
            // Limpia pantalla (ANSI: \x1b[2J\x1b[H)
            mesa_print!("\x1b[2J\x1b[H");
            drivers::battery::print_report(verbose);
            for _ in 0..36 {
                // ~2s con tick 18.2Hz
                crate::scheduler::yield_now();
                curr_arch::halt();
                // Permitir Ctrl+C para salir
                if let Some(crate::drivers::keyboard::KeyEvent::Special(
                    crate::drivers::keyboard::SpecialKey::CtrlC,
                )) = crate::drivers::keyboard::read_event()
                {
                    mesa_println!();
                    return;
                }
            }
        }
    } else {
        drivers::battery::print_report(verbose);
    }
}

fn cmd_hda(args: &[&str]) {
    #[cfg(target_arch = "x86_64")]
    {
        if args.len() == 1 {
            if args[0] == "enumerate" || args[0] == "enumeration" {
                crate::drivers::hda::enumerate_codecs();
            } else if args[0] == "test" {
                crate::drivers::hda::play_test();
                mesa_println!("Reproduciendo audio de prueba...");
            } else if args[0] == "stop" {
                crate::drivers::hda::stop_stream();
                mesa_println!("Audio detenido.");
            } else {
                crate::drivers::hda::print_status();
                mesa_println!("Comandos: hda enumerate, hda test, hda stop, hda vol <0-100>");
            }
        } else if args.len() == 2 && (args[0] == "vol" || args[0] == "volume") {
            if let Ok(v) = args[1].parse::<u8>() {
                crate::drivers::hda::set_volume(v);
                mesa_println!("Volumen fijado al {}%", v);
            } else {
                mesa_println!("Por favor, indique un volumen de 0 a 100.");
            }
        } else {
            crate::drivers::hda::print_status();
            mesa_println!("Comandos: hda enumerate, hda test, hda stop, hda vol <0-100>");
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    mesa_println!("HDA no soportado en esta arquitectura.");
}

fn cmd_log_ps2() {
    #[cfg(target_arch = "x86_64")]
    {
        use palette::*;
        mesa_println!("=== Estado PS/2 Mouse y Touchpad ===");
        mesa_println!();
        drivers::framebuffer::set_color(FOAM);
        mesa_print!("[PS/2] ");
        drivers::framebuffer::set_color(TEXT);
        if drivers::mouse::is_initialized() {
            mesa_println!("Mouse PS/2: INICIALIZADO");
        } else {
            mesa_println!("Mouse PS/2: NO INICIALIZADO");
        }
        mesa_println!();
        drivers::framebuffer::set_color(FOAM);
        mesa_print!("[TPD]  ");
        drivers::framebuffer::set_color(TEXT);
        if drivers::touchpad::is_present() {
            mesa_println!("Touchpad: PRESENTE");
        } else {
            mesa_println!("Touchpad: AUSENTE");
        }
        mesa_println!("{}", drivers::touchpad::status());
        mesa_println!();
        drivers::framebuffer::set_color(SUBTLE);
        mesa_println!("Sugerencia: escribe 'desktop' para el modo grafico con raton.");
        drivers::framebuffer::set_color(TEXT);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        mesa_println!("PS/2 solo soportado en x86_64.");
    }
}

fn cmd_mem() {
    let (free, total) = memory::pmm::stats();
    let used = total - free;
    let free_mb = (free * memory::PAGE_SIZE) / 1024 / 1024;
    let used_mb = (used * memory::PAGE_SIZE) / 1024 / 1024;
    let total_mb = (total * memory::PAGE_SIZE) / 1024 / 1024;

    print_section(t("Memoria Fisica", "Physical Memory"));
    mesa_println!();

    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_print!("  {}:  ", t("Usada", "Used"));
    drivers::framebuffer::set_color(palette::GOLD);
    mesa_println!("{:>4} MB  ({} frames)", used_mb, used);

    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_print!("  {}:  ", t("Libre", "Free"));
    drivers::framebuffer::set_color(palette::SUCCESS);
    mesa_println!("{:>4} MB  ({} frames)", free_mb, free);

    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_print!("  {}:  ", t("Total", "Total"));
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{:>4} MB  ({} frames)", total_mb, total);

    mesa_println!();

    let bar_width = 40usize;
    let used_pct = if total > 0 {
        (used * 100 / total) as usize
    } else {
        0
    };
    let used_width = (used_pct * bar_width) / 100;

    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_print!("  [");

    for i in 0..bar_width {
        if i < used_width {
            let color = if used_pct > 80 {
                palette::ERROR
            } else if used_pct > 60 {
                palette::GOLD
            } else {
                palette::IRIS
            };
            drivers::framebuffer::set_color(color);
            mesa_print!("#");
        } else {
            drivers::framebuffer::set_color(palette::MUTED);
            mesa_print!("-");
        }
    }

    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_print!("] ");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}%", used_pct);
    mesa_println!();
}

fn cmd_echo(args: &[&str]) {
    if args.is_empty() {
        shell_stdout_ln(format_args!(""));
    } else {
        shell_stdout_ln(format_args!("{}", args.join(" ")));
    }
    shell_stdout_ln(format_args!(""));
}

fn cmd_time() {
    let ticks = curr_arch::get_ticks();
    let seconds = ticks / 18;
    let minutes = seconds / 60;
    let hours = minutes / 60;

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_print!("Uptime: ");
    drivers::framebuffer::set_color(palette::TEXT);

    if hours > 0 {
        mesa_println!("{:02}:{:02}:{:02}", hours, minutes % 60, seconds % 60);
    } else if minutes > 0 {
        mesa_println!("{:02}:{:02}", minutes, seconds % 60);
    } else {
        mesa_println!("{}s", seconds);
    }
    mesa_println!();
}

#[cfg(target_arch = "x86_64")]
fn cmd_date() {
    let datetime = drivers::rtc::read();
    let tz = drivers::rtc::timezone_name();
    let offset = drivers::rtc::current_timezone_offset();

    print_section(t("Fecha y Hora", "Date and Time"));
    mesa_println!();

    print_info_line(t("Fecha", "Date"), &datetime.format_date());
    print_info_line(t("Hora", "Time"), &datetime.format_time());
    print_info_line("Timezone", &format!("{} (UTC+{})", tz, offset));
    print_info_line(
        t("Completo", "Full"),
        &format!("{} {}", datetime.format(), tz),
    );

    mesa_println!();
}

fn cmd_dmesg(args: &[&str]) {
    let count: usize = args.first().and_then(|s| s.parse().ok()).unwrap_or(20);

    print_section("Kernel Log");
    mesa_println!();

    let logbuf = log::KERNEL_LOG.lock();

    if logbuf.is_empty() {
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("  (sin mensajes)");
        drivers::framebuffer::set_color(palette::TEXT);
    } else {
        for entry in logbuf.last_n(count) {
            let time_secs = entry.timestamp / 18;
            let time_ms = (entry.timestamp % 18) * 55;

            let level_color = match entry.level {
                log::LogLevel::Debug => palette::MUTED,
                log::LogLevel::Info => palette::FOAM,
                log::LogLevel::Warn => palette::GOLD,
                log::LogLevel::Error => palette::ERROR,
            };

            drivers::framebuffer::set_color(palette::SUBTLE);
            mesa_print!("[{:5}.{:03}] ", time_secs, time_ms);

            drivers::framebuffer::set_color(level_color);
            mesa_print!("{} ", entry.level.prefix());

            drivers::framebuffer::set_color(palette::TEXT);
            mesa_println!("{}", entry.message);
        }
    }

    mesa_println!();
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("  Total: {} mensajes", logbuf.len());
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
}

fn cmd_hexdump(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: hexdump <direccion> [longitud]");
        mesa_println!();
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("  Ejemplo: hexdump 0x1000 64");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();
        return;
    }

    let addr_str = args[0].trim_start_matches("0x").trim_start_matches("0X");
    let addr = match u64::from_str_radix(addr_str, 16) {
        Ok(a) => a,
        Err(_) => {
            print_error(&format!("Direccion invalida: {}", args[0]));
            mesa_println!();
            return;
        }
    };

    let len: usize = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(64)
        .min(256);

    print_section(&format!("Hexdump @ {:#x}", addr));
    mesa_println!();

    let ptr = addr as *const u8;

    for row in 0..(len / 16 + if len % 16 != 0 { 1 } else { 0 }) {
        let row_addr = addr + (row * 16) as u64;
        let row_len = (len - row * 16).min(16);

        drivers::framebuffer::set_color(palette::FOAM);
        mesa_print!("  {:08x}  ", row_addr);

        drivers::framebuffer::set_color(palette::TEXT);
        for i in 0..16 {
            if i < row_len {
                let byte = unsafe { *ptr.add(row * 16 + i) };
                mesa_print!("{:02x} ", byte);
            } else {
                mesa_print!("   ");
            }
            if i == 7 {
                mesa_print!(" ");
            }
        }

        mesa_print!(" ");

        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_print!("|");
        for i in 0..row_len {
            let byte = unsafe { *ptr.add(row * 16 + i) };
            if byte >= 0x20 && byte < 0x7f {
                mesa_print!("{}", byte as char);
            } else {
                mesa_print!(".");
            }
        }
        mesa_println!("|");
    }

    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
}

#[cfg(target_arch = "x86_64")]
fn cmd_lspci() {
    print_section("Dispositivos PCI");
    mesa_println!();

    let devices = pci::devices();

    if devices.is_empty() {
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("  No se encontraron dispositivos PCI");
        drivers::framebuffer::set_color(palette::TEXT);
    } else {
        for dev in &devices {
            drivers::framebuffer::set_color(palette::FOAM);
            mesa_print!("  {:02x}:{:02x}.{}", dev.bus, dev.device, dev.function);

            drivers::framebuffer::set_color(palette::SUBTLE);
            mesa_print!(" {:04x}:{:04x}", dev.vendor_id, dev.device_id);

            drivers::framebuffer::set_color(palette::TEXT);
            mesa_print!(" {}", dev.class_name());

            drivers::framebuffer::set_color(palette::MUTED);
            mesa_println!(" ({})", dev.vendor_name());
        }
    }

    mesa_println!();
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("  Total: {} dispositivos", devices.len());
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
}

fn cmd_usb() {
    crate::drivers::usb::xhci_native::print_usb_status();
}

fn cmd_xhci_flush() {
    crate::drivers::usb::xhci_native::xhci_log_flush_to_file();
}

fn cmd_usbstatus() {
    crate::shim::usb_status_dump();
}

fn cmd_usb_net() {
    print_section("USB Tethering (RNDIS/CDC-ECM)");
    mesa_println!();
    print_info("La red USB ahora se configura via los drivers del Shim Linux.");
    mesa_println!();
}

fn cmd_vmm() {
    print_section("Virtual Memory Manager");
    mesa_println!();

    print_info_line("HHDM Offset", &format!("{:#x}", memory::vmm::hhdm_offset()));

    let (free, total) = memory::pmm::stats();
    print_info_line("Frames libres", &format!("{}", free));
    print_info_line("Frames totales", &format!("{}", total));

    mesa_println!();
}

fn cmd_tasks() {
    print_section("Tareas del Sistema");
    mesa_println!();

    let info = scheduler::get_info();

    print_info_line(
        "Estado",
        if info.scheduler_ready {
            "Activo"
        } else {
            "Inactivo"
        },
    );
    print_info_line(
        "Tarea actual",
        &format!("{} (ID={})", info.current_task_name, info.current_task_id),
    );
    print_info_line("Tareas listas", &format!("{}", info.ready_tasks));
    print_info_line("Total tareas", &format!("{}", info.total_tasks));

    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  Lista de tareas:");
    drivers::framebuffer::set_color(palette::TEXT);

    for (id, name, state, ticks) in scheduler::list_tasks() {
        let state_str = match state {
            scheduler::TaskState::Ready => "READY",
            scheduler::TaskState::Running => "RUNNING",
            scheduler::TaskState::Blocked => "BLOCKED",
            scheduler::TaskState::Sleeping(_) => "SLEEPING",
            scheduler::TaskState::Terminated => "TERMINATED",
        };

        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_print!("    {:>3} ", id);

        let state_color = match state {
            scheduler::TaskState::Running => palette::SUCCESS,
            scheduler::TaskState::Ready => palette::FOAM,
            scheduler::TaskState::Blocked => palette::GOLD,
            _ => palette::MUTED,
        };
        drivers::framebuffer::set_color(state_color);
        mesa_print!("{:<10} ", state_str);

        drivers::framebuffer::set_color(palette::TEXT);
        mesa_print!("{:<16} ", name);

        drivers::framebuffer::set_color(palette::MUTED);
        mesa_println!("{} ticks", ticks);
    }

    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
}

#[cfg(target_arch = "x86_64")]
fn cmd_acpi() {
    print_section("ACPI Information");
    mesa_println!();

    if let Some(info) = acpi::get_info() {
        print_info_line("RSDP", &format!("{:#x}", info.rsdp_address));
        print_info_line("Revision", &format!("{}", info.revision));
        print_info_line("OEM", &info.oem_id);
        print_info_line("Local APIC", &format!("{:#x}", info.local_apic_address));

        if info.ioapic_address != 0 {
            print_info_line("I/O APIC", &format!("{:#x}", info.ioapic_address));
        }

        print_info_line("CPUs (ACPI)", &format!("{}", info.cpu_count));
    } else {
        drivers::framebuffer::set_color(palette::GOLD);
        mesa_println!("  ACPI no disponible");
        drivers::framebuffer::set_color(palette::TEXT);
    }

    mesa_println!();
}

fn cmd_sched() {
    print_section("Estado del Scheduler");
    mesa_println!();

    let info = scheduler::get_info();

    print_info_line("Activo", if info.scheduler_ready { "Sí" } else { "No" });
    print_info_line(
        "Tarea actual",
        &format!("{} (ID {})", info.current_task_name, info.current_task_id),
    );
    print_info_line("En cola", &format!("{}", info.ready_tasks));
    print_info_line("Total", &format!("{}", info.total_tasks));

    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("  Detalles de tareas:");
    drivers::framebuffer::set_color(palette::TEXT);

    for (id, name, state, ticks) in scheduler::list_tasks() {
        let state_str = match state {
            scheduler::TaskState::Ready => "READY",
            scheduler::TaskState::Running => "RUNNING",
            scheduler::TaskState::Blocked => "BLOCKED",
            scheduler::TaskState::Sleeping(_) => "SLEEPING",
            scheduler::TaskState::Terminated => "TERMINATED",
        };

        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_print!("    [{}] ", id);

        let color = match state {
            scheduler::TaskState::Running => palette::SUCCESS,
            scheduler::TaskState::Ready => palette::FOAM,
            _ => palette::MUTED,
        };
        drivers::framebuffer::set_color(color);
        mesa_print!("{:<10} ", state_str);

        drivers::framebuffer::set_color(palette::TEXT);
        mesa_print!("{:<16} ", name);

        drivers::framebuffer::set_color(palette::MUTED);
        mesa_println!("(ticks: {})", ticks);
    }

    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();

    print_info("Usa 'yield' para forzar un cambio de contexto");
    mesa_println!();
}

// ══════════════════════════════════════════════════════════════════════════════
// COMANDOS DE MULTITAREA
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_spawn(args: &[&str]) {
    if args.is_empty() {
        print_section("Tareas Disponibles");
        mesa_println!();
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("  Uso: spawn <nombre>");
        mesa_println!();
        mesa_println!("  Demos disponibles:");
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_println!("    counter   - Contador en background");
        mesa_println!("    spinner   - Animación de spinner");
        mesa_println!("    worker    - Tarea de trabajo simulado");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();
        return;
    }

    let name = args[0];

    let entry: fn() = match name {
        "counter" => demo_counter,
        "spinner" => demo_spinner,
        "worker" => demo_worker,
        _ => {
            print_error(&format!("Demo '{}' no encontrada", name));
            mesa_println!();
            return;
        }
    };

    let id = scheduler::spawn(name, entry);
    print_success(&format!("Tarea '{}' creada con ID {}", name, id));
    mesa_println!();
}

fn cmd_kill(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: kill <id>");
        mesa_println!();
        return;
    }

    let id: u64 = match args[0].parse() {
        Ok(n) => n,
        Err(_) => {
            print_error("ID inválido");
            mesa_println!();
            return;
        }
    };

    match scheduler::kill(id) {
        Ok(()) => print_success(&format!("Tarea {} terminada", id)),
        Err(e) => print_error(e),
    }
    mesa_println!();
}

fn cmd_yield() {
    print_info("Cediendo CPU...");
    scheduler::yield_now();
    print_success("CPU recuperado");
    mesa_println!();
}

// ══════════════════════════════════════════════════════════════════════════════
// TAREAS DE DEMO
// ══════════════════════════════════════════════════════════════════════════════

fn demo_counter() {
    let mut count = 0u64;
    loop {
        count += 1;
        if count % 1000000 == 0 {
            crate::serial_println!("[COUNTER] count = {}", count);
        }
        if count % 100000 == 0 {
            scheduler::yield_now();
        }
    }
}

fn demo_spinner() {
    let chars = ['-', '\\', '|', '/'];
    let mut idx = 0usize;
    loop {
        crate::serial_println!("[SPINNER] {}", chars[idx % 4]);
        idx += 1;
        for _ in 0..50000 {
            core::hint::spin_loop();
        }
        scheduler::yield_now();
    }
}

fn demo_worker() {
    crate::serial_println!("[WORKER] Starting work...");
    for i in 0..10 {
        crate::serial_println!("[WORKER] Working... {}/10", i + 1);
        for _ in 0..100000 {
            core::hint::spin_loop();
        }
        scheduler::yield_now();
    }
    crate::serial_println!("[WORKER] Work complete!");
}

// ══════════════════════════════════════════════════════════════════════════════
// COMANDOS DE USUARIO
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_whoami() {
    let name = users::current_username();
    mesa_println!("{}", name);
    mesa_println!();
}

fn cmd_id() {
    let uid = users::current_uid();
    let gid = users::current_gid();
    let name = users::current_username();

    mesa_println!(
        "uid={}({}) gid={}({})",
        uid,
        name,
        gid,
        users::get_group(gid)
            .map(|g| g.name)
            .unwrap_or_else(|| String::from("unknown"))
    );
    mesa_println!();
}

fn cmd_users() {
    print_section("Usuarios del Sistema");
    mesa_println!();

    for user in users::list_users() {
        let is_current = user.uid == users::current_uid();

        drivers::framebuffer::set_color(if is_current {
            palette::SUCCESS
        } else {
            palette::TEXT
        });
        mesa_print!("  {}", if is_current { "→ " } else { "  " });

        drivers::framebuffer::set_color(palette::FOAM);
        mesa_print!("{:<12}", user.name);

        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_print!("uid={:<5} gid={:<5}", user.uid, user.gid);

        drivers::framebuffer::set_color(palette::MUTED);
        mesa_println!(" {}", user.home);
    }

    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
}

fn cmd_useradd(args: &[&str]) {
    if !users::is_root() {
        print_error(t(
            "Solo root puede crear usuarios",
            "Only root can create users",
        ));
        mesa_println!();
        return;
    }

    if args.len() < 2 {
        print_error(&format!(
            "{}: useradd <nombre> <password> [admin]",
            t("Uso", "Usage")
        ));
        mesa_println!();
        return;
    }

    let name = args[0];
    let pass = args[1];
    let is_admin = args.get(2).map(|&s| s == "admin").unwrap_or(false);

    crate::serial_println!("[CMD] Agregando usuario '{}'...", name);
    match users::add_user(name, pass, is_admin) {
        Ok(()) => {
            crate::serial_println!("[CMD] Usuario agregado a la base de datos.");
            print_success(&format!(
                "Usuario '{}' creado ({})",
                name,
                if is_admin { "Admin" } else { "Usuario" }
            ));

            // Crear home
            let home = format!("/home/{}", name);
            crate::serial_println!("[CMD] Creando directorio home '{}'...", home);
            match fs::mkdir(&home) {
                Ok(_) => crate::serial_println!("[CMD] Home creado."),
                Err(e) => crate::serial_println!("[CMD] Error creando home: {}", e.as_str()),
            }
        }
        Err(e) => {
            crate::serial_println!("[CMD] Error agregando usuario: {}", e);
            print_error(e);
        }
    }
    mesa_println!();
}

fn cmd_userdel(args: &[&str]) {
    if !users::is_root() {
        print_error(t(
            "Solo root puede borrar usuarios",
            "Only root can delete users",
        ));
        mesa_println!();
        return;
    }

    if args.is_empty() {
        print_error("Uso: userdel <nombre>");
        mesa_println!();
        return;
    }

    let name = args[0];
    match users::remove_user(name) {
        Ok(()) => print_success(&format!("Usuario '{}' eliminado", name)),
        Err(e) => print_error(e),
    }
    mesa_println!();
}

fn cmd_su(args: &[&str]) {
    if args.is_empty() {
        if users::current_uid() == 0 {
            print_info(t("Ya eres root", "You are already root"));
            mesa_println!();
            return;
        }

        // Pedir contraseña para root
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_print!("{}: ", t("Contraseña de root", "Root password"));
        drivers::framebuffer::set_color(palette::TEXT);

        let mut password = String::new();
        read_line_password(&mut password);

        if let Some(_) = users::get_user_by_name("root") {
            if users::login("root", &password).is_ok() {
                print_success(t("Cambiado a usuario 'root'", "Switched to 'root' user"));
            } else {
                print_error(t("Contraseña incorrecta", "Incorrect password"));
            }
        }
        mesa_println!();
        return;
    }

    let target = args[0];

    match users::get_user_by_name(target) {
        Some(user) => {
            // Root puede cambiar sin contraseña
            if users::current_uid() == 0 {
                match users::set_current_user_by_name(&user.name) {
                    Ok(()) => print_success(&format!("Cambiado a usuario '{}'", user.name)),
                    Err(e) => print_error(e),
                }
                mesa_println!();
                return;
            }

            // Pedir contraseña
            drivers::framebuffer::set_color(palette::FOAM);
            mesa_print!("Contraseña de {}: ", user.name);
            drivers::framebuffer::set_color(palette::TEXT);

            let mut password = String::new();
            read_line_password(&mut password);

            if user.check_password(password.trim()) {
                match users::set_current_user_by_name(&user.name) {
                    Ok(()) => print_success(&format!(
                        "{} '{}'",
                        t("Cambiado a usuario", "Switched to user"),
                        user.name
                    )),
                    Err(e) => print_error(e),
                }
            } else {
                print_error(t("Contraseña incorrecta", "Incorrect password"));
            }
        }
        None => {
            print_error(&format!("Usuario '{}' no encontrado", target));
        }
    }
    mesa_println!();
}

fn cmd_logout() {
    print_info("Cerrando sesión...");
    users::logout();
    klog_info!("User logged out");

    for _ in 0..5000000 {
        core::hint::spin_loop();
    }

    drivers::framebuffer::clear();
    login_screen();
}

fn cmd_passwd(args: &[&str]) {
    let target_user = if args.is_empty() {
        users::current_username()
    } else {
        String::from(args[0])
    };

    if target_user != users::current_username() && !users::is_root() {
        print_error("Solo root puede cambiar contraseñas de otros usuarios");
        mesa_println!();
        return;
    }

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_print!("Nueva contraseña para {}: ", target_user);
    drivers::framebuffer::set_color(palette::TEXT);

    let mut password = String::new();
    read_line_password(&mut password);

    match users::change_password(&target_user, password.trim()) {
        Ok(()) => print_success("Contraseña actualizada"),
        Err(e) => print_error(e),
    }
    mesa_println!();
}

fn cmd_ring3() {
    if !users::is_root() {
        print_error("Solo root puede ejecutar código en Ring 3");
        mesa_println!();
        return;
    }

    print_warning("Ejecutando demo Ring 3...");
    mesa_println!();

    let pid = userland::exec::exec_user_code("ring3_demo", &userland::exec::programs::HELLO);

    if pid > 0 {
        print_success(&format!("Proceso Ring 3 lanzado (PID={})", pid));
        print_info("El proceso tiene su propio espacio de direcciones");
    } else {
        print_error("Error al crear proceso Ring 3");
    }
    mesa_println!();
}

// ══════════════════════════════════════════════════════════════════════════════
// VISUAL
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_neofetch() {
    let ticks = curr_arch::get_ticks();
    let seconds = ticks / 18;
    let minutes = (seconds / 60) % 60;
    let hours = seconds / 3600;
    let secs = seconds % 60;

    let (free, total) = memory::pmm::stats();
    let total_mb = (total * memory::PAGE_SIZE) / 1024 / 1024;
    let used_mb = ((total - free) * memory::PAGE_SIZE) / 1024 / 1024;

    let uptime_str = if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    };
    #[cfg(target_arch = "x86_64")]
    let datetime = drivers::rtc::read();

    mesa_println!();

    drivers::framebuffer::set_color(palette::IRIS);
    mesa_print!("    ##m   m##");
    print_neo_info("OS", "Mesa OS v0.1.0");

    drivers::framebuffer::set_color(palette::IRIS);
    mesa_print!("    ###m m###");
    print_neo_info("Kernel", "Hybrid (Rust)");

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_print!("    ## ### ##");
    print_neo_info("Arch", "x86_64");

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_print!("    ## '#' ##");
    print_neo_info("Boot", "Limine (UEFI)");

    drivers::framebuffer::set_color(palette::GOLD);
    mesa_print!("    ##     ##");
    print_neo_info("CPUs", &format!("{}", limine_req::cpu_count()));

    drivers::framebuffer::set_color(palette::GOLD);
    mesa_print!("    ''     ''");
    print_neo_info("RAM", &format!("{}/{} MB", used_mb, total_mb));

    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!("             ");
    print_neo_info(t("Uptime", "Uptime"), &uptime_str);

    #[cfg(target_arch = "x86_64")]
    {
        mesa_print!("             ");
        print_neo_info(t("Fecha", "Date"), &datetime.format_date());

        mesa_print!("             ");
        print_neo_info(t("Hora", "Time"), &datetime.format_time());
    }

    mesa_print!("             ");
    print_neo_info(t("Usuario", "User"), &users::current_username());

    mesa_print!("             ");
    print_neo_info("Shell", "mesa-sh");

    mesa_println!();

    mesa_print!("    ");
    let colors = [
        palette::LOVE,
        palette::GOLD,
        palette::ROSE,
        palette::PINE,
        palette::FOAM,
        palette::IRIS,
        palette::SUCCESS,
        palette::TEXT,
    ];
    for color in colors {
        drivers::framebuffer::set_color(color);
        mesa_print!("###");
    }
    mesa_println!();

    mesa_print!("    ");
    for _ in 0..8 {
        drivers::framebuffer::set_color(palette::MUTED);
        mesa_print!("---");
    }
    mesa_println!();

    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
}

fn print_neo_info(label: &str, value: &str) {
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_print!("   ");
    drivers::framebuffer::set_color(palette::FOAM);
    mesa_print!("{}", label);
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_print!(": ");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}", value);
}

// ══════════════════════════════════════════════════════════════════════════════
// OTROS COMANDOS
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_history() {
    print_info("TAB autocompleta, flechas navegan historial");
    mesa_println!();
}

fn cmd_test() {
    print_section("Test de Teclado");
    mesa_println!();
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("Presiona teclas (Enter para terminar):");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();

    let mut test_input = String::new();
    read_line_simple(&mut test_input);

    print_success(&format!("Recibido: '{}'", test_input));
    mesa_println!();
}

fn cmd_panic() {
    print_warning("Provocando kernel panic...");
    klog_warn!("User requested kernel panic");
    panic!("Panic solicitado por el usuario");
}

#[cfg(target_arch = "x86_64")]
fn cmd_reboot() {
    print_warning("Reiniciando sistema...");
    klog_info!("System reboot requested");

    // Sincronización deshabilitada por seguridad

    unsafe {
        let mut port: x86_64::instructions::port::Port<u8> =
            x86_64::instructions::port::Port::new(0x64);

        for _ in 0..10000 {
            let status: u8 = {
                let mut status_port: x86_64::instructions::port::Port<u8> =
                    x86_64::instructions::port::Port::new(0x64);
                status_port.read()
            };
            if status & 0x02 == 0 {
                break;
            }
        }

        port.write(0xFE);

        curr_arch::disable_interrupts();
        let invalid_idt = x86_64::structures::DescriptorTablePointer {
            limit: 0,
            base: x86_64::VirtAddr::new(0),
        };
        x86_64::instructions::tables::lidt(&invalid_idt);
        core::arch::asm!("int3", options(nomem, nostack));
    }

    halt();
}

#[cfg(target_arch = "x86_64")]
fn cmd_halt() {
    print_section("Apagando Sistema");
    mesa_println!();

    klog_info!("System halt requested");

    // Sincronización deshabilitada por seguridad
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("  Mesa OS se ha detenido.");
    mesa_println!("  Es seguro apagar el equipo.");
    mesa_println!();
    mesa_println!("  (QEMU: Ctrl+A, X para salir)");

    curr_arch::disable_interrupts();
    loop {
        curr_arch::halt();
    }
}

#[cfg(target_arch = "x86_64")]
fn cmd_cpuinfo() {
    print_section("Informacion del CPU");
    mesa_println!();

    use core::arch::x86_64::__cpuid;

    let vendor = unsafe {
        let cpuid = __cpuid(0);
        let mut vendor_bytes = [0u8; 12];
        vendor_bytes[0..4].copy_from_slice(&cpuid.ebx.to_le_bytes());
        vendor_bytes[4..8].copy_from_slice(&cpuid.edx.to_le_bytes());
        vendor_bytes[8..12].copy_from_slice(&cpuid.ecx.to_le_bytes());

        let mut vendor_str = String::new();
        for &b in &vendor_bytes {
            if b != 0 {
                vendor_str.push(b as char);
            }
        }
        vendor_str
    };

    print_info_line("Vendor", &vendor);
    print_info_line("Cores", &format!("{}", limine_req::cpu_count()));
    print_info_line("Arquitectura", "x86_64 (64-bit)");
    #[cfg(target_arch = "x86_64")]
    {
        print_info_line(
            "Núcleos activos",
            &format!("{} (SMP)", crate::smp::active_cpu_count()),
        );
        print_info_line(
            "CPU actual",
            &format!(
                "{} (LAPIC ID {})",
                crate::smp::current_cpu_id(),
                crate::smp::current_cpu()
                    .lapic_id
                    .load(core::sync::atomic::Ordering::Relaxed)
            ),
        );
    }

    let features = unsafe {
        let cpuid = __cpuid(1);
        let mut feats: Vec<&str> = Vec::new();

        if cpuid.edx & (1 << 25) != 0 {
            feats.push("SSE");
        }
        if cpuid.edx & (1 << 26) != 0 {
            feats.push("SSE2");
        }
        if cpuid.ecx & (1 << 0) != 0 {
            feats.push("SSE3");
        }
        if cpuid.ecx & (1 << 19) != 0 {
            feats.push("SSE4.1");
        }
        if cpuid.ecx & (1 << 20) != 0 {
            feats.push("SSE4.2");
        }
        if cpuid.ecx & (1 << 28) != 0 {
            feats.push("AVX");
        }

        feats
    };

    if !features.is_empty() {
        print_info_line("Features", &features.join(", "));
    }

    mesa_println!();
}

#[cfg(target_arch = "x86_64")]
fn cmd_smp() {
    use core::sync::atomic::Ordering;

    print_section("Carga por Núcleo (SMP)");
    mesa_println!();

    let active = crate::smp::active_cpu_count();
    let bar_len = 20;

    for idx in 0..active {
        let pc = crate::smp::cpu_data(idx);
        let load = crate::smp::cpu_load(idx);
        let lapic = pc.lapic_id.load(Ordering::Relaxed);
        let total = pc.total_ticks.load(Ordering::Relaxed);
        let idle = pc.idle_ticks.load(Ordering::Relaxed);
        let task = pc
            .current
            .lock()
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| String::from("ninguna"));

        let filled = ((load / 100.0) * bar_len as f32) as usize;
        let filled = filled.min(bar_len);
        let mut bar = String::new();
        for i in 0..bar_len {
            if i < filled {
                bar.push('#');
            } else {
                bar.push('-');
            }
        }

        mesa_println!(
            "  CPU {:<2} LAPIC {:<3}  [{}]  {:>5.1}%  {}",
            idx,
            lapic,
            bar,
            load,
            task
        );
        mesa_println!("       ticks: total={} ocio={}", total, idle);
    }

    mesa_println!();
}

fn cmd_colors() {
    print_section("Paleta de Colores (Rose Pine)");
    mesa_println!();

    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("  Base:");
    mesa_print!("    ");
    drivers::framebuffer::set_color(palette::BASE);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!(" BASE    ");
    drivers::framebuffer::set_color(palette::SURFACE);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!(" SURFACE ");
    drivers::framebuffer::set_color(palette::OVERLAY);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!(" OVERLAY");

    mesa_println!();
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("  Texto:");
    mesa_print!("    ");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!("#### TEXT    ");
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_print!("#### SUBTLE  ");
    drivers::framebuffer::set_color(palette::MUTED);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!(" MUTED");

    mesa_println!();
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("  Acentos:");
    mesa_print!("    ");
    drivers::framebuffer::set_color(palette::LOVE);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!(" LOVE    ");
    drivers::framebuffer::set_color(palette::GOLD);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!(" GOLD    ");
    drivers::framebuffer::set_color(palette::ROSE);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!(" ROSE");

    mesa_print!("    ");
    drivers::framebuffer::set_color(palette::PINE);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!(" PINE    ");
    drivers::framebuffer::set_color(palette::FOAM);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!(" FOAM    ");
    drivers::framebuffer::set_color(palette::IRIS);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!(" IRIS");

    mesa_println!();
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("  Estados:");
    mesa_print!("    ");
    drivers::framebuffer::set_color(palette::SUCCESS);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!(" SUCCESS ");
    drivers::framebuffer::set_color(palette::WARNING);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_print!(" WARNING ");
    drivers::framebuffer::set_color(palette::ERROR);
    mesa_print!("####");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!(" ERROR");

    mesa_println!();
}

fn cmd_logo() {
    mesa_println!();

    drivers::framebuffer::set_color(palette::IRIS);
    mesa_println!("    #m   m# ###### ###### #####");
    mesa_println!("    ##m m## #      #      #   #");
    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("    # ### # ####   #####  #####");
    mesa_println!("    #  #  # #          #  #   #");
    drivers::framebuffer::set_color(palette::GOLD);
    mesa_println!("    #     # ###### #####  #   #");

    mesa_println!();
    drivers::framebuffer::set_color(palette::SUBTLE);
    mesa_println!("         O P E R A T I N G   S Y S T E M");
    mesa_println!();
    drivers::framebuffer::set_color(palette::MUTED);
    mesa_println!("           \"Minimalismo con Libertad\"");
    mesa_println!();
    mesa_println!("  hda       - Estado de Audio y Reproducción");
    mesa_println!("  usb       - Estado de controladores y puertos USB");
    mesa_println!("  echo      - Imprime texto");
    mesa_println!();
    mesa_println!("              Version 0.1.0 Alpha");

    mesa_println!();
    mesa_print!("         ");
    let colors = [
        palette::LOVE,
        palette::GOLD,
        palette::ROSE,
        palette::PINE,
        palette::FOAM,
        palette::IRIS,
        palette::SUCCESS,
        palette::TEXT,
    ];
    for color in colors {
        drivers::framebuffer::set_color(color);
        mesa_print!("####");
    }
    mesa_println!();
    mesa_println!();
    drivers::framebuffer::set_color(palette::TEXT);
}

fn cmd_net(args: &[&str]) {
    if !args.is_empty() {
        if args[0] == "auto" {
            drivers::framebuffer::set_color(palette::FOAM);
            mesa_println!(
                "{}",
                t(
                    "Iniciando autoconfiguracion DHCP...",
                    "Starting DHCP autoconfiguration..."
                )
            );
            drivers::framebuffer::set_color(palette::TEXT);

            if let Err(e) = net::dhcp::send_discover() {
                print_error(&format!("Error DHCP: {}", e));
            }
            return;
        }

        if args[0] == "save" {
            let name = if args.len() > 1 { args[1] } else { "default" };
            let method = if args.len() > 2 { args[2] } else { "static" };
            match net::config::save_profile(name, method) {
                Ok(_) => print_success(&format!(
                    "{} '{}' {}",
                    t("Perfil", "Profile"),
                    name,
                    t("guardado", "saved")
                )),
                Err(e) => print_error(&format!("{}: {}", t("Error", "Error"), e)),
            }
            return;
        }

        if args[0] == "load" {
            let name = if args.len() > 1 { args[1] } else { "default" };
            match net::config::load_profile(name) {
                Ok(_) => print_success(&format!("Perfil '{}' cargado", name)),
                Err(e) => print_error(&format!("Error: {}", e)),
            }
            return;
        }

        if args[0] == "list" {
            print_section(t("Perfiles de Red", "Network Profiles"));
            mesa_println!();
            let profiles = net::config::list_profiles();
            if profiles.is_empty() {
                mesa_println!(
                    "  ({})",
                    t("No hay perfiles guardados", "No saved profiles")
                );
            } else {
                for profile in profiles {
                    mesa_println!("  • {}", profile);
                }
            }
            mesa_println!();
            return;
        }

        if args[0] == "scan" {
            drivers::framebuffer::set_color(palette::FOAM);
            mesa_println!(
                "{}",
                t(
                    "Escaneando red local (ARP Scan)...",
                    "Scanning local network (ARP Scan)..."
                )
            );
            drivers::framebuffer::set_color(palette::TEXT);

            // Perform scan
            net::arp::scan_neighbors();

            mesa_println!();
            mesa_println!("Escaneo completado. Usa 'arp' para ver dispositivos detectados.");
            return;
        }
    }
    print_section("Informacion de Red");
    mesa_println!();

    if let Some(device) = pci::find_device(0x10EC, 0x8139) {
        print_info_line("Controlador", "Realtek RTL8139");
        print_info_line("PCI Bus", &format!("{}", device.bus));
        print_info_line("PCI Device", &format!("{}", device.device));
        print_info_line("Estado", "Inicializado");

        // Show MAC
        if let Some(mac) = drivers::net::rtl8139::get_mac() {
            print_info_line(
                "MAC Address",
                &format!(
                    "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                ),
            );
        }

        // Show IP config
        let ip = net::get_ip();
        let gw = net::get_gateway();
        print_info_line(
            "IP Address",
            &format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]),
        );
        print_info_line(
            "Gateway",
            &format!("{}.{}.{}.{}", gw[0], gw[1], gw[2], gw[3]),
        );

        mesa_println!();
        print_info("Stack TCP/IP: Funcional (ARP, IP, ICMP)");
    } else {
        print_error("No se detecto controlador RTL8139");
        mesa_println!();
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("  Usa: ./build.sh run (ahora incluye red)");
        drivers::framebuffer::set_color(palette::TEXT);
    }
    mesa_println!();
}

#[cfg(target_arch = "x86_64")]
fn cmd_ifconfig(args: &[&str]) {
    if args.len() < 3 {
        let ip = net::get_ip();
        let gw = net::get_gateway();
        print_section(t("Configuracion de Red", "Network Configuration"));
        mesa_println!();
        print_info_line("IP", &format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]));
        print_info_line(
            "Gateway",
            &format!("{}.{}.{}.{}", gw[0], gw[1], gw[2], gw[3]),
        );
        mesa_println!();
        print_info(&format!(
            "{}: ifconfig <ip> <netmask> <gateway>",
            t("Uso", "Usage")
        ));
        print_info(&format!(
            "{}: ifconfig 10.0.2.15 255.255.255.0 10.0.2.2",
            t("Ejemplo", "Example")
        ));
        mesa_println!();
        return;
    }

    // Parse IP address
    let ip_parts: Vec<&str> = args[0].split('.').collect();
    if ip_parts.len() != 4 {
        print_error("IP invalida");
        return;
    }
    let ip = [
        ip_parts[0].parse().unwrap_or(0),
        ip_parts[1].parse().unwrap_or(0),
        ip_parts[2].parse().unwrap_or(0),
        ip_parts[3].parse().unwrap_or(0),
    ];

    // Parse netmask
    let mask_parts: Vec<&str> = args[1].split('.').collect();
    if mask_parts.len() != 4 {
        print_error("Netmask invalida");
        return;
    }
    let netmask = [
        mask_parts[0].parse().unwrap_or(0),
        mask_parts[1].parse().unwrap_or(0),
        mask_parts[2].parse().unwrap_or(0),
        mask_parts[3].parse().unwrap_or(0),
    ];

    // Parse gateway
    let gw_parts: Vec<&str> = args[2].split('.').collect();
    if gw_parts.len() != 4 {
        print_error("Gateway invalido");
        return;
    }
    let gateway = [
        gw_parts[0].parse().unwrap_or(0),
        gw_parts[1].parse().unwrap_or(0),
        gw_parts[2].parse().unwrap_or(0),
        gw_parts[3].parse().unwrap_or(0),
    ];

    net::configure(ip, netmask, gateway);
    print_success("Red configurada");
    mesa_println!();
}

#[cfg(target_arch = "x86_64")]
fn cmd_ping(args: &[&str]) {
    if args.is_empty() {
        print_error(&format!("{}: ping <ip>", t("Uso", "Usage")));
        print_info(&format!("{}: ping 8.8.8.8", t("Ejemplo", "Example")));
        mesa_println!();
        return;
    }

    // Resolve hostname or parse IP (resolve handles both now)
    let dest_ip = if let Some(ip) = net::dns::resolve(args[0]) {
        ip
    } else {
        print_error(t(
            "No se pudo resolver el host o IP",
            "Could not resolve host or IP",
        ));
        return;
    };

    mesa_println!();
    let header = format!(
        "PING {}.{}.{}.{} ({}.{}.{}.{}): 56 data bytes",
        dest_ip[0],
        dest_ip[1],
        dest_ip[2],
        dest_ip[3],
        dest_ip[0],
        dest_ip[1],
        dest_ip[2],
        dest_ip[3]
    );
    print_info(&header);

    // Limpiar teclado para evitar detecciones falsas
    crate::drivers::keyboard::clear_buffer();

    let mut seq = 0;
    loop {
        // Verificar si el usuario quiere parar (Cualquier tecla o Ctrl+C)
        if crate::drivers::keyboard::has_events() {
            if let Some(event) = crate::drivers::keyboard::read_event() {
                match event {
                    crate::drivers::keyboard::KeyEvent::Special(
                        crate::drivers::keyboard::SpecialKey::CtrlC,
                    ) => break,
                    _ => break, // Parar con cualquier tecla por ahora
                }
            }
        }

        let start_tick = curr_arch::get_ticks();

        if let Err(e) = net::icmp::send_ping(dest_ip, 1, seq as u16) {
            print_error(&format!("Error enviando ping: {}", e));
        } else {
            // Esperar respuesta (máximo 1 segundo)
            if let Some((reply_ip, ttl)) = net::icmp::wait_for_reply(1, seq as u16, 1000) {
                let end_tick = curr_arch::get_ticks();
                let rtt_ticks = end_tick.wrapping_sub(start_tick);
                let rtt_ms = rtt_ticks * 55; // Aproximación 18.2 Hz -> 55ms por tick

                mesa_println!(
                    "64 bytes from {}.{}.{}.{}: icmp_seq={} ttl={} time={} ms",
                    reply_ip[0],
                    reply_ip[1],
                    reply_ip[2],
                    reply_ip[3],
                    seq,
                    ttl,
                    rtt_ms
                );
            } else {
                drivers::framebuffer::set_color(palette::GOLD);
                mesa_println!("Request timeout for icmp_seq {}", seq);
                drivers::framebuffer::set_color(palette::TEXT);
            }
        }

        seq += 1;

        // Esperar hasta completar 1 segundo (18-19 ticks)
        let wait_until = start_tick + 18;
        while curr_arch::get_ticks() < wait_until {
            crate::net::poll(); // Seguir procesando red mientras esperamos
            core::hint::spin_loop();

            // Permitir salir durante la espera
            if crate::drivers::keyboard::has_events() {
                break;
            }
        }

        if crate::drivers::keyboard::has_events() {
            break;
        }
    }

    mesa_println!(
        "\n--- {}.{}.{}.{} ping statistics ---",
        dest_ip[0],
        dest_ip[1],
        dest_ip[2],
        dest_ip[3]
    );
}

fn cmd_curl(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: curl <url> [archivo_destino]");
        return;
    }

    let url = args[0];
    let output_file = args.get(1).copied();
    let hostname = if url.starts_with("http://") {
        &url[7..]
    } else {
        url
    };

    // DNS Resolve
    mesa_print!("Resolviendo {}... ", hostname);
    let dest_ip = if let Some(ip) = net::dns::resolve(hostname) {
        mesa_println!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
        ip
    } else {
        mesa_println!();
        print_error("No se pudo resolver el host");
        return;
    };

    // TCP Connect (Handshake)
    mesa_print!(
        "Conectando a {}.{}.{}.{}:80... ",
        dest_ip[0],
        dest_ip[1],
        dest_ip[2],
        dest_ip[3]
    );
    let src_port = 49152 + (curr_arch::get_ticks() % 1000) as u16;
    let mut seq = 100;

    // SYN
    let syn = net::tcp::TcpHeader::new(src_port, 80, seq, 0, net::tcp::TCP_FLAG_SYN);
    let _ = net::tcp::send_tcp_packet(dest_ip, syn, &[]);

    // Wait for SYN-ACK
    let mut ack_num = 0;
    let mut success = false;
    unsafe {
        net::tcp::LAST_TCP_PACKETS.clear();
    }

    let start_wait = curr_arch::get_ticks();
    while curr_arch::get_ticks().wrapping_sub(start_wait) < 54 {
        // 3s
        net::poll();
        let packets = unsafe { core::mem::take(&mut net::tcp::LAST_TCP_PACKETS) };
        for (header, _, _) in packets {
            if header.dest_port == src_port
                && (header.flags & net::tcp::TCP_FLAG_SYN != 0)
                && (header.flags & net::tcp::TCP_FLAG_ACK != 0)
            {
                ack_num = header.seq_num + 1;
                seq += 1;
                success = true;
                break;
            }
        }
        if success {
            break;
        }
        core::hint::spin_loop();
    }

    if !success {
        mesa_println!();
        print_error("Error: Timeout en handshake TCP");
        return;
    }

    mesa_println!("Conectado.");

    // ACK
    let ack = net::tcp::TcpHeader::new(src_port, 80, seq, ack_num, net::tcp::TCP_FLAG_ACK);
    let _ = net::tcp::send_tcp_packet(dest_ip, ack, &[]);

    // Send GET
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        hostname
    );
    let push = net::tcp::TcpHeader::new(
        src_port,
        80,
        seq,
        ack_num,
        net::tcp::TCP_FLAG_ACK | net::tcp::TCP_FLAG_PSH,
    );
    let _ = net::tcp::send_tcp_packet(dest_ip, push, request.as_bytes());
    seq += request.len() as u32;

    mesa_println!("Peticion enviada. Esperando respuesta... (Ctrl+C para cancelar)\n");

    // Receive Response
    let mut received_fin = false;
    let mut downloaded_data = Vec::new();

    loop {
        net::poll();
        let packets = unsafe { core::mem::take(&mut net::tcp::LAST_TCP_PACKETS) };

        for (header, payload, _) in packets {
            if header.dest_port == src_port {
                if !payload.is_empty() {
                    if output_file.is_some() {
                        downloaded_data.extend_from_slice(&payload);
                    } else {
                        for &b in &payload {
                            mesa_print!("{}", b as char);
                        }
                    }

                    // ACK the data
                    ack_num += payload.len() as u32;
                    let ack = net::tcp::TcpHeader::new(
                        src_port,
                        80,
                        seq,
                        ack_num,
                        net::tcp::TCP_FLAG_ACK,
                    );
                    let _ = net::tcp::send_tcp_packet(dest_ip, ack, &[]);
                }

                if header.flags & net::tcp::TCP_FLAG_FIN != 0 {
                    received_fin = true;
                }
            }
        }

        if received_fin {
            break;
        }

        core::hint::spin_loop();

        // Salir si el usuario presiona Ctrl+C
        if crate::drivers::keyboard::has_events() {
            if let Some(crate::drivers::keyboard::KeyEvent::Special(
                crate::drivers::keyboard::SpecialKey::CtrlC,
            )) = crate::drivers::keyboard::read_event()
            {
                mesa_println!("\n[Cancelado por el usuario]");
                break;
            }
        }
    }

    // Guardar a archivo si se solicitó
    if let Some(filename) = output_file {
        match fs::write(filename, &downloaded_data) {
            Ok(()) => print_success(&format!(
                "Guardado {} bytes en '{}'",
                downloaded_data.len(),
                filename
            )),
            Err(e) => print_error(&format!("Error guardando archivo: {}", e.as_str())),
        }
    }

    mesa_println!("\n--- Fin de transmision ---");
}

fn cmd_ip() {
    let ip = net::get_ip();
    let mac = net::get_mac();
    let mask = net::get_netmask();
    let gw = net::get_gateway();

    print_section("Configuracion de Red");
    mesa_println!();

    print_info(&format!("Interfaz: virtio-net (enp0s3)"));
    mesa_println!("  Direccion IP:  {}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
    mesa_println!(
        "  Mascara:       {}.{}.{}.{}",
        mask[0],
        mask[1],
        mask[2],
        mask[3]
    );
    mesa_println!("  Gateway:       {}.{}.{}.{}", gw[0], gw[1], gw[2], gw[3]);
    mesa_println!(
        "  Direccion MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0],
        mac[1],
        mac[2],
        mac[3],
        mac[4],
        mac[5]
    );

    let status = if net::is_virtio() {
        "LINK UP"
    } else {
        "LINK DOWN"
    };
    drivers::framebuffer::set_color(palette::SUCCESS);
    mesa_println!("\n  Estado:        {}", status);
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
}

fn cmd_nano(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: nano <archivo>");
        return;
    }

    // Reconstruir filename con espacios: cada argumento individual es parte
    // del path. Soporta backslash escaping y comillas (parse_args ya las maneja).
    let filename = args.join(" ");
    // Procesar backslash escaping en el filename
    let filename = {
        let mut path_buf = String::with_capacity(filename.len());
        let mut chars = filename.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some(' ') => path_buf.push(' '),
                    Some('\\') => path_buf.push('\\'),
                    Some(next) => {
                        path_buf.push('\\');
                        path_buf.push(next);
                    }
                    None => path_buf.push('\\'),
                }
            } else {
                path_buf.push(c);
            }
        }
        path_buf
    };
    let mut lines: Vec<String> = Vec::new();

    // Cargar archivo si existe
    if let Ok(content) = fs::read_to_string(&filename) {
        for line in content.lines() {
            lines.push(String::from(line));
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    let (fb_ptr, width, height, pitch, bpp) = drivers::framebuffer::get_info();
    let ui = drivers::framebuffer::ui::UiRenderer::new(fb_ptr, width, height, pitch, bpp);

    let mut cursor_x = 0;
    let mut cursor_y = 0;
    let mut scroll_y = 0;
    let mut modified = false;
    let mut needs_redraw = true;

    // Bloquear consola para que no interfiera
    drivers::framebuffer::lock();
    drivers::keyboard::clear_buffer();

    loop {
        if needs_redraw {
            // --- DIBUJAR ---
            ui.fill_rect(0, 0, width, height, palette::BASE);
            ui.draw_top_bar(&format!("Mesa Nano 1.0 - {}", filename));

            let area_y = 28 + 8;
            let area_h = height - 28 - 28 - 16;
            let max_rows = area_h / 20;

            // Dibujar líneas visibles
            for i in 0..max_rows {
                let idx = i + scroll_y;
                if idx < lines.len() {
                    ui.draw_text(10, area_y + i * 20, &lines[idx], palette::TEXT);
                }
            }

            // Dibujar cursor
            if cursor_y >= scroll_y && cursor_y < scroll_y + max_rows {
                let vis_y = area_y + (cursor_y - scroll_y) * 20;
                ui.fill_rect(10 + cursor_x * 8, vis_y + 16, 8, 2, palette::GOLD);
            }

            // Barra inferior
            let footer_y = height - 28;
            ui.fill_rect(0, footer_y, width, 28, palette::OVERLAY);
            ui.draw_text(
                10,
                footer_y + 6,
                "^X Salir  ^S Guardar  ^C Cancelar",
                palette::SUBTLE,
            );
            if modified {
                ui.draw_text(width - 120, footer_y + 6, "[MODIFICADO]", palette::GOLD);
            }

            needs_redraw = false;
        }

        // --- TECLADO (Bloqueante simulado) ---
        if let Some(event) = drivers::keyboard::read_event() {
            needs_redraw = true;
            match event {
                drivers::keyboard::KeyEvent::Char(c) => {
                    if cursor_x < 200 {
                        lines[cursor_y].insert(cursor_x, c);
                        cursor_x += 1;
                        modified = true;
                    }
                }
                drivers::keyboard::KeyEvent::Special(key) => {
                    use drivers::keyboard::SpecialKey::*;
                    match key {
                        CtrlX => break,
                        CtrlS => {
                            let mut full_text = String::new();
                            for (i, line) in lines.iter().enumerate() {
                                full_text.push_str(line);
                                if i < lines.len() - 1 {
                                    full_text.push('\n');
                                }
                            }
                            if fs::write(&filename, full_text.as_bytes()).is_ok() {
                                modified = false;
                            }
                        }
                        Enter => {
                            let remaining = lines[cursor_y].split_off(cursor_x);
                            lines.insert(cursor_y + 1, remaining);
                            cursor_y += 1;
                            cursor_x = 0;
                            modified = true;
                        }
                        Backspace => {
                            if cursor_x > 0 {
                                cursor_x -= 1;
                                lines[cursor_y].remove(cursor_x);
                                modified = true;
                            } else if cursor_y > 0 {
                                let current = lines.remove(cursor_y);
                                cursor_y -= 1;
                                cursor_x = lines[cursor_y].len();
                                lines[cursor_y].push_str(&current);
                                modified = true;
                            }
                        }
                        ArrowUp => {
                            if cursor_y > 0 {
                                cursor_y -= 1;
                                cursor_x = cursor_x.min(lines[cursor_y].len());
                            }
                        }
                        ArrowDown => {
                            if cursor_y < lines.len() - 1 {
                                cursor_y += 1;
                                cursor_x = cursor_x.min(lines[cursor_y].len());
                            }
                        }
                        ArrowLeft => {
                            if cursor_x > 0 {
                                cursor_x -= 1;
                            } else if cursor_y > 0 {
                                cursor_y -= 1;
                                cursor_x = lines[cursor_y].len();
                            }
                        }
                        ArrowRight => {
                            if cursor_x < lines[cursor_y].len() {
                                cursor_x += 1;
                            } else if cursor_y < lines.len() - 1 {
                                cursor_y += 1;
                                cursor_x = 0;
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Ajustar scroll
            let area_h = height - 28 - 28 - 16;
            let max_rows = area_h / 20;
            if cursor_y < scroll_y {
                scroll_y = cursor_y;
            } else if cursor_y >= scroll_y + max_rows {
                scroll_y = cursor_y - max_rows + 1;
            }
        } else {
            // Pequeña espera si no hay teclas para no quemar CPU
            for _ in 0..10000 {
                core::hint::spin_loop();
            }
        }
    }

    // Al salir, desbloquear y limpiar
    drivers::framebuffer::unlock();
    drivers::framebuffer::clear();
    drivers::framebuffer::redraw_full();
}

#[cfg(target_arch = "x86_64")]
fn cmd_arp() {
    print_section("Cache ARP");
    mesa_println!();

    let cache = net::arp::get_cache();
    if cache.is_empty() {
        print_info("Cache vacia");
    } else {
        for (ip, mac) in cache {
            mesa_println!(
                "  {}.{}.{}.{} -> {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                ip[0],
                ip[1],
                ip[2],
                ip[3],
                mac[0],
                mac[1],
                mac[2],
                mac[3],
                mac[4],
                mac[5]
            );
        }
    }

    mesa_println!();
}

#[cfg(target_arch = "x86_64")]
fn cmd_scan() {
    print_section("Escaneo de Red Local");
    mesa_println!();

    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("Enviando peticiones ARP a la subred (rango .1 al .50)...");
    drivers::framebuffer::set_color(palette::TEXT);

    net::arp::scan_neighbors();

    mesa_println!();
    print_success("Escaneo completado");
    print_info("Usa 'arp' para ver la lista de dispositivos detectados");
    mesa_println!();
}

fn cmd_html(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: html <archivo.html>");
        mesa_println!();
        return;
    }

    let path = args[0];
    if !fs::exists(path) {
        print_error(&format!("El archivo '{}' no existe", path));
        mesa_println!();
        return;
    }

    drivers::framebuffer::html::render_html(path);
}

#[cfg(target_arch = "x86_64")]
fn cmd_speak(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: speak <texto>");
        mesa_println!();
        return;
    }

    let text = args.join(" ");
    drivers::audio::speak(&text);
}

// ══════════════════════════════════════════════════════════════════════════════
// COMANDO: chmod - Cambiar permisos de ejecución
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_chmod(args: &[&str]) {
    if args.len() < 2 {
        print_error("Uso: chmod <permisos> <archivo>");
        mesa_println!("  Permisos: +x (dar ejecucion), -x (quitar ejecucion)");
        mesa_println!("  Ejemplo: chmod +x /bin/miscript.sh");
        mesa_println!();
        return;
    }

    let mode = args[0];
    let path = args[1];

    // Obtener metadatos actuales
    let meta = match fs::stat(path) {
        Ok(m) => m,
        Err(e) => {
            print_error(&format!("chmod: {}: {}", path, e.as_str()));
            mesa_println!();
            return;
        }
    };

    let mut perms = meta.permissions;

    match mode {
        "+x" => {
            perms.execute = true;
        }
        "-x" => {
            perms.execute = false;
        }
        _ => {
            print_error(&format!("Modo '{}' no soportado. Usa +x o -x", mode));
            mesa_println!();
            return;
        }
    }

    match fs::update_permissions(path, perms) {
        Ok(()) => {
            print_success(&format!("Permisos actualizados: {}", perms.to_string()));
        }
        Err(e) => {
            print_error(&format!("chmod: {}: {}", path, e.as_str()));
        }
    }
    mesa_println!();
}

// ══════════════════════════════════════════════════════════════════════════════
// COMANDO: run - Ejecutar scripts .sh
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_run(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: run <archivo.sh> [args...]");
        mesa_println!();
        return;
    }

    let path = args[0];
    let script_args = if args.len() > 1 { &args[1..] } else { &[] };

    // Verificar que existe
    if !fs::exists(path) {
        print_error(&format!("run: {}: No such file or directory", path));
        mesa_println!();
        return;
    }

    // Leer el contenido
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            print_error(&format!("run: {}: {}", path, e.as_str()));
            mesa_println!();
            return;
        }
    };

    // Dar permisos de ejecución automáticamente
    let _ = fs::update_permissions_exec(path);

    // Verificar shebang #!/bin/sh o similar
    if !content.starts_with("#!") {
        print_error(&format!("run: {}: not a shell script (no shebang)", path));
        mesa_println!();
        return;
    }

    // Ejecutar cada línea del script
    print_success(&format!("Ejecutando script: {}", path));
    mesa_println!();

    // Variables de entorno simuladas
    let mut variables = alloc::collections::BTreeMap::<String, String>::new();
    variables.insert(String::from("?"), String::from("0"));

    for (line_num, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();

        // Saltar líneas vacías, comentarios y shebang
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Mostrar línea que se ejecuta (modo verbose)
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_println!("+ {}", line);
        drivers::framebuffer::set_color(palette::TEXT);

        // Reemplazar variables $1, $2, etc. con argumentos
        let mut processed = String::from(line);
        for (i, arg) in script_args.iter().enumerate() {
            let var = format!("${}", i + 1);
            processed = processed.replace(&var, arg);
        }
        // Reemplazar $@ con todos los args
        if !script_args.is_empty() {
            processed = processed.replace("$@", &script_args.join(" "));
        }
        processed = processed.replace("$*", &script_args.join(" "));
        processed = processed.replace("$0", path);
        processed = processed.replace("$#", &format!("{}", script_args.len()));

        // Reemplazar variables de entorno
        for (key, val) in variables.iter() {
            processed = processed.replace(&format!("${}", key), val);
        }

        // Ejecutar línea como comando
        execute_maybe_piped(&processed);
    }

    mesa_println!();
    print_success(&format!("Script '{}' completado", path));
    mesa_println!();
}

// ══════════════════════════════════════════════════════════════════════════════
// COMANDOS: play / audio-info / audio-list
// Reproductor de audio (apoyado en el driver HDA).
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_play(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: play <archivo> [-l]");
        mesa_println!();
        print_info("Formatos soportados: WAV (PCM 8/16/24/32 bits, mono/estéreo).");
        print_info("MP3: solo se muestra su información (decoder no implementado).");
        print_info("  -l : reproducción en bucle (loop)");
        mesa_println!();
        print_info("Ejemplos:");
        mesa_println!("  play /inyect/audio/sonido.wav");
        mesa_println!("  play /home/root/musica.wav -l");
        mesa_println!("  hda test                    # audio de prueba pre-cargado");
        mesa_println!();
        return;
    }

    let mut path = args[0];
    let mut loop_playback = false;
    for a in &args[1..] {
        if *a == "-l" || *a == "--loop" {
            loop_playback = true;
        } else if !path.is_empty() && path == args[0] {
            // Primer argumento no-flag
        }
    }

    // Si no se proporcionó ningún argumento de path, salir.
    if path.is_empty() {
        print_error("Sintaxis: play <archivo> [-l]");
        mesa_println!();
        return;
    }

    // Comprobar existencia
    match fs::stat(path) {
        Ok(meta) => {
            if meta.node_type == fs::NodeType::Directory {
                print_error(&format!("play: {}: es un directorio", path));
                mesa_println!();
                return;
            }
        }
        Err(e) => {
            print_error(&format!("play: {}: {}", path, e.as_str()));
            mesa_println!();
            return;
        }
    }

    // Mostrar info antes de reproducir
    match audio_player::inspect_file(path) {
        Ok(info) => {
            print_section("Información del archivo de audio");
            mesa_print!("{}", info);
            drivers::framebuffer::set_color(palette::TEXT);
            mesa_println!();
        }
        Err(e) => {
            print_error(&format!("No se pudo inspeccionar: {}", e.as_str()));
            return;
        }
    }

    if loop_playback {
        print_info("Modo loop activado. Pulsa Ctrl+C en otra terminal (o espera) para parar.");
        mesa_println!();
        loop {
            if let Err(e) = audio_player::play_file(path) {
                print_error(&format!("Error en reproducción: {}", e.as_str()));
                return;
            }
        }
    } else {
        match audio_player::play_file(path) {
            Ok(()) => {
                print_success("Reproducción completada");
            }
            Err(e) => {
                print_error(&format!("Error: {}", e.as_str()));
                mesa_println!();
                if matches!(e, audio_player::AudioError::NotPcm) {
                    print_info("Pista: convierte el MP3 a WAV 48kHz/16-bit/estéreo con:");
                    print_info("  ffmpeg -i input.mp3 -ar 48000 -ac 2 -sample_fmt s16 output.wav");
                }
            }
        }
        mesa_println!();
    }
}

fn cmd_audio_info(args: &[&str]) {
    if args.is_empty() {
        print_error("Uso: audio-info <archivo>");
        mesa_println!();
        return;
    }
    let path = args[0];
    match audio_player::inspect_file(path) {
        Ok(info) => {
            print_section(&format!("Audio info: {}", path));
            mesa_print!("{}", info);
            drivers::framebuffer::set_color(palette::TEXT);
            mesa_println!();
        }
        Err(e) => {
            print_error(&format!("audio-info: {}: {}", path, e.as_str()));
        }
    }
}

fn cmd_audio_list(args: &[&str]) {
    let _ = args;
    let dir = "/inyect/audio";
    print_section(&format!("Archivos de audio en {}", dir));
    mesa_println!();

    match fs::readdir(dir) {
        Ok(entries) => {
            if entries.is_empty() {
                drivers::framebuffer::set_color(palette::SUBTLE);
                mesa_println!("  (no hay archivos)");
                drivers::framebuffer::set_color(palette::TEXT);
            } else {
                for entry in entries {
                    if entry.node_type == fs::NodeType::File {
                        let full = alloc::format!("{}/{}", dir, entry.name);
                        mesa_print!("  {} ", entry.name);

                        // Detectar tipo por magic bytes
                        if let Ok(data) = fs::read(&full) {
                            let format = audio_player::detect_format(&data);
                            let dot = match format {
                                audio_player::AudioFormat::Wav => "♪WAV",
                                audio_player::AudioFormat::Mp3 => "♪MP3",
                                audio_player::AudioFormat::Unknown => " ?",
                            };
                            drivers::framebuffer::set_color(palette::FOAM);
                            mesa_print!("{:<8}", dot);
                            drivers::framebuffer::set_color(palette::SUBTLE);
                            mesa_print!("  {} bytes", entry.size);
                        } else {
                            drivers::framebuffer::set_color(palette::SUBTLE);
                            mesa_print!("  ? bytes");
                        }
                        drivers::framebuffer::set_color(palette::TEXT);
                        mesa_println!();
                    }
                }
            }
        }
        Err(e) => {
            print_error(&format!("No se pudo listar {}: {}", dir, e.as_str()));
        }
    }
    mesa_println!();
    print_info("Usa 'audio-info <archivo>' para ver los metadatos detallados.");
    print_info("Usa 'play <archivo>' para reproducir un WAV.");
    mesa_println!();
}

// ══════════════════════════════════════════════════════════════════════════════
// SNAKE — Juego de la serpiente
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_snake() {
    const W: usize = 30;
    const H: usize = 20;
    let mut snake: Vec<(usize, usize)> =
        alloc::vec![(W / 2, H / 2), (W / 2 - 1, H / 2), (W / 2 - 2, H / 2)];
    let mut dir: (i8, i8) = (1, 0);
    let mut next_dir: (i8, i8) = (1, 0);
    let mut food: (usize, usize);
    let mut score: u32 = 0;
    let mut game_over = false;
    let mut tick = 0u64;

    fn place_food(snake: &[(usize, usize)], w: usize, h: usize) -> (usize, usize) {
        let f;
        loop {
            let fx = (crate::curr_arch::get_ticks() as usize ^ 0x9e37) % w;
            let fy = (crate::curr_arch::get_ticks() as usize ^ 0x7a15) % h;
            if !snake.contains(&(fx, fy)) {
                f = (fx, fy);
                break;
            }
        }
        f
    }

    fn draw(
        snake: &[(usize, usize)],
        food: (usize, usize),
        score: u32,
        game_over: bool,
        w: usize,
        h: usize,
    ) {
        drivers::framebuffer::clear();
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_println!("  ╔{}╗", "═".repeat(w * 2 + 1));
        for y in 0..h {
            drivers::framebuffer::set_color(palette::FOAM);
            mesa_print!("  ║ ");
            drivers::framebuffer::set_color(palette::TEXT);
            for x in 0..w {
                if snake[0] == (x, y) {
                    drivers::framebuffer::set_color(palette::GOLD);
                    mesa_print!("██");
                } else if snake[1..].contains(&(x, y)) {
                    drivers::framebuffer::set_color(palette::SUCCESS);
                    mesa_print!("██");
                } else if food == (x, y) {
                    drivers::framebuffer::set_color(palette::LOVE);
                    mesa_print!("()");
                } else {
                    mesa_print!("  ");
                }
            }
            drivers::framebuffer::set_color(palette::FOAM);
            mesa_println!(" ║");
        }
        mesa_println!("  ╚{}╝", "═".repeat(w * 2 + 1));
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!("  Score: {}", score);
        if game_over {
            drivers::framebuffer::set_color(palette::ERROR);
            mesa_println!("  GAME OVER — Press ESC to exit");
        } else {
            drivers::framebuffer::set_color(palette::SUBTLE);
            mesa_println!("  Arrow keys: move | Q: quit");
        }
        drivers::framebuffer::set_color(palette::TEXT);
    }

    food = place_food(&snake, W, H);
    loop {
        while let Some(ev) = drivers::keyboard::read_event() {
            match ev {
                KeyEvent::Special(SpecialKey::ArrowUp) if dir != (0, 1) => next_dir = (0, -1),
                KeyEvent::Special(SpecialKey::ArrowDown) if dir != (0, -1) => next_dir = (0, 1),
                KeyEvent::Special(SpecialKey::ArrowLeft) if dir != (1, 0) => next_dir = (-1, 0),
                KeyEvent::Special(SpecialKey::ArrowRight) if dir != (-1, 0) => next_dir = (1, 0),
                KeyEvent::Special(SpecialKey::Escape) => {
                    drivers::framebuffer::clear();
                    return;
                }
                KeyEvent::Char('q') | KeyEvent::Char('Q') => {
                    drivers::framebuffer::clear();
                    return;
                }
                _ => {}
            }
        }
        if game_over {
            continue;
        }

        tick += 1;
        if tick % 4 != 0 {
            continue;
        }

        dir = next_dir;
        let head = snake[0];
        let nx = (head.0 as isize + dir.0 as isize) as usize;
        let ny = (head.1 as isize + dir.1 as isize) as usize;

        if nx >= W || ny >= H || snake[..snake.len() - 1].contains(&(nx, ny)) {
            game_over = true;
            draw(&snake, food, score, game_over, W, H);
            continue;
        }

        snake.insert(0, (nx, ny));
        if (nx, ny) == food {
            score += 10;
            food = place_food(&snake, W, H);
        } else {
            snake.pop();
        }
        draw(&snake, food, score, game_over, W, H);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CALCULADORA GRÁFICA (BETA)
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_calc() {
    let mut display = String::new();
    let mut result = String::new();

    fn eval(expr: &str) -> Option<f64> {
        // Simple 4-function evaluator using operator precedence
        let expr = expr.trim();
        if expr.is_empty() {
            return None;
        }
        // Tokenize: numbers and operators
        let mut nums: Vec<f64> = Vec::new();
        let mut ops: Vec<char> = Vec::new();
        let mut num_buf = String::new();
        let chars: Vec<char> = expr.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_digit() || c == '.' {
                num_buf.push(c);
            } else if "+-*/".contains(c) {
                if num_buf.is_empty()
                    && c == '-'
                    && i + 1 < chars.len()
                    && (chars[i + 1].is_ascii_digit() || chars[i + 1] == '.')
                {
                    num_buf.push(c);
                } else {
                    if !num_buf.is_empty() {
                        nums.push(num_buf.parse::<f64>().ok()?);
                        num_buf.clear();
                    }
                    // Handle precedence: * and / first
                    while let Some(&last_op) = ops.last() {
                        if (c == '+' || c == '-') && (last_op == '*' || last_op == '/') {
                            let b = nums.pop()?;
                            let a = nums.pop()?;
                            nums.push(apply_op(a, b, ops.pop()?)?);
                        } else {
                            break;
                        }
                    }
                    ops.push(c);
                }
            }
            i += 1;
        }
        if !num_buf.is_empty() {
            nums.push(num_buf.parse::<f64>().ok()?);
        }
        while let Some(op) = ops.pop() {
            if nums.len() < 2 {
                return None;
            }
            let b = nums.pop()?;
            let a = nums.pop()?;
            nums.push(apply_op(a, b, op)?);
        }
        nums.pop()
    }

    fn apply_op(a: f64, b: f64, op: char) -> Option<f64> {
        match op {
            '+' => Some(a + b),
            '-' => Some(a - b),
            '*' => Some(a * b),
            '/' => {
                if b == 0.0 {
                    None
                } else {
                    Some(a / b)
                }
            }
            _ => None,
        }
    }

    loop {
        drivers::framebuffer::clear();
        drivers::framebuffer::set_color(palette::IRIS);
        mesa_println!("  ┌─────────────────────────────┐");
        mesa_println!("  │      CALCULADORA (BETA)     │");
        mesa_println!("  └─────────────────────────────┘");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();

        // Display
        drivers::framebuffer::set_color(palette::SURFACE);
        mesa_println!("  ┌─────────────────────────────┐");
        mesa_print!("  │ ");
        drivers::framebuffer::set_color(palette::TEXT);
        let disp = if result.is_empty() { &display } else { &result };
        let padded = format!("{:>27}", disp);
        mesa_print!("{}", padded);
        drivers::framebuffer::set_color(palette::SURFACE);
        mesa_println!(" │");
        mesa_println!("  └─────────────────────────────┘");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();

        // Buttons
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("  ┌────┬────┬────┬────┐");
        mesa_println!("  │ 7  │ 8  │ 9  │  / │");
        mesa_println!("  ├────┼────┼────┼────┤");
        mesa_println!("  │ 4  │ 5  │ 6  │  * │");
        mesa_println!("  ├────┼────┼────┼────┤");
        mesa_println!("  │ 1  │ 2  │ 3  │  - │");
        mesa_println!("  ├────┼────┼────┼────┤");
        mesa_println!("  │ 0  │ .  │ C  │  + │");
        mesa_println!("  ├────┼────┼────┼────┤");
        mesa_println!("  │    │    │    │    │");
        mesa_print!("  │ ");
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_print!("=");
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_print!("   │ ");
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_print!("DEL");
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_print!("  │AC  │    │");
        mesa_println!();
        mesa_println!("  └────┴────┴────┴────┘");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("  Type numbers/operators, Enter=eval, ESC=exit");
        drivers::framebuffer::set_color(palette::TEXT);

        // Input
        mesa_print!("  > ");
        let mut line = String::new();
        read_line_simple(&mut line);
        let line = String::from(line.trim());

        if line == "exit" || line.is_empty() {
            // check for ESC in read_line_simple
        }

        // Check for key event (ESC)
        let mut exit = false;
        while let Some(ev) = drivers::keyboard::read_event() {
            if let KeyEvent::Special(SpecialKey::Escape) = ev {
                exit = true;
            }
        }
        if exit {
            break;
        }

        if line == "c" || line == "C" || line == "clear" || line == "AC" {
            display.clear();
            result.clear();
            continue;
        }

        // Accumulate input or evaluate
        if line.contains('=') || line.chars().any(|c| "+-*/".contains(c)) {
            let clean = line.replace('=', "");
            result = match eval(&clean) {
                Some(v) => {
                    if (v as i64) as f64 == v && v.is_finite() {
                        format!("= {}", v as i64)
                    } else {
                        format!("= {:.4}", v)
                    }
                }
                None => {
                    drivers::framebuffer::set_color(palette::ERROR);
                    String::from("  Error")
                }
            };
            display.push_str(&line);
        } else {
            display.push_str(&line);
        }
    }
    drivers::framebuffer::clear();
}

// ══════════════════════════════════════════════════════════════════════════════
// CALENDARIO
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_calendar() {
    fn get_days_in_month(year: u16, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }

    fn day_of_week(year: u16, month: u8, day: u8) -> u8 {
        let (y, m) = if month <= 2 {
            (year - 1, month + 12)
        } else {
            (year, month)
        };
        let c = y / 100;
        let y = y % 100;
        ((day as u16 + (13 * (m as u16 + 1)) / 5 + y + y / 4 + c / 4 + 5 * c) % 7) as u8
    }

    let now = drivers::rtc::read();
    let mut year = now.year;
    let mut month = now.month;

    let month_names = [
        "",
        "Enero",
        "Febrero",
        "Marzo",
        "Abril",
        "Mayo",
        "Junio",
        "Julio",
        "Agosto",
        "Septiembre",
        "Octubre",
        "Noviembre",
        "Diciembre",
    ];

    loop {
        drivers::framebuffer::clear();
        drivers::framebuffer::set_color(palette::IRIS);
        mesa_println!("  ┌─────────────────────────────┐");
        mesa_println!(
            "  │       {:>2} / {:04}          │",
            month_names[month as usize],
            year
        );
        mesa_println!("  └─────────────────────────────┘");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();

        drivers::framebuffer::set_color(palette::FOAM);
        mesa_println!("        Lu  Ma  Mi  Ju  Vi  Sa  Do");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_print!("    ");

        let days = get_days_in_month(year, month);
        let start_dow = day_of_week(year, month, 1);
        // In European convention Monday=0, shift to have Monday as first column
        let start = (start_dow + 6) % 7;

        for _ in 0..start {
            mesa_print!("    ");
        }

        for d in 1..=days {
            if d == now.day && month == now.month && year == now.year {
                drivers::framebuffer::set_color(palette::GOLD);
                mesa_print!("{:>3} ", d);
                drivers::framebuffer::set_color(palette::TEXT);
            } else {
                mesa_print!("{:>3} ", d);
            }
            if (start + d as u8) % 7 == 0 && d != days {
                mesa_println!();
                mesa_print!("    ");
            }
        }
        mesa_println!();
        mesa_println!();
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("  ← → : navigate month  |  T : today  |  ESC : exit");
        drivers::framebuffer::set_color(palette::TEXT);

        // Wait for key
        loop {
            crate::scheduler::yield_now();
            if let Some(ev) = drivers::keyboard::read_event() {
                match ev {
                    KeyEvent::Special(SpecialKey::ArrowLeft) => {
                        if month == 1 {
                            month = 12;
                            year -= 1;
                        } else {
                            month -= 1;
                        }
                        break;
                    }
                    KeyEvent::Special(SpecialKey::ArrowRight) => {
                        if month == 12 {
                            month = 1;
                            year += 1;
                        } else {
                            month += 1;
                        }
                        break;
                    }
                    KeyEvent::Char('t') | KeyEvent::Char('T') => {
                        let n = drivers::rtc::read();
                        year = n.year;
                        month = n.month;
                        break;
                    }
                    KeyEvent::Special(SpecialKey::Escape) => {
                        drivers::framebuffer::clear();
                        return;
                    }
                    _ => {}
                }
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// RELOJ MUNDIAL
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_clock() {
    fn add_hours(h: u8, m: u8, s: u8, offset: i8) -> (u8, u8, u8, bool) {
        let total = (h as i32) + (offset as i32);
        let mut nh = total % 24;
        if nh < 0 {
            nh += 24;
        }
        (nh as u8, m, s, offset >= 0)
    }

    let zones: &[(&str, i8)] = &[
        ("🌴 Honolulu  ", -10),
        ("🌉 San Francisco", -7),
        ("🏛️  México    ", -6),
        ("🗽 New York   ", -5),
        ("🇧🇷 Brasília  ", -3),
        ("🇬🇧 London    ", 0),
        ("🇪🇸 Madrid    ", 1),
        ("🇪🇺 Berlin    ", 1),
        ("🇷🇺 Moscow    ", 3),
        ("🇮🇳 India     ", 5),
        ("🇯🇵 Tokyo     ", 9),
        ("🇦🇺 Sydney    ", 11),
    ];

    loop {
        let now = drivers::rtc::read();
        // Read raw UTC
        let utc;
        if drivers::rtc::is_summer_time() {
            utc = (now.hour as i16 - 2) % 24;
        } else {
            utc = (now.hour as i16 - 1) % 24;
        }
        let utc = if utc < 0 { (utc + 24) as u8 } else { utc as u8 };

        drivers::framebuffer::clear();
        drivers::framebuffer::set_color(palette::IRIS);
        mesa_println!("  ┌─────────────────────────────────┐");
        mesa_println!("  │         R E L O J   M U N D I A L        │");
        mesa_println!("  └─────────────────────────────────┘");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!();
        mesa_println!("  Local: {}\n", now.format());

        for (name, offset) in zones {
            let total_h = (utc as i16 + *offset as i16) % 24;
            let h = if total_h < 0 {
                (total_h + 24) as u8
            } else {
                total_h as u8
            };
            drivers::framebuffer::set_color(palette::SUBTLE);
            mesa_print!("  {} ", name);
            drivers::framebuffer::set_color(palette::TEXT);
            mesa_println!("  {:02}:{:02}:{:02}", h, now.minute, now.second);
        }

        mesa_println!();
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_print!("  UTC: ");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!("{:02}:{:02}:{:02}", utc, now.minute, now.second);
        mesa_println!();
        drivers::framebuffer::set_color(palette::SUBTLE);
        mesa_println!("  Press ESC to exit");
        drivers::framebuffer::set_color(palette::TEXT);

        // Wait ~1 second while checking for ESC
        for _ in 0..18000 {
            crate::scheduler::yield_now();
            if let Some(KeyEvent::Special(SpecialKey::Escape)) = drivers::keyboard::read_event() {
                drivers::framebuffer::clear();
                return;
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// DESKTOP — Entorno gráfico beta con ratón
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_desktop() {
    use drivers::framebuffer::ui::UiRenderer;
    use drivers::mouse::{MouseButton, MouseEvent};

    let (fb_ptr, width, height, pitch, bpp) = drivers::framebuffer::get_info();
    let ui = UiRenderer::new(fb_ptr, width, height, pitch, bpp);

    // App buttons: name, x, y, w, h
    let apps: &[(&str, usize, usize, usize, usize)] = &[
        ("Terminal", 50, 80, 140, 100),
        ("Snake", 210, 80, 140, 100),
        ("Calc", 370, 80, 140, 100),
        ("Calendar", 530, 80, 140, 100),
        ("Clock", 690, 80, 140, 100),
    ];

    drivers::mouse::init();
    let mut cursor = drivers::framebuffer::cursor::Cursor::new(width, height);
    ui.draw_background();
    ui.draw_top_bar("Mesa OS Desktop (BETA)");

    for (name, x, y, w, h) in apps {
        ui.draw_card(*x, *y, *w, *h, Some(name));
        let icon_cx = x + w / 2;
        let icon_cy = y + h / 2 + 8;
        ui.fill_rounded_rect(
            icon_cx - 16,
            icon_cy - 16,
            32,
            32,
            6,
            Color::new(49, 116, 143),
        );
    }

    let taskbar_y = height - 40;
    ui.fill_rect(0, taskbar_y, width, 40, Color::new(20, 18, 30));
    ui.draw_text(
        12,
        taskbar_y + 12,
        "Mesa OS Desktop  •  ESC to exit",
        Color::new(144, 140, 170),
    );

    let mut running = true;
    while running {
        cursor.draw(&ui);

        loop {
            let mut processed = false;
            if let Some(ev) = drivers::mouse::read_event() {
                match ev {
                    MouseEvent::Move(dx, dy) => {
                        cursor.erase(&ui);
                        cursor.move_by(dx, dy);
                        cursor.draw(&ui);
                        processed = true;
                    }
                    MouseEvent::ButtonDown(MouseButton::Left) => {
                        let (mcx, mcy) = cursor.pos();
                        for (name, x, y, w, h) in apps {
                            if mcx >= *x as i32
                                && mcx < (*x + *w) as i32
                                && mcy >= *y as i32
                                && mcy < (*y + *h) as i32
                            {
                                launch_app(name);
                                // Redraw desktop after app closes
                                ui.draw_background();
                                ui.draw_top_bar("Mesa OS Desktop (BETA)");
                                for (n2, x2, y2, w2, h2) in apps {
                                    ui.draw_card(*x2, *y2, *w2, *h2, Some(n2));
                                    let icx = x2 + w2 / 2;
                                    let icy = y2 + h2 / 2 + 8;
                                    ui.fill_rounded_rect(
                                        icx - 16,
                                        icy - 16,
                                        32,
                                        32,
                                        6,
                                        Color::new(49, 116, 143),
                                    );
                                }
                                ui.fill_rect(0, taskbar_y, width, 40, Color::new(20, 18, 30));
                                ui.draw_text(
                                    12,
                                    taskbar_y + 12,
                                    "Mesa OS Desktop  •  ESC to exit",
                                    Color::new(144, 140, 170),
                                );
                                cursor = drivers::framebuffer::cursor::Cursor::new(width, height);
                                processed = true;
                                break;
                            }
                        }
                    }
                    _ => {
                        processed = true;
                    }
                }
            }
            if let Some(KeyEvent::Special(SpecialKey::Escape)) = drivers::keyboard::read_event() {
                running = false;
                break;
            }
            if !processed && !drivers::mouse::has_events() && !drivers::keyboard::has_events() {
                break;
            }
        }

        crate::scheduler::yield_now();
    }

    drivers::framebuffer::clear();
    ui.draw_background();
    ui.draw_top_bar("Mesa OS");
    drivers::framebuffer::console::redraw_full();
}

fn launch_app(name: &str) {
    match name {
        "Terminal" => {
            drivers::framebuffer::clear();
            drivers::framebuffer::console::redraw_full();
            mesa_println!("Terminal — type 'exit' to return to desktop");
            loop {
                mesa_print!("$ ");
                let mut line = String::new();
                read_line_simple(&mut line);
                let trimmed = line.trim();
                if trimmed == "exit" || trimmed == "quit" || trimmed == "q" {
                    break;
                }
                if !trimmed.is_empty() {
                    execute_command(trimmed);
                }
            }
        }
        "Snake" => cmd_snake(),
        "Calc" => cmd_calc(),
        "Calendar" => cmd_calendar(),
        "Clock" => cmd_clock(),
        _ => {}
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// HELPERS
// ══════════════════════════════════════════════════════════════════════════════

fn print_success(msg: &str) {
    drivers::framebuffer::set_color(palette::SUCCESS);
    mesa_print!("[OK] ");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}", msg);
}

fn print_error(msg: &str) {
    drivers::framebuffer::set_color(palette::ERROR);
    mesa_print!("[ERROR] ");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}", msg);
}

fn print_warning(msg: &str) {
    drivers::framebuffer::set_color(palette::GOLD);
    mesa_print!("[WARN] ");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}", msg);
}

fn print_info(msg: &str) {
    drivers::framebuffer::set_color(palette::FOAM);
    mesa_print!("[INFO] ");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}", msg);
}

fn print_section(title: &str) {
    drivers::framebuffer::set_color(palette::IRIS);
    mesa_println!("=== {} ===", title);
    drivers::framebuffer::set_color(palette::TEXT);
}

fn print_info_line(label: &str, value: &str) {
    drivers::framebuffer::set_color(palette::FOAM);
    mesa_print!("  {:<14}", label);
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}", value);
}

fn print_system_info() {
    drivers::framebuffer::set_color(palette::FOAM);
    mesa_println!("[Sistema]");
    drivers::framebuffer::set_color(palette::TEXT);

    if let Some((phys, virt)) = limine_req::kernel_address() {
        mesa_println!("  Kernel: {:#x} -> {:#x}", phys, virt);
    }
    mesa_println!("  CPUs:   {}", limine_req::cpu_count());

    if let Some(entries) = limine_req::memory_map_entries() {
        let total: u64 = entries
            .iter()
            .filter(|e| e.entry_type == EntryType::USABLE)
            .map(|e| e.length)
            .sum();
        mesa_println!("  RAM:    {} MB", total / 1024 / 1024);
    }
    mesa_println!();
}

fn print_ok(name: &str) {
    drivers::framebuffer::set_color(palette::SUCCESS);
    mesa_print!("  [OK] ");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}", name);
}

fn print_err(msg: &str) {
    drivers::framebuffer::set_color(palette::ERROR);
    mesa_print!("  [!!] ");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!("{}", msg);
}

fn halt() -> ! {
    loop {
        curr_arch::halt();
    }
}

/// Función de detección específica para HP Laptop 15s-eq2xxx
#[cfg(target_arch = "x86_64")]
fn detect_hp_laptop_15s() {
    // Detectar si estamos en hardware HP específico
    let is_hp_hardware = if let Some(acpi_info) = acpi::get_info() {
        // Verificar OEM ID específico de HP
        acpi_info.oem_id.contains("HP") || acpi_info.oem_id.contains("Hewlett")
    } else {
        false
    };

    if is_hp_hardware {
        drivers::framebuffer::set_color(palette::SUCCESS);
        mesa_print!("[HP] ");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!("HP Laptop 15s-eq2xxx detectado - Aplicando optimizaciones");
        serial_println!("[HP] HP Laptop 15s-eq2xxx detected - Applying optimizations");

        // Aquí podríamos agregar configuraciones específicas para HP
        // como ajustes de energía, configuraciones de pantalla, etc.
    } else {
        drivers::framebuffer::set_color(palette::FOAM);
        mesa_print!("[HW] ");
        drivers::framebuffer::set_color(palette::TEXT);
        mesa_println!("Hardware genérico detectado");
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    curr_arch::disable_interrupts();

    serial_println!();
    serial_println!("!!! KERNEL PANIC !!!");
    serial_println!("{}", info);

    drivers::framebuffer::set_color(palette::ERROR);
    mesa_println!();
    mesa_println!("============================================");
    mesa_println!("             KERNEL PANIC");
    mesa_println!("============================================");
    drivers::framebuffer::set_color(palette::TEXT);
    mesa_println!();
    mesa_println!("{}", info);

    halt();
}
