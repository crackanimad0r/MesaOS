//! initrd_data.rs - Punto de entrada para los datos del initrd de MesaOS
//!
//! Los datos binarios se generan con tools/inject_to_iso.sh y se compilan
//! directamente desde output/initrd.bin mediante include_bytes! para evitar
//! el consumo masivo de RAM al compilar arrays literales enormes.

/// Datos del initrd empaquetado (formato: entries_count + [name_len|name|path_len|path|data_len|data]*)
pub static INITRD_DATA: &[u8] = include_bytes!("../../../output/initrd.bin");
