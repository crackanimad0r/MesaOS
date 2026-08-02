use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::instructions::port::Port;
use x86_64::registers::model_specific::Msr;

pub const LAPIC_BASE: u64 = 0xFEE00000;

// LAPIC Registers (offsets)
pub const LAPIC_ID: u32 = 0x20;
pub const LAPIC_VER: u32 = 0x30;
pub const LAPIC_TPR: u32 = 0x80;
pub const LAPIC_EOI: u32 = 0x0B0;
pub const LAPIC_SVR: u32 = 0x0F0;
pub const LAPIC_ESR: u32 = 0x280;
pub const LAPIC_ICR_LOW: u32 = 0x300;
pub const LAPIC_ICR_HIGH: u32 = 0x310;
pub const LAPIC_LVT_TIMER: u32 = 0x320;
pub const LAPIC_LVT_PC: u32 = 0x340;
pub const LAPIC_LVT_LINT0: u32 = 0x350;
pub const LAPIC_LVT_LINT1: u32 = 0x360;
pub const LAPIC_LVT_ERR: u32 = 0x370;
pub const LAPIC_TICCONT: u32 = 0x380; // Timer Initial Count
pub const LAPIC_TICCNT: u32 = 0x390;
pub const LAPIC_TDCR: u32 = 0x3E0;

/// Local APIC Controller
pub struct LocalApic {
    base: u64,
}

impl LocalApic {
    pub fn new(physical_base: u64) -> Self {
        let hhdm = crate::limine_req::hhdm_offset().unwrap_or(0);
        Self {
            base: physical_base + hhdm,
        }
    }

    pub unsafe fn read(&self, reg: u32) -> u32 {
        core::ptr::read_volatile((self.base + reg as u64) as *const u32)
    }

    pub unsafe fn write(&self, reg: u32, value: u32) {
        core::ptr::write_volatile((self.base + reg as u64) as *mut u32, value)
    }

    pub unsafe fn init(&self) {
        // Enforce APIC enable via MSR
        let mut apic_base_msr = Msr::new(0x1B);
        let mut value = apic_base_msr.read();
        value |= 1 << 11; // Enable bit
        apic_base_msr.write(value);

        // Set Spurious Interrupt Vector and enable APIC
        // Vector 0xFF, bit 8 is enable
        self.write(LAPIC_SVR, self.read(LAPIC_SVR) | 0x1FF);

        // Clear Task Priority Register to enable all interrupts
        self.write(LAPIC_TPR, 0);
    }

    pub unsafe fn eoi(&self) {
        self.write(LAPIC_EOI, 0);
    }
}

/// Frecuencia del timer LAPIC en Hz (calibrada una vez en el BSP).
static LAPIC_TIMER_HZ: AtomicU64 = AtomicU64::new(0);

/// Devuelve la frecuencia calibrada del timer LAPIC (0 si aún no se calibró).
pub fn lapic_timer_hz() -> u64 {
    LAPIC_TIMER_HZ.load(Ordering::Relaxed)
}

/// Calibra la frecuencia del timer LAPIC usando el PIT (canal 0, modo 0,
/// one-shot) durante un periodo conocido y devuelve los Hz resultantes.
///
/// IMPORTANTE: llamar con interrupciones deshabilitadas (arranque temprano).
pub unsafe fn calibrate_lapic_timer() -> u64 {
    const PIT_FREQ: u64 = 1_193_182;
    let period_ms: u64 = 10;
    let pit_count: u16 = ((PIT_FREQ * period_ms) / 1000) as u16; // ~11931 ticks

    let info = crate::acpi::get_info();
    let lapic_addr = info
        .map(|i| i.local_apic_address)
        .filter(|&a| a != 0)
        .unwrap_or(LAPIC_BASE);
    let lapic = LocalApic::new(lapic_addr);

    unsafe {
        // PIT: canal 0, modo 0 (interrupt on terminal count), lobyte/hibyte, binario
        let mut cmd: Port<u8> = Port::new(0x43);
        let mut data: Port<u8> = Port::new(0x40);
        cmd.write(0x30);
        data.write((pit_count & 0xFF) as u8);
        data.write(((pit_count >> 8) & 0xFF) as u8);

        // LAPIC timer: one-shot (bit 17 = 0), vector TIMER_INTERRUPT_ID, divider 1
        lapic.write(
            LAPIC_LVT_TIMER,
            crate::arch::x86_64::interrupts::TIMER_INTERRUPT_ID as u32,
        );
        lapic.write(LAPIC_TDCR, 0b1011); // dividir por 1
        lapic.write(LAPIC_TICCONT, 0xFFFF_FFFF);

        // Esperar a que el PIT llegue a terminal count (bit 7 del status = OUT alto)
        loop {
            cmd.write(0xE2); // read-back: latchear status del canal 0
            let status: u8 = data.read();
            if status & 0x80 != 0 {
                break;
            }
        }

        let remaining = lapic.read(LAPIC_TICCNT);
        let elapsed = 0xFFFF_FFFFu32.wrapping_sub(remaining) as u64;
        let hz = elapsed * PIT_FREQ / pit_count as u64;
        hz
    }
}

