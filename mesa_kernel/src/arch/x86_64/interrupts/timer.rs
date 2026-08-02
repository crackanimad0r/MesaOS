// mesa_kernel/src/interrupts/timer.rs

use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::InterruptStackFrame;

static TICKS: AtomicU64 = AtomicU64::new(0);

pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // En SMP cada núcleo recibe su propio tick de LAPIC timer. El reloj global
    // (TICKS) y el polling de PS/2/touchpad los gestiona SOLO el BSP (cpu 0)
    // para mantener la frecuencia de reloj y no leer el puerto 0x60 a la vez.
    let is_bsp = crate::smp::current_cpu_id() == 0;

    if is_bsp {
        let ticks = TICKS.fetch_add(1, Ordering::Relaxed);

        // VISUAL FEEDBACK: DESHABILITADO para uso normal
        // if ticks % 100 == 0 {
        //    crate::mesa_print!(".");
        // }

        // POLLING FALLBACK PARA TECLADO Y RATON (BARE METAL FIX)
        // Verificamos el Bit 0 (Output Buffer Full) del puerto de estado 0x64.
        // Bit 5 del status indica qué dispositivo: 0=teclado, 1=ratón
        use x86_64::instructions::port::Port;
        let mut status_port: Port<u8> = Port::new(0x64);
        let mut data_port: Port<u8> = Port::new(0x60);

        // Leemos el estado sin bloquear
        let status = unsafe { status_port.read() };

        // Si hay datos esperando (Bit 0 set), los leemos y procesamos
        if (status & 0x01) != 0 {
            let data = unsafe { data_port.read() };
            if (status & 0x20) != 0 {
                // Bit 5 set → mouse data
                crate::drivers::mouse::handle_data(data);
            } else {
                // Keyboard data
                crate::drivers::keyboard::handle_interrupt_simple(data);
            }
        }

        // Poll touchpad data (SMBus/Elan) cada ~4 ticks (~20ms @ 250Hz)
        if ticks % 4 == 0 {
            crate::drivers::touchpad::poll();
        }
    }

    // IMPORTANTE: Llamar a timer_tick() que hace el scheduling per-CPU
    crate::scheduler::timer_tick();

    // EOI
    crate::arch::x86_64::interrupts::apic::send_eoi();
}

pub fn get_ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
