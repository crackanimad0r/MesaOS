// mesa_kernel/src/drivers/hda.rs

use crate::memory::{pmm, vmm};
use crate::pci;

const HDA_GCAP: usize = 0x00;
const HDA_GCTL: usize = 0x08;
const HDA_STATESTS: usize = 0x0E;

const HDA_CORBLBASE: usize = 0x40;
const HDA_CORBUBASE: usize = 0x44;
const HDA_CORBWP: usize = 0x48;
const HDA_CORBRP: usize = 0x4A;
const HDA_CORBCTL: usize = 0x4C;
const HDA_CORBSIZE: usize = 0x4E;

const HDA_RIRBLBASE: usize = 0x50;
const HDA_RIRBUBASE: usize = 0x54;
const HDA_RIRBWP: usize = 0x58;
const HDA_RINTCNT: usize = 0x5A;
const HDA_RIRBCTL: usize = 0x5C;
const HDA_RIRBSTS: usize = 0x5D;
const HDA_RIRBSIZE: usize = 0x5E;

pub struct HdaDriver {
    bar_virt: usize,
    corb_virt: *mut u32,
    rirb_virt: *mut u64,
    corb_entries: u16,
    rirb_entries: u16,
    codec_mask: u16, // Bitmask de codecs detectados (guardada durante init)

    stream_base: usize,
    bdl_virt: *mut u32,
    audio_virt: *mut u8,
    stream_running: bool,
    pub volume: u8,
}

static MP3_PCM: &[u8] = include_bytes!("audio_test.raw");

unsafe impl Send for HdaDriver {}
unsafe impl Sync for HdaDriver {}

use spin::Mutex;
pub static HDA: Mutex<Option<HdaDriver>> = Mutex::new(None);

impl HdaDriver {
    fn read_reg8(&self, offset: usize) -> u8 {
        unsafe { core::ptr::read_volatile((self.bar_virt + offset) as *const u8) }
    }

    fn write_reg8(&mut self, offset: usize, value: u8) {
        unsafe { core::ptr::write_volatile((self.bar_virt + offset) as *mut u8, value) }
    }

    fn read_reg16(&self, offset: usize) -> u16 {
        unsafe { core::ptr::read_volatile((self.bar_virt + offset) as *const u16) }
    }

    fn write_reg16(&mut self, offset: usize, value: u16) {
        unsafe { core::ptr::write_volatile((self.bar_virt + offset) as *mut u16, value) }
    }

    fn read_reg32(&self, offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((self.bar_virt + offset) as *const u32) }
    }

    fn write_reg32(&mut self, offset: usize, value: u32) {
        unsafe { core::ptr::write_volatile((self.bar_virt + offset) as *mut u32, value) }
    }

    pub fn send_verb(&mut self, codec: u8, node: u8, payload: u32) -> u32 {
        let verb =
            ((codec as u32 & 0xF) << 28) | ((node as u32 & 0xFF) << 20) | (payload & 0xFFFFF);

        let mut corb_wp = self.read_reg16(HDA_CORBWP);
        corb_wp = (corb_wp + 1) % self.corb_entries;

        unsafe {
            core::ptr::write_volatile(self.corb_virt.add(corb_wp as usize), verb);
        }

        let initial_rirb_wp = self.read_reg16(HDA_RIRBWP);

        self.write_reg16(HDA_CORBWP, corb_wp);

        let mut timeout = 10_000_000;
        let mut current_rirb_wp = initial_rirb_wp;
        while current_rirb_wp == initial_rirb_wp && timeout > 0 {
            current_rirb_wp = self.read_reg16(HDA_RIRBWP);
            core::hint::spin_loop();
            timeout -= 1;
        }

        if timeout == 0 {
            crate::serial_println!("[HDA] Timeout esperando respuesta del verbo!");
            return 0;
        }

        self.write_reg8(HDA_RIRBSTS, 0x05); // Clear RINTFL and RIRBOI

        unsafe {
            let response_64 =
                core::ptr::read_volatile(self.rirb_virt.add(current_rirb_wp as usize));
            (response_64 & 0xFFFFFFFF) as u32
        }
    }
}