/// Habilita y limpia el Local APIC de la CPU actual (SVR enable + TPR 0).
/// Cada núcleo (BSP y APs) debe llamarlo antes de usar su LAPIC.
pub unsafe fn init_local_apic_cpu() {
    let info = crate::acpi::get_info();
    let lapic_addr = info
        .map(|i| i.local_apic_address)
        .filter(|&a| a != 0)
        .unwrap_or(LAPIC_BASE);
    LocalApic::new(lapic_addr).init();
}

/// Programa el timer LAPIC en modo periódico a ~100 Hz.
/// Debe llamarse en CADA núcleo (BSP y APs).
pub unsafe fn init_lapic_timer(hz: u64) {
    let info = crate::acpi::get_info();
    let lapic_addr = info
        .map(|i| i.local_apic_address)
        .filter(|&a| a != 0)
        .unwrap_or(LAPIC_BASE);
    let lapic = LocalApic::new(lapic_addr);

    // Dividir por 1 y disparar cada (hz/100) ticks → ~100 interrupciones/s.
    let ticks_per_period: u32 = (hz / 100).max(1) as u32;

    unsafe {
        lapic.write(LAPIC_TDCR, 0b1011); // dividir por 1
        lapic.write(
            LAPIC_LVT_TIMER,
            (crate::arch::x86_64::interrupts::TIMER_INTERRUPT_ID as u32) | (1 << 17), // periódico
        );
        lapic.write(LAPIC_TICCONT, ticks_per_period);
    }
}

pub const IOAPIC_REGSEL: u32 = 0x00;
pub const IOAPIC_IOWIN: u32 = 0x10;

pub struct IoApic {
    base: u64,
}

impl IoApic {
    pub fn new(physical_base: u64) -> Self {
        let hhdm = crate::limine_req::hhdm_offset().unwrap_or(0);
        Self {
            base: physical_base + hhdm,
        }
    }

    pub unsafe fn read(&self, reg: u32) -> u32 {
        core::ptr::write_volatile(self.base as *mut u32, reg);
        core::ptr::read_volatile((self.base + IOAPIC_IOWIN as u64) as *const u32)
    }

    pub unsafe fn write(&self, reg: u32, value: u32) {
        core::ptr::write_volatile(self.base as *mut u32, reg);
        core::ptr::write_volatile((self.base + IOAPIC_IOWIN as u64) as *mut u32, value);
    }

    pub unsafe fn set_redirection(&self, irq: u8, vector: u8, flags: u16, dest_apic_id: u8) {
        let low_index = 0x10 + (irq as u32 * 2);
        let high_index = low_index + 1;

        let mut low = vector as u32;

        // ACPI MADT flags:
        // Polarity: bits 0-1
        // (01 = Active High, 11 = Active Low)
        let polarity = flags & 0x03;
        if polarity == 0x03 {
            low |= 1 << 13; // Set Polarity bit to Active Low
        }

        // Trigger Mode: bits 2-3
        // (01 = Edge, 11 = Level)
        let trigger = (flags >> 2) & 0x03;
        if trigger == 0x03 {
            low |= 1 << 15; // Set Trigger Mode bit to Level
        }

        // Low 32 bits: vector, delivery mode (fixed=0), destination mode (physical=0),
        // interrupt mask (enabled=0), trigger mode, polarity
        self.write(low_index, low);
        // High 32 bits: destination APIC ID
        self.write(high_index, (dest_apic_id as u32) << 24);
    }
}

