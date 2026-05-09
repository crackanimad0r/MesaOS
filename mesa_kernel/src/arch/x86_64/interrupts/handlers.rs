// mesa_kernel/src/interrupts/handlers.rs

use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};

/// En excepciones/IRQs, imprimir con locks puede ser peligroso.
/// Pero aquí lo dejamos por serial para debug. Si te vuelve a fallar por locks,
/// migramos a "emergency serial" sin locks.
fn halt_forever() -> ! {
    x86_64::instructions::interrupts::disable();
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    crate::mesa_println!("[EXCEPTION] BREAKPOINT\n{:#?}", stack_frame);
}

pub extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    crate::mesa_println!("\n[EXCEPTION] INVALID OPCODE (#UD)");
    crate::mesa_println!("  RIP = {:#018x}", stack_frame.instruction_pointer.as_u64());
    crate::mesa_println!("  RSP = {:#018x}", stack_frame.stack_pointer.as_u64());
    halt_forever();
}

pub extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    crate::mesa_println!("\n[EXCEPTION] GENERAL PROTECTION FAULT (#GP)");
    crate::mesa_println!("  Err = {:#x}", error_code);
    crate::mesa_println!("  RIP = {:#018x}", stack_frame.instruction_pointer.as_u64());
    crate::mesa_println!("  RSP = {:#018x}", stack_frame.stack_pointer.as_u64());
    halt_forever();
}

pub extern "x86-interrupt" fn stack_segment_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    crate::mesa_println!("\n[EXCEPTION] STACK SEGMENT FAULT (#SS)");
    crate::mesa_println!("  Err = {:#x}", error_code);
    crate::mesa_println!("  RIP = {:#018x}", stack_frame.instruction_pointer.as_u64());
    crate::mesa_println!("  RSP = {:#018x}", stack_frame.stack_pointer.as_u64());
    halt_forever();
}

pub extern "x86-interrupt" fn segment_not_present_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    crate::mesa_println!("\n[EXCEPTION] SEGMENT NOT PRESENT (#NP)");
    crate::mesa_println!("  error_code = {:#x}", error_code);
    crate::mesa_println!("  RIP        = {:#x}", stack_frame.instruction_pointer.as_u64());
    crate::mesa_println!("  RSP        = {:#x}", stack_frame.stack_pointer.as_u64());
    halt_forever();
}

pub extern "x86-interrupt" fn invalid_tss_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    crate::mesa_println!("\n[EXCEPTION] INVALID TSS (#TS)");
    crate::mesa_println!("  error_code = {:#x}", error_code);
    crate::mesa_println!("  RIP        = {:#x}", stack_frame.instruction_pointer.as_u64());
    crate::mesa_println!("  RSP        = {:#x}", stack_frame.stack_pointer.as_u64());
    halt_forever();
}

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    crate::mesa_println!("\n[EXCEPTION] PAGE FAULT (#PF)");
    crate::mesa_println!("  CR2 = {:#018x}", Cr2::read().map(|v| v.as_u64()).unwrap_or(0));
    crate::mesa_println!("  Err = {:?}", error_code);
    crate::mesa_println!("  RIP = {:#018x}", stack_frame.instruction_pointer.as_u64());
    crate::mesa_println!("  RSP = {:#018x}", stack_frame.stack_pointer.as_u64());
    halt_forever();
}

pub extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    crate::mesa_println!("\n[FATAL] DOUBLE FAULT");
    crate::mesa_println!("  RIP = {:#x}", stack_frame.instruction_pointer.as_u64());
    crate::mesa_println!("  RSP = {:#x}", stack_frame.stack_pointer.as_u64());
    halt_forever();
}

// =======================
// IRQ TECLADO
// =======================

pub extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    // Leer scancode PS/2
    let mut data: Port<u8> = Port::new(0x60);
    let scancode: u8 = unsafe { data.read() };

    // Procesar sin bloquear (try_lock)
    crate::drivers::keyboard::handle_interrupt_simple(scancode);

    // EOI
    crate::arch::x86_64::interrupts::apic::send_eoi();
}

// =======================
// IRQ WIFI (Realtek)
// =======================

pub extern "x86-interrupt" fn wifi_handler(_stack_frame: InterruptStackFrame) {
    // C wifi driver ha sido eliminado, el handler solo envía EOI por ahora

    // EOI
    crate::arch::x86_64::interrupts::apic::send_eoi();
}