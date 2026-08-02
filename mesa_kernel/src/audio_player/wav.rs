//! Decodificador de archivos WAV (RIFF/WAVE) en formato PCM.
//!
//! Soporta:
//! - PCM 8 / 16 / 24 / 32 bits por muestra
//! - Mono y estéreo
//! - Cualquier sample rate (se re-muestrea fuera, en `mod.rs`)
//!
//! No soporta formatos comprimidos (IMA ADPCM, μ-law, A-law, etc.):
//! en esos casos `parse_wav` devolverá `Ok(meta)` con `is_pcm = false`,
//! pero `decode_wav` devolverá `Err(AudioError::NotPcm)`.

use super::{AudioError, AudioResult, PcmBuffer, HDA_CHANNELS};
use alloc::string::String;
use alloc::vec::Vec;

/// Metadatos extraídos del header WAV.
#[derive(Debug, Clone, Copy)]
pub struct WavMeta {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub is_pcm: bool,
    pub data_offset: usize,
    pub data_size: usize,
    pub duration_ms: u64,
}

impl WavMeta {
    pub fn is_stereo(&self) -> bool {
        self.channels == 2
    }
}

fn read_u16_le(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn read_u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn read_u32_be(data: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

/// Parsea el header WAV y devuelve los metadatos. No copia los samples.
pub fn parse_wav(data: &[u8]) -> AudioResult<WavMeta> {
    if data.len() < 44 {
        return Err(AudioError::InvalidFormat);
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err(AudioError::InvalidFormat);
    }

    // Recorremos los chunks para encontrar "fmt " y "data".
    let mut off: usize = 12;
    let mut sample_rate: u32 = 0;
    let mut channels: u16 = 0;
    let mut bits_per_sample: u16 = 0;
    let mut is_pcm: bool = false;
    let mut data_offset: usize = 0;
    let mut data_size: usize = 0;

    while off + 8 <= data.len() {
        let id = &data[off..off + 4];
        let size = read_u32_le(data, off + 4) as usize;
        let body_off = off + 8;
        if body_off + size > data.len() {
            break;
        }
        if id == b"fmt " {
            // El chunk "fmt " tiene al menos 16 bytes
            if size < 16 {
                return Err(AudioError::InvalidFormat);
            }
            // Formato: 0 = PCM, 1 = PCM (alias), otros = comprimido
            let fmt_code = read_u16_le(data, body_off);
            is_pcm = fmt_code == 0 || fmt_code == 1;
            channels = read_u16_le(data, body_off + 2);
            sample_rate = read_u32_le(data, body_off + 4);
            // byte_rate (4) y block_align (2) los ignoramos
            bits_per_sample = read_u16_le(data, body_off + 14);
        } else if id == b"data" {
            data_offset = body_off;
            data_size = size;
            break; // ya tenemos lo que necesitábamos
        }
        // Saltamos al siguiente chunk, alineado a 2 bytes
        let aligned_size = (size + 1) & !1usize;
        off = body_off + aligned_size;
    }

    if sample_rate == 0 || channels == 0 || bits_per_sample == 0 || data_offset == 0 {
        return Err(AudioError::InvalidFormat);
    }
    if data_size == 0 {
        // data_size puede ser 0 si el archivo termina prematuramente
        data_size = data.len().saturating_sub(data_offset);
    }

    let bytes_per_frame = (channels as usize) * (bits_per_sample as usize / 8);
    let frames = if bytes_per_frame > 0 {
        data_size / bytes_per_frame
    } else {
        0
    };
    let duration_ms = if sample_rate > 0 {
        (frames as u64 * 1000) / (sample_rate as u64)
    } else {
        0
    };

    Ok(WavMeta {
        sample_rate,
        channels,
        bits_per_sample,
        is_pcm,
        data_offset,
        data_size,
        duration_ms,
    })
}

/// Convierte samples 8/16/24/32 bits a `i16` saturando.
fn sample_to_i16(s: i32, bits: u16) -> i16 {
    match bits {
        8 => {
            // WAV 8-bit es unsigned, centrado en 128, escalar a rango 16-bit
            ((s - 128) * 256).max(-32768).min(32767) as i16
        }
        16 => s.max(-32768).min(32767) as i16,
        24 => (s >> 8).max(-32768).min(32767) as i16,
        32 => (s >> 16).max(-32768).min(32767) as i16,
        _ => 0,
    }
}

fn read_sample_le(data: &[u8], off: usize, bits: u16) -> i32 {
    match bits {
        8 => data[off] as i32,
        16 => i16::from_le_bytes([data[off], data[off + 1]]) as i32,
        24 => {
            let b0 = data[off] as i32;
            let b1 = data[off + 1] as i32;
            let b2 = data[off + 2] as i8 as i32; // sign-extend
            (b2 << 16) | (b1 << 8) | b0
        }
        32 => i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]),
        _ => 0,
    }
}

