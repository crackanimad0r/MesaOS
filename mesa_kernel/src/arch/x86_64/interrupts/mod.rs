// mesa_kernel/src/interrupts/mod.rs

pub mod idt;
pub mod handlers;
pub mod timer;
pub mod apic;

use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin::Mutex;
use x86_64::instructions::port::Port;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub const TIMER_INTERRUPT_ID: u8 = PIC_1_OFFSET;
pub const KEYBOARD_INTERRUPT_ID: u8 = PIC_1_OFFSET + 1;
pub const WIFI_INTERRUPT_ID: u8 = PIC_1_OFFSET + 11; // IRQ 11 = 0x2B

lazy_static! {
    pub static ref PICS: Mutex<ChainedPics> =
        Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });
}

pub fn init_idt() {
    idt::IDT.load();
}

pub fn init_pic() {
    unsafe {
        PICS.lock().initialize();

        // PIC1: habilitar IRQ0 (timer) + IRQ1 (teclado)
        let mut pic1_mask: Port<u8> = Port::new(0x21);
        let mask: u8 = 0xFF & !0b0000_0011; // bits 0 y 1 habilitados
        pic1_mask.write(mask);

        // PIC2: todo enmascarado
        let mut pic2_mask: Port<u8> = Port::new(0xA1);
        pic2_mask.write(0xFF);
    }
}