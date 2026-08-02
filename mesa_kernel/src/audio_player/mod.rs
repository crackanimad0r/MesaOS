//! Audio Player para Mesa OS
//!
//! Módulo de reproducción de audio que se apoya en el driver HDA
//! (`crate::drivers::hda`) para enviar PCM al hardware.
//!
//! Formatos soportados:
//! - **WAV** (RIFF/WAVE, PCM sin comprimir) — reproducido directamente
//! - **MP3** (ADTS, MPEG-1 Layer III) — solo extracción de metadatos
//!   (duración, sample rate, canales) ya que la decodificación
//!   completa requiere un decoder completo (Huffman, MDCT, etc.)
//!   que añadiría demasiada complejidad al kernel.
//!
//! Para los MP3 se sugiere convertirlos a WAV 48 kHz / 16-bit / estéreo
//! desde el host y luego usar el comando `play`.
//!
//! # Estrategia de reproducción
//!
//! El buffer DMA del HDA es de 512 KB (128 entradas BDL × 4 KB). A
//! 48 kHz / 16-bit / estéreo, eso son ~2.7 segundos de audio. Para
//! poder reproducir archivos más largos, dividimos el audio en
//! "chunks" (trozos) que llenan todo el buffer DMA y los reproducimos
//! secuencialmente:
//!
//! 1. Cargamos y decodificamos el WAV completo a su frecuencia nativa.
//! 2. Por cada chunk, resampleamos sobre la marcha de la frecuencia
//!    original a 48 kHz para evitar alocar el audio expandido.
//! 3. Cada chunk (512 KB ≈ 2.73 s) se envía al HDA.
//! 4. Cuando se acaban los chunks, paramos.

pub mod mp3;
pub mod wav;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Frecuencia de muestreo del hardware HDA (la fija el driver)
pub const HDA_SAMPLE_RATE: u32 = 48_000;
/// Canales del hardware HDA (estéreo)
pub const HDA_CHANNELS: u16 = 2;
/// Bits por muestra del hardware HDA
pub const HDA_BITS_PER_SAMPLE: u16 = 16;

/// Tamaño máximo del buffer DMA de audio (debe coincidir con hda.rs)
pub const AUDIO_BUFFER_SIZE: usize = 128 * 4096; // 512 KB

/// Tamaño de cada chunk PCM que se envía al HDA.
/// Usamos el buffer DMA completo (512 KB ≈ 2.73 s a 48 kHz / 16-bit / estéreo)
/// para minimizar el número de transiciones entre chunks y reducir los
/// glitches auditivos por el SRST del stream.
const CHUNK_BYTES: usize = 128 * 4096;

/// Errores del reproductor de audio
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    /// El archivo no existe o no se pudo leer
    FileNotFound,
    /// Formato no soportado o archivo corrupto
    InvalidFormat,
    /// El archivo no contiene samples PCM reproducibles
    NotPcm,
    /// No hay suficiente memoria para cargar el audio
    OutOfMemory,
    /// Driver HDA no inicializado
    DriverNotReady,
    /// Error durante la decodificación
    DecodeError,
}

impl AudioError {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioError::FileNotFound => "Archivo no encontrado o no legible",
            AudioError::InvalidFormat => "Formato no soportado o archivo corrupto",
            AudioError::NotPcm => "El archivo no contiene audio PCM reproducible",
            AudioError::OutOfMemory => "Memoria insuficiente",
            AudioError::DriverNotReady => "Driver de audio (HDA) no inicializado",
            AudioError::DecodeError => "Error al decodificar el audio",
        }
    }
}

pub type AudioResult<T> = Result<T, AudioError>;

/// Estructura con los samples PCM en formato interno (s16, estéreo, 48 kHz)
#[derive(Clone)]
pub struct PcmBuffer {
    pub samples: Vec<i16>, // Interleaved L,R,L,R,...
    pub sample_rate: u32,
    pub channels: u16,
}

impl PcmBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            sample_rate: HDA_SAMPLE_RATE,
            channels: HDA_CHANNELS,
        }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Duración en milisegundos
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0;
        }
        let frames = (self.samples.len() as u64) / (self.channels as u64);
        (frames * 1000) / (self.sample_rate as u64)
    }
}

/// Detecta el tipo de archivo a partir de los primeros bytes
pub fn detect_format(data: &[u8]) -> AudioFormat {
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE" {
        return AudioFormat::Wav;
    }
    // MP3 con header ADTS: 0xFF 0xFx (sync), bit 1 siempre a 1
    if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xE0) == 0xE0 {
        return AudioFormat::Mp3;
    }
    AudioFormat::Unknown
}

