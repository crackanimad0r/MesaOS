// mesa_kernel/src/interrupts/idt.rs

use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

use super::{KEYBOARD_INTERRUPT_ID, TIMER_INTERRUPT_ID, WIFI_INTERRUPT_ID};
use super::{handlers, timer};

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // Excepciones básicas
        idt.breakpoint.set_handler_fn(handlers::breakpoint_handler);
        idt.invalid_opcode.set_handler_fn(handlers::invalid_opcode_handler);

        // IMPORTANTES para evitar Double Fault "misterioso"
        idt.general_protection_fault
            .set_handler_fn(handlers::general_protection_fault_handler);
        idt.segment_not_present
            .set_handler_fn(handlers::segment_not_present_handler);
        idt.stack_segment_fault
            .set_handler_fn(handlers::stack_segment_fault_handler);
        idt.invalid_tss
            .set_handler_fn(handlers::invalid_tss_handler);

        idt.page_fault.set_handler_fn(handlers::page_fault_handler);

        // Double fault con IST dedicado
        unsafe {
            idt.double_fault
                .set_handler_fn(handlers::double_fault_handler)
                .set_stack_index(crate::arch::x86_64::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        // IRQs
        idt[TIMER_INTERRUPT_ID].set_handler_fn(timer::timer_interrupt_handler);
        idt[KEYBOARD_INTERRUPT_ID].set_handler_fn(handlers::keyboard_handler);
        idt[WIFI_INTERRUPT_ID].set_handler_fn(handlers::wifi_handler);

        idt
    };
}