pub fn init() -> Result<(), &'static str> {
    crate::serial_println!("[HDA] Buscando controlador HD Audio...");

    let hda_dev = match pci::find_hda_controller() {
        Some(dev) => dev,
        None => {
            crate::serial_println!("[HDA] No se encontro controlador HD Audio.");
            return Err("HDA Controller not found");
        }
    };

    crate::serial_println!(
        "[HDA] Encontrado controlador en {:02x}:{:02x}.{}",
        hda_dev.bus,
        hda_dev.device,
        hda_dev.function
    );

    // Habilitar Bus Mastering y Memory Space
    pci::pci_enable_bus_mastering(hda_dev.bus, hda_dev.device, hda_dev.function);
    pci::pci_enable_memory_space(hda_dev.bus, hda_dev.device, hda_dev.function);

    let (bar0_phys, _bar0_size) =
        match pci::pci_read_bar(hda_dev.bus, hda_dev.device, hda_dev.function, 0) {
            Some(bar) => bar,
            None => return Err("Failed to read HDA BAR0"),
        };

    let bar_virt = vmm::phys_to_virt(bar0_phys) as usize;
    crate::serial_println!(
        "[HDA] BAR0 Fisico: {:#x}, Virtual: {:#x}",
        bar0_phys,
        bar_virt
    );

    let mut driver = HdaDriver {
        bar_virt,
        corb_virt: core::ptr::null_mut(),
        rirb_virt: core::ptr::null_mut(),
        corb_entries: 0,
        rirb_entries: 0,
        codec_mask: 0,
        stream_base: 0,
        bdl_virt: core::ptr::null_mut(),
        audio_virt: core::ptr::null_mut(),
        stream_running: false,
        volume: 25,
    };

    // Leer GCAP *despues* del reset, por ahora calculamos stream_base con fallback
    // (se actualizará tras el reset)
    driver.stream_base = 0x80; // Fallback: asumimos 0 input streams

    // 1. Controller Reset
    crate::serial_println!("[HDA] Iniciando Controller Reset...");

    // Leer GCTL y poner CRST (bit 0) a 0
    let mut gctl = driver.read_reg32(HDA_GCTL);
    driver.write_reg32(HDA_GCTL, gctl & !1);

    // Esperar a que el bit baje
    for _ in 0..100_000 {
        if (driver.read_reg32(HDA_GCTL) & 1) == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    // Poner CRST a 1
    gctl = driver.read_reg32(HDA_GCTL);
    driver.write_reg32(HDA_GCTL, gctl | 1);

    // Esperar a que el bit suba
    let mut reset_ok = false;
    for _ in 0..1_000_000 {
        if (driver.read_reg32(HDA_GCTL) & 1) == 1 {
            reset_ok = true;
            break;
        }
        core::hint::spin_loop();
    }

    if !reset_ok {
        crate::serial_println!("[HDA] ERROR: Timeout esperando reset del controlador");
        return Err("HDA Controller Reset Timeout");
    }

    // Esperar un poco a que los Codecs se despierten
    for _ in 0..1_000_000 {
        core::hint::spin_loop();
    }

    crate::serial_println!("[HDA] Controller Reset Exitoso.");

    // Ahora que el controlador está en marcha, leer GCAP real
    let gcap = driver.read_reg16(HDA_GCAP);
    let iss = (gcap >> 8) & 0x0F;
    let oss = (gcap >> 12) & 0x0F;
    crate::serial_println!("[HDA] GCAP: ISS={} OSS={}", iss, oss);
    driver.stream_base = 0x80 + (iss as usize * 0x20);
    crate::serial_println!("[HDA] Output Stream 0 offset: {:#x}", driver.stream_base);

    // Leer STATESTS y guardarlo en el struct ANTES de limpiarlo
    let statests = driver.read_reg16(HDA_STATESTS);
    driver.codec_mask = statests; // Guardar para futuros usos
    crate::serial_println!(
        "[HDA] STATESTS: {:#06x} (Codecs detectados: {})",
        statests,
        statests.count_ones()
    );

    // 2. Configurar CORB y RIRB
    // Asignar 1 frame (4KB) fisico continuo para ambos (CORB usa 1KB, RIRB usa 2KB tipicamente)
    let dma_phys = match pmm::alloc_frames(1) {
        Some(p) => p,
        None => return Err("Failed to alloc DMA for CORB/RIRB"),
    };
    let dma_virt = vmm::phys_to_virt(dma_phys);

    unsafe {
        core::ptr::write_bytes(dma_virt as *mut u8, 0, 4096);
    }

    let corb_phys = dma_phys;
    let rirb_phys = dma_phys + 2048; // Mitad del frame

    driver.corb_virt = dma_virt as *mut u32;
    driver.rirb_virt = (dma_virt + 2048) as *mut u64;

    // Detener CORB y RIRB
    driver.write_reg8(HDA_CORBCTL, 0);
    driver.write_reg8(HDA_RIRBCTL, 0);

    // Configurar CORB
    driver.write_reg32(HDA_CORBLBASE, (corb_phys & 0xFFFFFFFF) as u32);
    driver.write_reg32(HDA_CORBUBASE, (corb_phys >> 32) as u32);

    // Tamano maximo (usualmente 256 entradas = bit 1 en CORBSIZE)
    driver.write_reg8(HDA_CORBSIZE, 0x02);
    driver.corb_entries = 256;

    // Resetear punteros CORB
    driver.write_reg16(HDA_CORBRP, 0x8000); // Bit 15: Reset
    for _ in 0..10_000 {
        core::hint::spin_loop();
    }
    driver.write_reg16(HDA_CORBRP, 0);
    driver.write_reg16(HDA_CORBWP, 0);

    // Configurar RIRB
    driver.write_reg32(HDA_RIRBLBASE, (rirb_phys & 0xFFFFFFFF) as u32);
    driver.write_reg32(HDA_RIRBUBASE, (rirb_phys >> 32) as u32);

    // Tamano maximo (usualmente 256 entradas = bit 1 en RIRBSIZE)
    driver.write_reg8(HDA_RIRBSIZE, 0x02);
    driver.rirb_entries = 256;

    // Resetear punteros RIRB
    driver.write_reg16(HDA_RIRBWP, 0x8000);
    driver.write_reg16(HDA_RINTCNT, 255); // Interrumpir cada 255 respuestas

    // Arrancar CORB y RIRB (Bit 1 = Enable)
    driver.write_reg8(HDA_CORBCTL, 0x02);
    driver.write_reg8(HDA_RIRBCTL, 0x02);

    crate::serial_println!("[HDA] CORB y RIRB configurados e iniciados.");

    // 3. Configurar Stream DMA
    let stream_dma_phys = match pmm::alloc_frames(129) {
        // 1 frame for BDL + 128 frames for audio (512KB)
        Some(p) => p,
        None => return Err("Failed to alloc DMA for Stream"),
    };

    let stream_bdl_virt = vmm::phys_to_virt(stream_dma_phys) as *mut u32;
    let stream_audio_phys = stream_dma_phys + 4096;
    let stream_audio_virt = vmm::phys_to_virt(stream_audio_phys) as *mut u8;

    unsafe {
        core::ptr::write_bytes(stream_bdl_virt as *mut u8, 0, 4096);
        core::ptr::write_bytes(stream_audio_virt, 0, 128 * 4096);
    }

    driver.bdl_virt = stream_bdl_virt;
    driver.audio_virt = stream_audio_virt;

    // No sine wave generation, buffer is clear

    // Configurar el BDL
    // 128 entradas de 4KB cada una
    unsafe {
        for i in 0..128 {
            let offset = i * 4096;
            let phys = stream_audio_phys + offset as u64;
            core::ptr::write_volatile(stream_bdl_virt.add(i * 4 + 0), (phys & 0xFFFFFFFF) as u32);
            core::ptr::write_volatile(stream_bdl_virt.add(i * 4 + 1), (phys >> 32) as u32);
            core::ptr::write_volatile(stream_bdl_virt.add(i * 4 + 2), 4096);
            // Solo activar IOC en la ultima entrada
            core::ptr::write_volatile(
                stream_bdl_virt.add(i * 4 + 3),
                if i == 127 { 0x01 } else { 0x00 },
            );
        }
    }

    // Configurar registros del Output Stream 0
    let stream_base = driver.stream_base;

    // Reset Stream
    driver.write_reg8(stream_base + 0x00, 0x01); // SD0_CTL reset
    for _ in 0..10_000 {
        core::hint::spin_loop();
    }
    driver.write_reg8(stream_base + 0x00, 0x00); // Clear reset
    for _ in 0..10_000 {
        core::hint::spin_loop();
    }

    // Configurar Stream
    let stream_tag = 1;
    // SDnCTL: Stream Tag está en los bits 23:20
    let sd_ctl = (stream_tag << 20) as u32;
    // Escribimos 24 bits de CTL y 8 bits de STS (cero)
    driver.write_reg32(stream_base + 0x00, sd_ctl);

    driver.write_reg32(stream_base + 0x08, (128 * 4096) as u32); // SD0_CBL (Cyclic Buffer Length)
                                                                 // LVI = 127: el índice del último BDL válido
    driver.write_reg16(stream_base + 0x0C, 127); // SD0_LVI

    driver.write_reg32(stream_base + 0x18, (stream_dma_phys & 0xFFFFFFFF) as u32); // BDLPL
    driver.write_reg32(stream_base + 0x1C, (stream_dma_phys >> 32) as u32); // BDLPU

    // SD0_FMT: 48kHz base (bits 14:11 = 0000), MULT x1 (bits 13:11=000),
    //          DIV /1 (bits 10:8=000), 16-bit (bits 6:4=001), 2 canales stereo (bits 3:0=0001)
    // = 0x0011 (48kHz, 16-bit, stereo)
    driver.write_reg16(stream_base + 0x12, 0x0011); // SD0_FMT: 48kHz, 16-bit, 2ch

    crate::serial_println!("[HDA] Stream DMA 0 Configurado (Tag {}).", stream_tag);

    *HDA.lock() = Some(driver);

    // Enumerar codecs para mostrar que funciona
    enumerate_codecs();

    // El stream no se inicia automaticamente por defecto
    // Usar el comando hda test para reproducir el MP3

    Ok(())
}