/// Tipos de audio reconocibles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Mp3,
    Unknown,
}

impl AudioFormat {
    pub fn name(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "WAV (PCM)",
            AudioFormat::Mp3 => "MP3 (ADTS)",
            AudioFormat::Unknown => "Desconocido",
        }
    }
}

/// Convierte samples i16 a bytes little-endian (interleaved L,R,L,R,...)
fn samples_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        let v = *s as u16;
        bytes.push((v & 0xFF) as u8);
        bytes.push((v >> 8) as u8);
    }
    bytes
}

/// Carga un archivo PCM de un path del VFS y lo convierte
/// al formato nativo del HDA (48 kHz, estéreo, 16-bit).
pub fn load_file_as_pcm(path: &str) -> AudioResult<PcmBuffer> {
    let data = crate::fs::read(path).map_err(|_| AudioError::FileNotFound)?;
    load_bytes_as_pcm(&data)
}

/// Carga bytes arbitrarios y los convierte al formato del HDA.
pub fn load_bytes_as_pcm(data: &[u8]) -> AudioResult<PcmBuffer> {
    let format = detect_format(data);
    match format {
        AudioFormat::Wav => wav::decode_wav(data),
        AudioFormat::Mp3 => Err(AudioError::NotPcm), // No se decodifica MP3
        AudioFormat::Unknown => Err(AudioError::InvalidFormat),
    }
}

/// Re-muestrea un buffer PCM a la frecuencia objetivo.
pub fn resample_to(pcm: &mut PcmBuffer, target_rate: u32) {
    if pcm.sample_rate == target_rate {
        return;
    }

    let channels = pcm.channels as usize;
    let frames_in = pcm.samples.len() / channels;
    if frames_in == 0 {
        return;
    }

    // Cálculo con u64 para evitar overflow
    let frames_out =
        ((frames_in as u64) * (target_rate as u64) / (pcm.sample_rate as u64)) as usize;

    let mut out = Vec::with_capacity(frames_out * channels);
    for i in 0..frames_out {
        // Posición "ideal" en el input (en frames)
        let pos = (i as u64) * (pcm.sample_rate as u64) / (target_rate as u64);
        let pos = pos.min((frames_in - 1) as u64) as usize;

        // Interpolación lineal entre `pos` y `pos+1`
        let frac_num = (i as u64) * (pcm.sample_rate as u64) % (target_rate as u64);
        let frac = (frac_num as f32) / (target_rate as f32);

        for c in 0..channels {
            let s0 = pcm.samples[pos * channels + c] as f32;
            let s1_idx = (pos + 1).min(frames_in - 1) * channels + c;
            let s1 = pcm.samples[s1_idx] as f32;
            let s = s0 + (s1 - s0) * frac;
            let rounded = if s >= 0.0 { s + 0.5 } else { s - 0.5 };
            let clamped = if rounded > 32767.0 {
                32767.0
            } else if rounded < -32768.0 {
                -32768.0
            } else {
                rounded
            };
            out.push(clamped as i16);
        }
    }

    pcm.samples = out;
    pcm.sample_rate = target_rate;
}

/// Inspecciona un archivo de audio y devuelve un string formateado
/// con sus metadatos (duración, sample rate, canales, etc.).
pub fn inspect_file(path: &str) -> AudioResult<String> {
    use alloc::format;
    let data = crate::fs::read(path).map_err(|_| AudioError::FileNotFound)?;
    let format = detect_format(&data);
    match format {
        AudioFormat::Wav => {
            let meta = wav::parse_wav(&data)?;
            let codec = if meta.is_pcm { "PCM" } else { "Comprimido" };
            Ok(format!(
                "  Formato:   WAV ({codec})\n  Sample rate: {} Hz\n  Canales:    {}\n  Bits:       {}\n  Duración:   {}.{:03} s\n  Datos:      {} bytes\n",
                meta.sample_rate,
                meta.channels,
                meta.bits_per_sample,
                meta.duration_ms / 1000,
                meta.duration_ms % 1000,
                meta.data_size,
            ))
        }
        AudioFormat::Mp3 => {
            let meta = mp3::parse_mp3(&data)?;
            Ok(format!(
                "  Formato:   MP3 (ADTS, Layer III)\n  Sample rate: {} Hz\n  Canales:    {}\n  Bitrate:    {} kbps\n  Duración:   {}.{:03} s\n  Frames:     {}\n  Pista: usa ffmpeg para convertir a WAV 48kHz/16-bit/estéreo\n",
                meta.sample_rate,
                meta.channels,
                meta.bitrate_kbps,
                meta.duration_ms / 1000,
                meta.duration_ms % 1000,
                meta.num_frames_estimate,
            ))
        }
        AudioFormat::Unknown => Err(AudioError::InvalidFormat),
    }
}

