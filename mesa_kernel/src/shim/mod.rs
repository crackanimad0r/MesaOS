pub mod bridge;
pub mod loader;
pub mod manager;
pub mod scm;
pub mod symbols;

pub use bridge::*;
pub use loader::*;
pub use manager::*;
pub use scm::*;
pub use symbols::*;

pub fn init() {
    crate::printk!("[SHIM] Inicializando capa de compatibilidad de drivers Linux...");
    manager::init();
    crate::printk!(
        "[SHIM] {} simbolos de kernel Linux disponibles",
        symbols::KERNEL_SYMBOLS.len()
    );
    crate::printk!("[SHIM] Capa de drivers lista");
}
