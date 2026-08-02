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
    crate::mesa_println!(
        "  RIP        = {:#x}",
        stack_frame.instruction_pointer.as_u64()
    );
    crate::mesa_println!("  RSP        = {:#x}", stack_frame.stack_pointer.as_u64());
    halt_forever();
}

pub extern "x86-interrupt" fn invalid_tss_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    crate::mesa_println!("\n[EXCEPTION] INVALID TSS (#TS)");
    crate::mesa_println!("  error_code = {:#x}", error_code);
    crate::mesa_println!(
        "  RIP        = {:#x}",
        stack_frame.instruction_pointer.as_u64()
    );
    crate::mesa_println!("  RSP        = {:#x}", stack_frame.stack_pointer.as_u64());
    halt_forever();
}

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use crate::memory::page_flags;
    use x86_64::registers::control::Cr2;

    let fault_addr = Cr2::read().map(|v| v.as_u64()).unwrap_or(0);
    let is_write = (error_code.bits() & 2) != 0;

    // COW handling: if it's a write to a COW page in user space
    if is_write && fault_addr < 0x0000_8000_0000_0000 {
        let hhdm = crate::memory::vmm::hhdm_offset();
        // Walk the current page tables to find the PTE
        let cr3: u64;
        unsafe {
            core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
        }
        let pml4_virt = hhdm + (cr3 & !0xFFF);
        let pml4_idx = (fault_addr >> 39) & 0x1FF;
        let pdpt_idx = (fault_addr >> 30) & 0x1FF;
        let pd_idx = (fault_addr >> 21) & 0x1FF;
        let pt_idx = (fault_addr >> 12) & 0x1FF;

        unsafe {
            let pml4e = *(pml4_virt as *const u64).add(pml4_idx as usize);
            if pml4e & page_flags::PRESENT == 0 {
                halt_forever();
            }
            let pdpt_virt = hhdm + (pml4e & !0xFFF);
            let pdpte = *(pdpt_virt as *const u64).add(pdpt_idx as usize);
            if pdpte & page_flags::PRESENT == 0 {
                halt_forever();
            }
            let pd_virt = hhdm + (pdpte & !0xFFF);
            let pde = *(pd_virt as *const u64).add(pd_idx as usize);
            if pde & page_flags::PRESENT == 0 {
                halt_forever();
            }
            let pt_virt = hhdm + (pde & !0xFFF);
            let pte = *(pt_virt as *const u64).add(pt_idx as usize);

            if pte & page_flags::PRESENT == 0 {
                halt_forever();
            }

            // Check if this is a COW page
            if pte & page_flags::COW != 0 {
                let old_phys = pte & !0xFFF;
                // Allocate a new frame
                if let Some(new_phys) = crate::memory::pmm::alloc_frame() {
                    let old_virt = hhdm + old_phys;
                    let new_virt = hhdm + new_phys;
                    // Copy page content
                    core::ptr::copy_nonoverlapping(
                        old_virt as *const u8,
                        new_virt as *mut u8,
                        4096,
                    );
                    // Update PTE: new phys, writable, no COW
                    let new_pte =
                        new_phys | (pte & !(!0xFFF | page_flags::COW)) | page_flags::WRITABLE;
                    *(pt_virt as *mut u64).add(pt_idx as usize) = new_pte;
                    // Flush TLB for this page
                    core::arch::asm!("invlpg [{}]", in(reg) fault_addr, options(nostack, preserves_flags));
                    return;
                }
            }
        }

        // If COW handling failed, fall through to normal panic
        crate::mesa_println!("\n[EXCEPTION] PAGE FAULT (#PF) - COW handling failed");
        crate::mesa_println!("  CR2 = {:#018x}", fault_addr);
        crate::mesa_println!("  Err = {:?}", error_code);
        crate::mesa_println!("  RIP = {:#018x}", stack_frame.instruction_pointer.as_u64());
        crate::mesa_println!("  RSP = {:#018x}", stack_frame.stack_pointer.as_u64());
        halt_forever();
    }

    crate::mesa_println!("\n[EXCEPTION] PAGE FAULT (#PF)");
    crate::mesa_println!("  CR2 = {:#018x}", fault_addr);
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