/// Decodifica un archivo WAV completo a un `PcmBuffer`.
/// Si el archivo es mono, lo duplica a estéreo.
/// Si no es PCM, devuelve `Err(AudioError::NotPcm)`.
pub fn decode_wav(data: &[u8]) -> AudioResult<PcmBuffer> {
    let meta = parse_wav(data)?;
    if !meta.is_pcm {
        return Err(AudioError::NotPcm);
    }
    // Sólo soportamos 8/16/24/32 bits
    if !matches!(meta.bits_per_sample, 8 | 16 | 24 | 32) {
        return Err(AudioError::NotPcm);
    }

    let bytes_per_sample = (meta.bits_per_sample / 8) as usize;
    let channels_in = meta.channels as usize;
    let bytes_per_frame = channels_in * bytes_per_sample;

    let data_end = (meta.data_offset + meta.data_size).min(data.len());
    let available = data_end.saturating_sub(meta.data_offset);
    let frames = available / bytes_per_frame;
    if frames == 0 {
        return Err(AudioError::DecodeError);
    }

    let channels_out = HDA_CHANNELS as usize;
    let mut samples: Vec<i16> = Vec::with_capacity(frames * channels_out);

    let mut off = meta.data_offset;
    for _ in 0..frames {
        // Leemos todos los canales del frame
        let mut frame_samples = [0i16; 8];
        for c in 0..channels_in {
            let s = read_sample_le(data, off + c * bytes_per_sample, meta.bits_per_sample);
            frame_samples[c] = sample_to_i16(s, meta.bits_per_sample);
        }
        // A estéreo
        match channels_in {
            1 => {
                samples.push(frame_samples[0]);
                samples.push(frame_samples[0]);
            }
            2 => {
                samples.push(frame_samples[0]);
                samples.push(frame_samples[1]);
            }
            n if n <= 8 => {
                // Promedio de canales extras al canal L, mantenemos R como promedio
                let mut l = 0i32;
                for c in 0..n {
                    l += frame_samples[c] as i32;
                }
                samples.push((l / n as i32) as i16);
                // Para R, mezclamos al revés (R=ultimo canal)
                samples.push(frame_samples[n - 1]);
            }
            _ => {
                // Demasiados canales: downmix a estéreo con promedios
                let mut l = 0i32;
                let mut r = 0i32;
                let half = channels_in / 2;
                for c in 0..half {
                    l += frame_samples[c] as i32;
                }
                for c in half..channels_in {
                    r += frame_samples[c] as i32;
                }
                samples.push((l / half as i32) as i16);
                samples.push((r / (channels_in - half) as i32) as i16);
            }
        }
        off += bytes_per_frame;
    }

    Ok(PcmBuffer {
        samples,
        sample_rate: meta.sample_rate,
        channels: HDA_CHANNELS,
    })
}

/// Información rápida del archivo en una sola string
pub fn describe(path: &str) -> Option<String> {
    let data = crate::fs::read(path).ok()?;
    let meta = parse_wav(&data).ok()?;
    Some(String::from(""))
}

#[allow(dead_code)]
pub fn read_u32_be_safe(data: &[u8], off: usize) -> u32 {
    read_u32_be(data, off)
}