/// Carga un archivo de audio, lo decodifica y lo reproduce.
/// Soporta WAV PCM. MP3 devuelve `NotPcm` (decoder no implementado).
pub fn play_file(path: &str) -> AudioResult<()> {
    let pcm = load_file_as_pcm(path)?;
    play_pcm_blocking(&pcm)
}

/// Convierte un chunk de samples i16 a bytes (little-endian, interleaved).
/// Reusa un buffer interno para evitar allocaciones por chunk.
fn chunk_to_bytes(samples: &[i16], out: &mut Vec<u8>) {
    let needed = samples.len() * 2;
    if out.capacity() < needed {
        out.reserve(needed - out.capacity());
    }
    out.clear();
    for s in samples {
        let v = *s as u16;
        out.push((v & 0xFF) as u8);
        out.push((v >> 8) as u8);
    }
}

/// Reproduce un buffer PCM completo a través del driver HDA en chunks.
/// Si el sample rate no es 48 kHz, hace resampling lineal por chunk
/// para evitar alocar el buffer expandido completo.
pub fn play_pcm_blocking(pcm: &PcmBuffer) -> AudioResult<()> {
    if pcm.samples.is_empty() {
        return Err(AudioError::DecodeError);
    }
    if pcm.channels != HDA_CHANNELS {
        return Err(AudioError::NotPcm);
    }

    let channels = HDA_CHANNELS as usize;
    let samples = &pcm.samples;
    let total_input_frames = samples.len() / channels;
    let source_rate = pcm.sample_rate;
    let target_rate = HDA_SAMPLE_RATE;

    if total_input_frames == 0 {
        return Err(AudioError::DecodeError);
    }

    // Output frames por chunk: 64KB de bytes PCM a 48kHz/16-bit/stéreo
    let output_frames_per_chunk = CHUNK_BYTES / (channels * 2);

    // Total de output frames (después de resamplear si aplica)
    let total_output_frames = if source_rate == target_rate {
        total_input_frames
    } else {
        ((total_input_frames as u64) * (target_rate as u64) / (source_rate as u64)) as usize
    };

    let total_chunks =
        (total_output_frames + output_frames_per_chunk - 1) / output_frames_per_chunk;

    crate::serial_println!(
        "[AUDIO] Streaming {} frames (src {} Hz) en {} chunks de {} frames out",
        total_input_frames,
        source_rate,
        total_chunks,
        output_frames_per_chunk,
    );

    // Buffer reutilizable para bytes del chunk (evita alloc/free por iteración)
    let mut chunk_bytes = Vec::with_capacity(CHUNK_BYTES);

    #[cfg(target_arch = "x86_64")]
    {
        let mut output_frame = 0usize;
        for chunk_idx in 0..total_chunks {
            let out_frames_this = output_frames_per_chunk.min(total_output_frames - output_frame);
            let mut chunk_samples = Vec::with_capacity(out_frames_this * channels);

            for _ in 0..out_frames_this {
                let pos = if source_rate == target_rate {
                    output_frame
                } else {
                    ((output_frame as u64) * (source_rate as u64) / (target_rate as u64)) as usize
                };
                let pos = pos.min(total_input_frames - 1);

                let frac = if source_rate == target_rate {
                    0.0
                } else {
                    let frac_num =
                        (output_frame as u64) * (source_rate as u64) % (target_rate as u64);
                    frac_num as f32 / target_rate as f32
                };

                for c in 0..channels {
                    let s0 = samples[pos * channels + c] as f32;
                    let s1_idx = (pos + 1).min(total_input_frames - 1) * channels + c;
                    let s1 = samples[s1_idx] as f32;
                    let s = s0 + (s1 - s0) * frac;
                    let rounded = if s >= 0.0 { s + 0.5 } else { s - 0.5 };
                    let clamped = if rounded > 32767.0 {
                        32767.0
                    } else if rounded < -32768.0 {
                        -32768.0
                    } else {
                        rounded
                    };
                    chunk_samples.push(clamped as i16);
                }
                output_frame += 1;
            }

            chunk_to_bytes(&chunk_samples, &mut chunk_bytes);

            crate::serial_println!(
                "[AUDIO] Chunk {}/{}: {} bytes ({} out frames)",
                chunk_idx + 1,
                total_chunks,
                chunk_bytes.len(),
                out_frames_this,
            );

            if let Err(_e) = crate::drivers::hda::play_chunk(&chunk_bytes) {
                return Err(AudioError::DriverNotReady);
            }
        }
    }

    Ok(())
}