pub fn print_status() {
    let mut lock = HDA.lock();
    if let Some(ref mut driver) = *lock {
        crate::mesa_println!("[HDA] Controlador detectado y en linea.");
        crate::mesa_println!("  BAR0 Virtual : {:#x}", driver.bar_virt);
        crate::mesa_println!(
            "  Codecs       : {} (mask: {:#06x})",
            driver.codec_mask.count_ones(),
            driver.codec_mask
        );
        crate::mesa_println!("  Stream Base  : {:#x}", driver.stream_base);
        crate::mesa_println!(
            "  Stream DMA   : {}",
            if driver.stream_running {
                "ACTIVO"
            } else {
                "PARADO"
            }
        );
        crate::mesa_println!("  CORB Entradas: {}", driver.corb_entries);
        crate::mesa_println!("  RIRB Entradas: {}", driver.rirb_entries);
    } else {
        crate::mesa_println!(
            "[HDA] El controlador de audio no esta inicializado o no fue encontrado."
        );
    }
}

pub fn enumerate_codecs() {
    let mut lock = HDA.lock();
    if let Some(ref mut driver) = *lock {
        // Usar el codec_mask guardado durante el init (STATESTS ya fue limpiado)
        let codec_mask = driver.codec_mask;

        if codec_mask == 0 {
            crate::mesa_println!("[HDA] No se detectaron codecs durante el arranque.");
            return;
        }

        for codec in 0u8..15u8 {
            if (codec_mask & (1 << codec)) != 0 {
                crate::mesa_println!("[HDA] Codec {}:", codec);

                let vendor_id = driver.send_verb(codec, 0, 0xF0000);
                crate::mesa_println!("  Vendor ID: {:#010x}", vendor_id);

                let nodes_info = driver.send_verb(codec, 0, 0xF0004);
                let starting_node = (nodes_info >> 16) & 0xFF;
                let total_nodes = nodes_info & 0xFF;

                crate::mesa_println!("  Nodos base: {} (Total: {})", starting_node, total_nodes);

                for node in starting_node..(starting_node + total_nodes) {
                    let widget_type = driver.send_verb(codec, node as u8, 0xF0005);
                    let fg_type = widget_type & 0xFF;
                    crate::mesa_println!("  Nodo {:#04x}: Grupo Funcional {:#04x}", node, fg_type);

                    if fg_type == 0x01 {
                        let afg_nodes_info = driver.send_verb(codec, node as u8, 0xF0004);
                        let afg_start = (afg_nodes_info >> 16) & 0xFF;
                        let afg_total = afg_nodes_info & 0xFF;
                        crate::mesa_println!(
                            "    AFG Nodos: Start {} Total {}",
                            afg_start,
                            afg_total
                        );

                        for w_node in afg_start..(afg_start + afg_total) {
                            let w_caps = driver.send_verb(codec, w_node as u8, 0xF0009);
                            let w_type = (w_caps >> 20) & 0xF;

                            let w_type_str = match w_type {
                                0 => "Audio Output (DAC)",
                                1 => "Audio Input (ADC)",
                                2 => "Audio Mixer",
                                3 => "Audio Selector",
                                4 => "Pin Complex",
                                5 => "Power Widget",
                                6 => "Volume Widget",
                                7 => "Beep Generator",
                                _ => "Unknown",
                            };

                            crate::mesa_println!(
                                "      Widget {:#04x}: {} (Caps: {:#010x})",
                                w_node,
                                w_type_str,
                                w_caps
                            );
                        }
                    }
                }
            }
        }
    } else {
        crate::mesa_println!("[HDA] Driver no inicializado.");
    }
}

