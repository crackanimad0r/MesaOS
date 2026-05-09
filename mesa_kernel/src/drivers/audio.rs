use x86_64::instructions::port::Port;

pub fn init() {
    // El altavoz PC no requiere inicialización compleja, solo puertos
}

pub struct PcSpeaker;

impl PcSpeaker {
    pub fn play_tone(frequency: u32) {
        if frequency == 0 {
            Self::stop();
            return;
        }

        let divider = 1193180 / frequency;
        
        unsafe {
            let mut port_43 = Port::<u8>::new(0x43);
            let mut port_42 = Port::<u8>::new(0x42);
            let mut port_61 = Port::<u8>::new(0x61);

            port_43.write(0xB6);
            port_42.write((divider & 0xFF) as u8);
            port_42.write(((divider >> 8) & 0xFF) as u8);

            let tmp = port_61.read();
            if tmp != (tmp | 3) {
                port_61.write(tmp | 3);
            }
        }
    }

    pub fn stop() {
        unsafe {
            let mut port_61 = Port::<u8>::new(0x61);
            let tmp = port_61.read() & 0xFC;
            port_61.write(tmp);
        }
    }

    pub fn beep() {
        Self::play_tone(1000);
        // Pequeña pausa
        for _ in 0..10000000 { core::hint::spin_loop(); }
        Self::stop();
    }
}

// Un motor de TTS ultra-simple (basado en fonemas/beeps)
pub fn speak(text: &str) {
    for c in text.to_lowercase().chars() {
        match c {
            'a' | 'e' | 'i' | 'o' | 'u' => PcSpeaker::play_tone(440),
            'b' | 'c' | 'd' | 'f' | 'g' => PcSpeaker::play_tone(330),
            'h' | 'j' | 'k' | 'l' | 'm' => PcSpeaker::play_tone(550),
            'n' | 'p' | 'q' | 'r' | 's' => PcSpeaker::play_tone(220),
            't' | 'v' | 'w' | 'x' | 'y' | 'z' => PcSpeaker::play_tone(660),
            ' ' => {
                PcSpeaker::stop();
                for _ in 0..5000000 { core::hint::spin_loop(); }
                continue;
            }
            _ => continue,
        }
        // Duración del "fonema"
        for _ in 0..5000000 { core::hint::spin_loop(); }
        PcSpeaker::stop();
        for _ in 0..1000000 { core::hint::spin_loop(); }
    }
}
