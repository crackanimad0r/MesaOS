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
pub const LAPIC_TICONT: u32 = 0x380;
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
    crate::serial_println!("[APIC] Local APIC initialized at {:#x}", info.local_apic_address);

    // Read the actual APIC ID of the BSP
    let bsp_apic_id = (lapic.read(LAPIC_ID) >> 24) as u8;
    crate::serial_println!("[APIC] BSP APIC ID: {}", bsp_apic_id);

    if info.ioapic_address != 0 {
        let ioapic = IoApic::new(info.ioapic_address);
        
        // Find GSI for IRQ 0 (Timer) and IRQ 1 (Keyboard)
        let mut timer_gsi = 2; // Default for IRQ 0
        let mut timer_flags = 0;
        let mut kbd_gsi = 1;   // Default for IRQ 1
        let mut kbd_flags = 0;
        
        for ovr in &info.overrides {
            crate::serial_println!("[APIC] MADT Override: Source IRQ {} -> GSI {} (flags {:#x})", ovr.source, ovr.global_system_interrupt, ovr.flags);
            if ovr.source == 0 {
                timer_gsi = ovr.global_system_interrupt as u8;
                timer_flags = ovr.flags;
            }
            if ovr.source == 1 {
                kbd_gsi = ovr.global_system_interrupt as u8;
                kbd_flags = ovr.flags;
            }
        }
        
        crate::serial_println!("[APIC] Routing Timer: IRQ 0 -> GSI {} (flags: {:#x})", timer_gsi, timer_flags);
        crate::serial_println!("[APIC] Routing Keyboard: IRQ 1 -> GSI {} (flags: {:#x})", kbd_gsi, kbd_flags);
        
        ioapic.set_redirection(timer_gsi, crate::arch::x86_64::interrupts::TIMER_INTERRUPT_ID, timer_flags, bsp_apic_id);
        // PRUEBA: Usar 0xFF (Broadcast) para el teclado en lugar de solo el BSP
        ioapic.set_redirection(kbd_gsi, crate::arch::x86_64::interrupts::KEYBOARD_INTERRUPT_ID, kbd_flags, 0xFF);
        
        // IRQ 11: WiFi (Realtek 8822CE)
        // PCI IRQs are Level-Triggered, Active-Low
        ioapic.set_redirection(11, crate::arch::x86_64::interrupts::WIFI_INTERRUPT_ID, 0x000F, 0xFF);
        
        // DUMP DE VERIFICACIÓN (Leer lo que acabamos de escribir para estar seguros)
        let low_kbd = ioapic.read(0x10 + (kbd_gsi as u32 * 2));
        let high_kbd = ioapic.read(0x10 + (kbd_gsi as u32 * 2) + 1);
        crate::serial_println!("[APIC] Verificación KBD GSI {}: {:#x}_{:#x}", kbd_gsi, high_kbd, low_kbd);

        // DUMP COMPLETO DE REDIRECCIONES (IRQ 0-15)
        crate::serial_println!("[APIC] Redirection Table Dump:");
        for i in 0..16 {
            let low = ioapic.read(0x10 + (i * 2));
            let high = ioapic.read(0x10 + (i * 2) + 1);
            crate::serial_println!("  [IRQ {}] GSI {}: {:#011x}_{:#010x}", i, i, high, low);
        }

        crate::serial_println!("[APIC] I/O APIC initialized and IRQs routed to BSP APIC ID {}", bsp_apic_id);
    }

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