pub fn play_test() {
    let mut lock = HDA.lock();
    if let Some(ref mut driver) = *lock {
        let stream_base = driver.stream_base;

        // --- Reset COMPLETO del stream para reiniciar LPIB a 0 ---
        // 1. Detener (clear RUN bit)
        driver.write_reg8(stream_base + 0x00, 0x00);
        driver.stream_running = false;
        for _ in 0..10_000 {
            core::hint::spin_loop();
        }

        // 2. Activar SRST (bit 0 del byte bajo de CTL) para reset del stream
        driver.write_reg8(stream_base + 0x00, 0x01); // SRST = 1
        let mut srst_ok = false;
        for _ in 0..100_000 {
            if (driver.read_reg8(stream_base + 0x00) & 0x01) != 0 {
                srst_ok = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !srst_ok {
            crate::serial_println!("[HDA] WARN: Timeout esperando SRST set");
        }

        // 3. Limpiar SRST (poner a 0) para salir del reset
        driver.write_reg8(stream_base + 0x00, 0x00); // SRST = 0
        for _ in 0..100_000 {
            if (driver.read_reg8(stream_base + 0x00) & 0x01) == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // 4. Reconfigurar todos los registros del stream (LPIB ya es 0 tras el reset)
        let stream_tag: u32 = 1;
        driver.write_reg32(stream_base + 0x00, (stream_tag << 20) as u32); // Tag, sin RUN
        driver.write_reg32(stream_base + 0x08, (128 * 4096) as u32); // CBL
        driver.write_reg16(stream_base + 0x0C, 127); // LVI
        driver.write_reg16(stream_base + 0x12, 0x0011); // FMT: 48kHz, 16-bit, 2ch

        // Reescribir dirección física del BDL (puede que el reset la haya borrado)
        let bdl_phys = vmm::virt_to_phys(driver.bdl_virt as u64);
        driver.write_reg32(stream_base + 0x18, (bdl_phys & 0xFFFFFFFF) as u32);
        driver.write_reg32(stream_base + 0x1C, (bdl_phys >> 32) as u32);

        for _ in 0..10_000 {
            core::hint::spin_loop();
        }
        // --- Fin reset stream ---

        // Copiar el audio
        unsafe {
            core::ptr::write_bytes(driver.audio_virt, 0, 128 * 4096);
            let copy_len = core::cmp::min(MP3_PCM.len(), 128 * 4096);
            core::ptr::copy_nonoverlapping(MP3_PCM.as_ptr(), driver.audio_virt, copy_len);
        }

        let codec = 0; // Asumimos Codec 0 por defecto

        let nodes_info = driver.send_verb(codec, 0, 0xF0004);
        let starting_node = (nodes_info >> 16) & 0xFF;
        let total_nodes = nodes_info & 0xFF;

        let gain = ((0x7F * driver.volume as u32) / 100) & 0x7F;

        for node in starting_node..(starting_node + total_nodes) {
            let fg_type = driver.send_verb(codec, node as u8, 0xF0005) & 0xFF;
            if fg_type == 0x01 {
                let afg_info = driver.send_verb(codec, node as u8, 0xF0004);
                let afg_start = (afg_info >> 16) & 0xFF;
                let afg_total = afg_info & 0xFF;

                for w_node in afg_start..(afg_start + afg_total) {
                    let w_node = w_node as u8;
                    let w_caps = driver.send_verb(codec, w_node, 0xF0009);
                    let w_type = (w_caps >> 20) & 0xF;

                    let has_in_amp = (w_caps & (1 << 1)) != 0;
                    let has_out_amp = (w_caps & (1 << 2)) != 0;

                    if has_out_amp {
                        driver.send_verb(codec, w_node, 0x3B000 | gain);
                    }

                    if has_in_amp {
                        for i in 0..8 {
                            driver.send_verb(codec, w_node, 0x37000 | (i << 8) | gain);
                        }
                    }

                    match w_type {
                        0 => {
                            // DAC
                            driver.send_verb(codec, w_node, 0x70610);
                            driver.send_verb(codec, w_node, 0x20011);
                        }
                        3 => {
                            // Audio Selector
                            driver.send_verb(codec, w_node, 0x70100);
                        }
                        4 => {
                            // Pin Complex
                            driver.send_verb(codec, w_node, 0x707C0);
                            driver.send_verb(codec, w_node, 0x70C02);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Arrancar el DMA
        let ctl = driver.read_reg32(stream_base + 0x00);
        driver.write_reg32(stream_base + 0x00, ctl | 0x02);
        driver.stream_running = true;

        crate::serial_println!(
            "[HDA] DMA Arrancado. Reproduciendo sonido de notificacion al {}%...",
            driver.volume
        );
    } // Closes if let

    // EXPLICITAMENTE liberar el lock antes del bucle para evitar deadlock
    drop(lock);

    // Polling para detener al terminar (usando el timer del sistema en lugar de LPIB)
    // 48000 Hz * 16-bit (2 bytes) * 2 canales = 192000 bytes/segundo
    // El PIT va a 18.2 Hz (aprox). Ticks = (Bytes / 192000) * 18.2
    let target_ticks = (MP3_PCM.len() as u64 * 182) / (192000 * 10);
    let start_tick = crate::curr_arch::get_ticks();

    loop {
        crate::scheduler::yield_now();

        let current_tick = crate::curr_arch::get_ticks();
        if current_tick.wrapping_sub(start_tick) >= target_ticks {
            let mut lock = HDA.lock();
            if let Some(ref mut driver) = *lock {
                driver.write_reg8(driver.stream_base + 0x00, 0x00); // Clear RUN bit
                driver.stream_running = false;
            }
            break;
        }

        // Comprobar si alguien mas lo detuvo prematuramente con 'hda stop'
        let lock = HDA.lock();
        if let Some(ref driver) = *lock {
            if !driver.stream_running {
                break;
            }
        }
    }
}

pub fn set_volume(vol: u8) {
    let mut lock = HDA.lock();
    if let Some(ref mut driver) = *lock {
        let vol = if vol > 100 { 100 } else { vol };
        driver.volume = vol;
        crate::serial_println!("[HDA] Volumen fijado al {}%", vol);
    }
}

pub fn stop_stream() {
    let mut lock = HDA.lock();
    if let Some(ref mut driver) = *lock {
        let stream_base = driver.stream_base;
        driver.write_reg8(stream_base + 0x00, 0x00);
        driver.stream_running = false;
        crate::serial_println!("[HDA] DMA Detenido.");
    }
}

/// Reproduce un buffer PCM arbitrario (formato 48 kHz, 16-bit, estéreo,
/// interleaved L,R,L,R,... en bytes little-endian). El driver copia
/// internamente los bytes al buffer DMA y arranca la reproducción.
/// Devuelve un error si el driver no está inicializado o si el buffer
/// es inválido.
pub fn play_pcm(pcm_bytes: &[u8]) -> Result<(), &'static str> {
    if pcm_bytes.is_empty() {
        return Err("Buffer PCM vacío");
    }

    let mut lock = HDA.lock();
    let driver = match lock.as_mut() {
        Some(d) => d,
        None => return Err("Driver HDA no inicializado"),
    };

    let stream_base = driver.stream_base;

    // --- Reset COMPLETO del stream para reiniciar LPIB a 0 ---
    // 1. Detener (clear RUN bit)
    driver.write_reg8(stream_base + 0x00, 0x00);
    driver.stream_running = false;
    for _ in 0..10_000 {
        core::hint::spin_loop();
    }

    // 2. Activar SRST (bit 0 del byte bajo de CTL) para reset del stream
    driver.write_reg8(stream_base + 0x00, 0x01); // SRST = 1
    let mut srst_ok = false;
    for _ in 0..100_000 {
        if (driver.read_reg8(stream_base + 0x00) & 0x01) != 0 {
            srst_ok = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !srst_ok {
        crate::serial_println!("[HDA] WARN: Timeout esperando SRST set");
    }

    // 3. Limpiar SRST (poner a 0) para salir del reset
    driver.write_reg8(stream_base + 0x00, 0x00); // SRST = 0
    for _ in 0..100_000 {
        if (driver.read_reg8(stream_base + 0x00) & 0x01) == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    // 4. Reconfigurar todos los registros del stream (LPIB ya es 0 tras el reset)
    let stream_tag: u32 = 1;
    driver.write_reg32(stream_base + 0x00, (stream_tag << 20) as u32); // Tag, sin RUN
    driver.write_reg32(stream_base + 0x08, (128 * 4096) as u32); // CBL
    driver.write_reg16(stream_base + 0x0C, 127); // LVI
    driver.write_reg16(stream_base + 0x12, 0x0011); // FMT: 48kHz, 16-bit, 2ch

    // Reescribir dirección física del BDL (puede que el reset la haya borrado)
    let bdl_phys = vmm::virt_to_phys(driver.bdl_virt as u64);
    driver.write_reg32(stream_base + 0x18, (bdl_phys & 0xFFFFFFFF) as u32);
    driver.write_reg32(stream_base + 0x1C, (bdl_phys >> 32) as u32);

    for _ in 0..10_000 {
        core::hint::spin_loop();
    }
    // --- Fin reset stream ---

    // Copiar el audio al buffer DMA (con padding a múltiplo de 4 bytes)
    let copy_len = core::cmp::min(pcm_bytes.len(), 128 * 4096);
    unsafe {
        core::ptr::write_bytes(driver.audio_virt, 0, 128 * 4096);
        core::ptr::copy_nonoverlapping(pcm_bytes.as_ptr(), driver.audio_virt, copy_len);
    }

    let codec = 0; // Asumimos Codec 0 por defecto
    let nodes_info = driver.send_verb(codec, 0, 0xF0004);
    let starting_node = (nodes_info >> 16) & 0xFF;
    let total_nodes = nodes_info & 0xFF;

    let gain = ((0x7F * driver.volume as u32) / 100) & 0x7F;

    for node in starting_node..(starting_node + total_nodes) {
        let fg_type = driver.send_verb(codec, node as u8, 0xF0005) & 0xFF;
        if fg_type == 0x01 {
            let afg_info = driver.send_verb(codec, node as u8, 0xF0004);
            let afg_start = (afg_info >> 16) & 0xFF;
            let afg_total = afg_info & 0xFF;

            for w_node in afg_start..(afg_start + afg_total) {
                let w_node = w_node as u8;
                let w_caps = driver.send_verb(codec, w_node, 0xF0009);
                let w_type = (w_caps >> 20) & 0xF;

                let has_in_amp = (w_caps & (1 << 1)) != 0;
                let has_out_amp = (w_caps & (1 << 2)) != 0;

                if has_out_amp {
                    driver.send_verb(codec, w_node, 0x3B000 | gain);
                }
                if has_in_amp {
                    for i in 0..8 {
                        driver.send_verb(codec, w_node, 0x37000 | (i << 8) | gain);
                    }
                }

                match w_type {
                    0 => {
                        // DAC
                        driver.send_verb(codec, w_node, 0x70610);
                        driver.send_verb(codec, w_node, 0x20011);
                    }
                    3 => {
                        // Audio Selector
                        driver.send_verb(codec, w_node, 0x70100);
                    }
                    4 => {
                        // Pin Complex
                        driver.send_verb(codec, w_node, 0x707C0);
                        driver.send_verb(codec, w_node, 0x70C02);
                    }
                    _ => {}
                }
            }
        }
    }

    // Arrancar el DMA
    let ctl = driver.read_reg32(stream_base + 0x00);
    driver.write_reg32(stream_base + 0x00, ctl | 0x02);
    driver.stream_running = true;

    crate::serial_println!(
        "[HDA] play_pcm: {} bytes ({} ms) al {}% vol.",
        copy_len,
        (copy_len as u64 * 1000) / (192_000), // 48000 * 2 ch * 2 bytes = 192000 B/s
        driver.volume
    );

    // EXPLICITAMENTE liberar el lock antes del bucle para evitar deadlock
    drop(lock);

    // Esperar a que termine: polling por ticks con timeout
    let target_ticks = (copy_len as u64 * 182) / (192_000 * 10);
    let start_tick = crate::curr_arch::get_ticks();
    let max_wait = target_ticks + 200; // margen extra

    loop {
        crate::scheduler::yield_now();
        let elapsed = crate::curr_arch::get_ticks().wrapping_sub(start_tick);
        if elapsed >= target_ticks {
            // Tiempo cumplido, parar DMA
            let mut lock = HDA.lock();
            if let Some(ref mut driver) = *lock {
                driver.write_reg8(driver.stream_base + 0x00, 0x00);
                driver.stream_running = false;
            }
            break;
        }
        if elapsed > max_wait {
            // Timeout de seguridad
            let mut lock = HDA.lock();
            if let Some(ref mut driver) = *lock {
                driver.write_reg8(driver.stream_base + 0x00, 0x00);
                driver.stream_running = false;
            }
            break;
        }
    }

    Ok(())
}

/// Rellena el buffer DMA y arranca la reproducción sin hacer SRST.
/// Esta función es para streaming: el caller ya debe haber llamado
/// a stop_stream() antes. Más ligero que play_pcm() para chunks.
pub fn play_chunk(pcm_bytes: &[u8]) -> Result<(), &'static str> {
    if pcm_bytes.is_empty() {
        return Err("Buffer PCM vacío");
    }

    let copy_len = core::cmp::min(pcm_bytes.len(), 128 * 4096);

    let mut lock = HDA.lock();
    let driver = match lock.as_mut() {
        Some(d) => d,
        None => return Err("Driver HDA no inicializado"),
    };

    let stream_base = driver.stream_base;

    // Si el stream estaba corriendo, detenerlo
    if driver.stream_running {
        driver.write_reg8(stream_base + 0x00, 0x00);
        for _ in 0..10_000 {
            core::hint::spin_loop();
        }
        driver.stream_running = false;
    }

    // SRST rápido (sin reconfigurar codecs)
    driver.write_reg8(stream_base + 0x00, 0x01);
    for _ in 0..100_000 {
        if (driver.read_reg8(stream_base + 0x00) & 0x01) != 0 {
            break;
        }
        core::hint::spin_loop();
    }
    driver.write_reg8(stream_base + 0x00, 0x00);
    for _ in 0..100_000 {
        if (driver.read_reg8(stream_base + 0x00) & 0x01) == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    // Reconfigurar stream (sin tocar codecs)
    let stream_tag: u32 = 1;
    driver.write_reg32(stream_base + 0x00, (stream_tag << 20) as u32);
    driver.write_reg32(stream_base + 0x08, (128 * 4096) as u32); // CBL
    driver.write_reg16(stream_base + 0x0C, 127); // LVI
    driver.write_reg16(stream_base + 0x12, 0x0011); // FMT: 48kHz, 16-bit, 2ch
    let bdl_phys = vmm::virt_to_phys(driver.bdl_virt as u64);
    driver.write_reg32(stream_base + 0x18, (bdl_phys & 0xFFFFFFFF) as u32);
    driver.write_reg32(stream_base + 0x1C, (bdl_phys >> 32) as u32);

    for _ in 0..5_000 {
        core::hint::spin_loop();
    }

    // Copiar audio al buffer DMA
    unsafe {
        core::ptr::write_bytes(driver.audio_virt, 0, 128 * 4096);
        core::ptr::copy_nonoverlapping(pcm_bytes.as_ptr(), driver.audio_virt, copy_len);
    }

    // Configurar codec (volumen, ruteo DAC, pin) igual que play_pcm
    let codec = 0;
    let nodes_info = driver.send_verb(codec, 0, 0xF0004);
    let starting_node = (nodes_info >> 16) & 0xFF;
    let total_nodes = nodes_info & 0xFF;

    let gain = ((0x7F * driver.volume as u32) / 100) & 0x7F;

    for node in starting_node..(starting_node + total_nodes) {
        let fg_type = driver.send_verb(codec, node as u8, 0xF0005) & 0xFF;
        if fg_type == 0x01 {
            let afg_info = driver.send_verb(codec, node as u8, 0xF0004);
            let afg_start = (afg_info >> 16) & 0xFF;
            let afg_total = afg_info & 0xFF;

            for w_node in afg_start..(afg_start + afg_total) {
                let w_node = w_node as u8;
                let w_caps = driver.send_verb(codec, w_node, 0xF0009);
                let w_type = (w_caps >> 20) & 0xF;

                let has_in_amp = (w_caps & (1 << 1)) != 0;
                let has_out_amp = (w_caps & (1 << 2)) != 0;

                if has_out_amp {
                    driver.send_verb(codec, w_node, 0x3B000 | gain);
                }
                if has_in_amp {
                    for i in 0..8 {
                        driver.send_verb(codec, w_node, 0x37000 | (i << 8) | gain);
                    }
                }

                match w_type {
                    0 => {
                        driver.send_verb(codec, w_node, 0x70610);
                        driver.send_verb(codec, w_node, 0x20011);
                    }
                    3 => {
                        driver.send_verb(codec, w_node, 0x70100);
                    }
                    4 => {
                        driver.send_verb(codec, w_node, 0x707C0);
                        driver.send_verb(codec, w_node, 0x70C02);
                    }
                    _ => {}
                }
            }
        }
    }

    // Arrancar DMA
    let ctl = driver.read_reg32(stream_base + 0x00);
    driver.write_reg32(stream_base + 0x00, ctl | 0x02);
    driver.stream_running = true;

    drop(lock);

    // Esperar a que termine el chunk antes de volver
    // 48000 Hz * 16-bit * 2 canales = 192000 bytes/segundo
    let target_ticks = (copy_len as u64 * 182) / (192_000 * 10);
    let start_tick = crate::curr_arch::get_ticks();

    loop {
        crate::scheduler::yield_now();
        let elapsed = crate::curr_arch::get_ticks().wrapping_sub(start_tick);
        if elapsed >= target_ticks {
            let mut lock = HDA.lock();
            if let Some(ref mut driver) = *lock {
                driver.write_reg8(driver.stream_base + 0x00, 0x00);
                driver.stream_running = false;
            }
            break;
        }
    }

    Ok(())
}
