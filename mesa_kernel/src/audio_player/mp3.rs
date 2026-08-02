//! Parser básico de MP3 (ADTS, MPEG-1/2 Layer III).
//!
//! Este módulo **NO decodifica** MP3. Solo extrae metadatos del header
//! ADTS (sample rate, canales, bitrate) y estima la duración.
//!
//! La decodificación completa requeriría un decoder Layer III (Huffman,
//! MDCT, banco de filtros, etc.) lo cual sería enormemente complejo para
//! un kernel. Se sugiere a los usuarios convertir los MP3 a WAV 48 kHz
//! 16-bit estéreo desde el host.

use super::AudioResult;

/// Tabla de sample rates MPEG-1 Layer III (en Hz), indexada por el campo
/// `sampling_frequency_index` del header.
const MPEG1_SAMPLE_RATES: [u32; 4] = [44100, 48000, 32000, 0];

/// Tabla de bitrates MPEG-1 Layer III en kbps, indexada por
/// `bitrate_index`. `0` significa inválido.
const MPEG1_L3_BITRATES: [u32; 16] = [
    0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
];

/// Metadatos extraídos del header ADTS.
#[derive(Debug, Clone, Copy)]
pub struct Mp3Meta {
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate_kbps: u32,
    pub frame_size: u16,
    pub duration_ms: u64,
    pub num_frames_estimate: u64,
}

fn read_u32_be(data: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

/// Parsea el primer header ADTS del archivo y devuelve los metadatos
/// básicos (sample rate, canales, bitrate, frame size, duración estimada).
pub fn parse_mp3(data: &[u8]) -> AudioResult<Mp3Meta> {
    if data.len() < 7 {
        return Err(super::AudioError::InvalidFormat);
    }

    // Buscar sync word 0xFF 0xFx
    let mut start = 0usize;
    while start + 1 < data.len() {
        if data[start] == 0xFF && (data[start + 1] & 0xE0) == 0xE0 {
            break;
        }
        start += 1;
    }
    if start + 7 > data.len() {
        return Err(super::AudioError::InvalidFormat);
    }

    // Header ADTS: 7 bytes
    //   byte 1: ID(1) | layer(2) | protection(1)
    //   byte 2: profile(2) | bitrate_index(4) | private(1) | padding(1)  (en realidad sf_idx es 2bits en ADTS)
    //   byte 3: channel_config(2) | original(1) | home(1) | copyright_id(1) | copyright_start(1) | frame_length_high(3)
    //   byte 4: frame_length_low(8)
    //   byte 5: frame_length_top(3) | buffer_fullness(5)
    //   ...
    //
    // Para MPEG-1 Layer III, el sf_idx ocupa los 2 bits más bajos del
    // header (los 2 últimos bits del byte 2) y la primera mitad de los
    // bits de channel. Para simplificar, usamos el enfoque de MPEG-1
    // tradicional que es el más común.

    let b0 = data[start + 1];
    let _mpeg_id = (b0 >> 3) & 1;
    let layer = (b0 >> 1) & 3;
    if layer != 1 {
        // Layer III == 01
        return Err(super::AudioError::InvalidFormat);
    }
    let _protection_absent = b0 & 1;

    let b1 = data[start + 2];
    let _profile = (b1 >> 6) & 3;
    // En MPEG-1 Layer III, bitrate_index son los 4 bits tras profile:
    let bitrate_index = ((b1 >> 2) & 0x0F) as usize;
    // sample_rate_index son los 2 bits más bajos:
    let sf_idx_low = (b1 & 3) as usize;
    let _private_pad = 0;

    // El sample_rate_index "real" para MPEG-1 es de 2 bits, pero en ADTS
    // header está partido. Usamos los 2 bits del byte 2 + los 2 bits más
    // altos del byte 3 (channel_config) como índice.
    let b2 = data[start + 3];
    let ch_config_top = (b2 >> 6) & 3; // 2 bits
    let sf_idx_combined = (sf_idx_low << 2) | ch_config_top as usize;
    let sample_rate_idx = sf_idx_combined & 0x03; // 0..=3

    let sample_rate = if sample_rate_idx < MPEG1_SAMPLE_RATES.len() {
        MPEG1_SAMPLE_RATES[sample_rate_idx]
    } else {
        return Err(super::AudioError::InvalidFormat);
    };
    if sample_rate == 0 {
        return Err(super::AudioError::InvalidFormat);
    }

    let bitrate_kbps = if bitrate_index < MPEG1_L3_BITRATES.len() {
        MPEG1_L3_BITRATES[bitrate_index]
    } else {
        0
    };

    // Channel config (3 bits en realidad) en byte 3 + byte 2
    let ch_low = ((b2 >> 6) & 3) as u16;
    let ch_high = ((data[start + 2]) & 1) as u16;
    let channel_config = (ch_high << 2) | ch_low;
    let channels: u16 = match channel_config {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 2,
    };

    // Frame length: 11 bits a partir de los bytes 3-4
    // bits en byte 3: 0-2 son los 3 bits altos de frame_length
    // bits en byte 4: 0-7 son los 8 bits bajos
    let frame_length: u16 = (((data[start + 3] as u16) & 0x03) << 8) | (data[start + 4] as u16);

    // Estimación de duración: contar frames válidos en el archivo
    let mut pos = start;
    let mut frame_count: u64 = 0;
    let mut last_frame_size: u16 = frame_length;
    while pos + 7 < data.len() {
        if data[pos] == 0xFF && (data[pos + 1] & 0xE0) == 0xE0 {
            // Re-leer el frame length en esta posición
            let fl = (((data[pos + 3] as u16) & 0x03) << 8) | (data[pos + 4] as u16);
            if fl < 7 || pos + fl as usize > data.len() {
                break;
            }
            last_frame_size = fl;
            pos += fl as usize;
            frame_count += 1;
        } else {
            pos += 1;
        }
        if frame_count > 100_000 {
            break; // seguridad
        }
    }

    let duration_ms = if sample_rate > 0 {
        // 1152 samples por frame en Layer III MPEG-1
        let samples_per_frame: u64 = 1152;
        let total_samples = frame_count.saturating_mul(samples_per_frame);
        (total_samples * 1000) / (sample_rate as u64)
    } else {
        0
    };

    let _ = last_frame_size;

    Ok(Mp3Meta {
        sample_rate,
        channels,
        bitrate_kbps,
        frame_size: frame_length,
        duration_ms,
        num_frames_estimate: frame_count,
    })
}

#[allow(dead_code)]
pub fn read_u32_be_safe(data: &[u8], off: usize) -> u32 {
    read_u32_be(data, off)
}
