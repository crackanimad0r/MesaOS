//! Soporte para ejecutar código en Ring 3 (User Mode)

pub mod exec;

// Re-exportar para compatibilidad con main.rs
pub use exec::programs;