/// Helper to initialize APIC system
pub unsafe fn init_apic() -> Result<(), &'static str> {
    let info = crate::acpi::get_info().ok_or("ACPI not initialized")?;

    if info.local_apic_address == 0 {
        return Err("No Local APIC address found in ACPI");
    }

    disable_pic();
    crate::serial_println!("[APIC] Legacy PIC disabled");

    let lapic = LocalApic::new(info.local_apic_address);
    lapic.init();
    crate::serial_println!(
        "[APIC] Local APIC initialized at {:#x}",
        info.local_apic_address
    );

    // Read the actual APIC ID of the BSP
    let bsp_apic_id = (lapic.read(LAPIC_ID) >> 24) as u8;
    crate::serial_println!("[APIC] BSP APIC ID: {}", bsp_apic_id);

    if info.ioapic_address != 0 {
        let ioapic = IoApic::new(info.ioapic_address);

        // Find GSI for IRQ 0 (Timer) and IRQ 1 (Keyboard)
        let mut timer_gsi = 2; // Default for IRQ 0
        let mut timer_flags = 0;
        let mut kbd_gsi = 1; // Default for IRQ 1
        let mut kbd_flags = 0;

        for ovr in &info.overrides {
            crate::serial_println!(
                "[APIC] MADT Override: Source IRQ {} -> GSI {} (flags {:#x})",
                ovr.source,
                ovr.global_system_interrupt,
                ovr.flags
            );
            if ovr.source == 0 {
                timer_gsi = ovr.global_system_interrupt as u8;
                timer_flags = ovr.flags;
            }
            if ovr.source == 1 {
                kbd_gsi = ovr.global_system_interrupt as u8;
                kbd_flags = ovr.flags;
            }
        }

        crate::serial_println!(
            "[APIC] Routing Timer: IRQ 0 -> GSI {} (flags: {:#x})",
            timer_gsi,
            timer_flags
        );
        crate::serial_println!(
            "[APIC] Routing Keyboard: IRQ 1 -> GSI {} (flags: {:#x})",
            kbd_gsi,
            kbd_flags
        );

        ioapic.set_redirection(
            timer_gsi,
            crate::arch::x86_64::interrupts::TIMER_INTERRUPT_ID,
            timer_flags,
            bsp_apic_id,
        );
        // Enmascarar IRQ0 (PIT): a partir de ahora el timer lo lleva el LAPIC.
        let low_timer = ioapic.read(0x10 + (timer_gsi as u32 * 2));
        ioapic.write(0x10 + (timer_gsi as u32 * 2), low_timer | (1 << 16));
        // PRUEBA: Usar 0xFF (Broadcast) para el teclado en lugar de solo el BSP
        ioapic.set_redirection(
            kbd_gsi,
            crate::arch::x86_64::interrupts::KEYBOARD_INTERRUPT_ID,
            kbd_flags,
            bsp_apic_id,
        );

        // IRQ 11: WiFi (Realtek 8822CE)
        // PCI IRQs are Level-Triggered, Active-Low
        ioapic.set_redirection(
            11,
            crate::arch::x86_64::interrupts::WIFI_INTERRUPT_ID,
            0x000F,
            bsp_apic_id,
        );

        // DUMP DE VERIFICACIÓN (Leer lo que acabamos de escribir para estar seguros)
        let low_kbd = ioapic.read(0x10 + (kbd_gsi as u32 * 2));
        let high_kbd = ioapic.read(0x10 + (kbd_gsi as u32 * 2) + 1);
        crate::serial_println!(
            "[APIC] Verificación KBD GSI {}: {:#x}_{:#x}",
            kbd_gsi,
            high_kbd,
            low_kbd
        );

        // DUMP COMPLETO DE REDIRECCIONES (IRQ 0-15)
        crate::serial_println!("[APIC] Redirection Table Dump:");
        for i in 0..16 {
            let low = ioapic.read(0x10 + (i * 2));
            let high = ioapic.read(0x10 + (i * 2) + 1);
            crate::serial_println!("  [IRQ {}] GSI {}: {:#011x}_{:#010x}", i, i, high, low);
        }

        crate::serial_println!(
            "[APIC] I/O APIC initialized and IRQs routed to BSP APIC ID {}",
            bsp_apic_id
        );
    }

    // Calibrar y arrancar el timer LAPIC del BSP (~100 Hz).
    // El PIT queda enmascarado: cada núcleo usa su propio timer LAPIC.
    let hz = calibrate_lapic_timer();
    LAPIC_TIMER_HZ.store(hz.max(1), Ordering::SeqCst);
    crate::serial_println!("[APIC] LAPIC timer calibrado: {} Hz", hz);
    init_lapic_timer(hz);

    Ok(())
}

/// Send End of Interrupt to the active controller
pub fn send_eoi() {
    unsafe {
        if let Some(info) = crate::acpi::get_info() {
            if info.local_apic_address != 0 {
                let lapic = LocalApic::new(info.local_apic_address);
                lapic.eoi();
                return;
            }
        }

        // Fallback to legacy PIC
        let mut pic: Port<u8> = Port::new(0x20);
        pic.write(0x20);
    }
}

/// Disable the old 8259 PIC
pub unsafe fn disable_pic() {
    // Mask all interrupts on both PICs
    let mut pic1_mask: Port<u8> = Port::new(0x21);
    let mut pic2_mask: Port<u8> = Port::new(0xA1);
    pic1_mask.write(0xFF);
    pic2_mask.write(0xFF);
}
