// mesa_kernel/src/interrupts/timer.rs

use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::InterruptStackFrame;

static TICKS: AtomicU64 = AtomicU64::new(0);

pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let ticks = TICKS.fetch_add(1, Ordering::Relaxed);
    
    // VISUAL FEEDBACK: DESHABILITADO para uso normal
    // if ticks % 100 == 0 {
    //    crate::mesa_print!("."); 
    // }

    // POLLING FALLBACK PARA TECLADO (BARE METAL FIX)
    // Si la interrupción del teclado (IRQ 1) falla, esto leerá el dato manualmente.
    // Verificamos el Bit 0 (Output Buffer Full) del puerto de estado 0x64.
    use x86_64::instructions::port::Port;
    let mut status_port: Port<u8> = Port::new(0x64);
    let mut data_port: Port<u8> = Port::new(0x60);
    
    // Leemos el estado sin bloquear
    let status = unsafe { status_port.read() };
    
    // Si hay datos esperando (Bit 0 set), los leemos y procesamos
    if (status & 0x01) != 0 {
        let scancode = unsafe { data_port.read() };
        // Llamamos al handler simple que decodifica y mete al buffer
        // Nota: mesa_print!("*") ya está dentro de handle_interrupt_simple
        crate::drivers::keyboard::handle_interrupt_simple(scancode);
    }

    // IMPORTANTE: Llamar a timer_tick() que hace el scheduling
    crate::scheduler::timer_tick();
    
    // EOI
    crate::arch::x86_64::interrupts::apic::send_eoi();
}

pub fn get_ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
