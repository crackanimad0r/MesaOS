/**
 * UEFI NVRAM Shell Commands for MesaOS Kernel
 *
 * Provides shell commands to test and use UEFI variable storage.
 * These functions are called from main.rs where the mesa_println
 * macros are available.
 */
use crate::arch::x86_64::uefi_nvram::*;

/// Command: nvram_set <name> <value> - Store a string variable in NVRAM
pub fn cmd_nvram_set(args: &[&str]) {
    if args.len() < 2 {
        crate::mesa_println!("Uso: nvram_set <nombre> <valor>");
        return;
    }

    let name = args[0];
    let value = args[1..].join(" ");

    let name_utf16 = str_to_utf16(&name);
    let value_bytes = value.as_bytes();

    match set_var(&name_utf16, &MESA_KERNEL_GUID, value_bytes) {
        Ok(()) => {
            crate::print_success(&alloc::format!("Variable '{}' guardada en NVRAM", name));
        }
        Err(status) => {
            crate::print_error(&alloc::format!("Error guardando variable: {}", status));
        }
    }
}

/// Command: nvram_get <name> - Read a variable from NVRAM
pub fn cmd_nvram_get(args: &[&str]) {
    if args.is_empty() {
        crate::print_error("Uso: nvram_get <nombre>");
        return;
    }

    let name = args[0];
    let name_utf16 = str_to_utf16(&name);

    match get_var(&name_utf16, &MESA_KERNEL_GUID) {
        Ok(data) => {
            if let Ok(s) = core::str::from_utf8(&data) {
                crate::print_success(&alloc::format!("Variable '{}': \"{}\"", name, s));
            } else {
                crate::print_success(&alloc::format!(
                    "Variable '{}' ({} bytes)",
                    name,
                    data.len()
                ));
            }
        }
        Err(status) => {
            if status == EFI_NOT_FOUND {
                crate::print_error(&alloc::format!(
                    "Variable '{}' no encontrada (EFI_NOT_FOUND)",
                    name
                ));
            } else {
                crate::print_error(&alloc::format!("Error leyendo variable: {}", status));
            }
        }
    }
}

/// Command: nvram_del <name> - Delete a variable from NVRAM
pub fn cmd_nvram_del(args: &[&str]) {
    if args.is_empty() {
        crate::print_error("Uso: nvram_del <nombre>");
        return;
    }

    let name = args[0];
    let name_utf16 = str_to_utf16(&name);

    match delete_var(&name_utf16, &MESA_KERNEL_GUID) {
        Ok(()) => {
            crate::print_success(&alloc::format!("Variable '{}' eliminada de NVRAM", name));
        }
        Err(status) => {
            if status == EFI_NOT_FOUND {
                crate::print_error(&alloc::format!("Variable '{}' no encontrada", name));
            } else {
                crate::print_error(&alloc::format!("Error eliminando variable: {}", status));
            }
        }
    }
}

/// Command: nvram_list - List variables in kernel GUID namespace
pub fn cmd_nvram_list(_args: &[&str]) {
    crate::mesa_println!("=== Variables UEFI en Namespace MesaOS ===");
    crate::mesa_println!("  (Requiere inicialización UEFI Runtime Services)");
    crate::mesa_println!("  Usa 'nvram_test' para probar el subsistema");
    crate::mesa_println!();
}

/// Command: nvram_test - Run NVRAM test
pub fn cmd_nvram_test(_args: &[&str]) {
    crate::mesa_println!("=== Test de UEFI NVRAM ===");
    crate::mesa_println!();
    crate::print_info("Nota: UEFI Runtime Services requieren acceso");
    crate::print_info("al puntero EFI_SYSTEM_TABLE del bootloader.");
    crate::mesa_println!();
    crate::print_info("Para probar en hardware real o QEMU con UEFI:");
    crate::mesa_println!("  nvram_set MiVariable \"Hola NVRAM\"");
    crate::mesa_println!("  nvram_get MiVariable");
    crate::mesa_println!("  nvram_del MiVariable");
    crate::mesa_println!();
}
