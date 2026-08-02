//! Global Descriptor Table con soporte para Ring 0 y Ring 3
//!
//! En modo SMP cada núcleo tiene su propia GDT/TSS (el descriptor TSS de la
//! GDT codifica la base, por lo que no puede compartirse). Esta GDT temprana
//! (lazy_static) se usa solo durante el arranque, antes de que `smp::init`
//! cargue las GDT/TSS per-CPU.

use core::cell::UnsafeCell;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const SYSCALL_IST_INDEX: u16 = 1;

const STACK_SIZE: usize = 4096 * 5; // 20 KiB

static mut DOUBLE_FAULT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
static mut SYSCALL_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
static mut KERNEL_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

/// TSS mutable para poder actualizar RSP0
struct MutableTss {
    inner: UnsafeCell<TaskStateSegment>,
}

unsafe impl Sync for MutableTss {}

impl MutableTss {
    const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(TaskStateSegment::new()),
        }
    }

    fn init(&self) {
        unsafe {
            let tss = &mut *self.inner.get();

            tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
                let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(DOUBLE_FAULT_STACK));
                stack_start + STACK_SIZE as u64
            };

            tss.interrupt_stack_table[SYSCALL_IST_INDEX as usize] = {
                let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(SYSCALL_STACK));
                stack_start + STACK_SIZE as u64
            };

            tss.privilege_stack_table[0] = {
                let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(KERNEL_STACK));
                stack_start + STACK_SIZE as u64
            };
        }
    }

    fn set_rsp0(&self, stack_top: u64) {
        unsafe {
            let tss = &mut *self.inner.get();
            tss.privilege_stack_table[0] = VirtAddr::new(stack_top);
        }
    }

    fn as_ptr(&self) -> *const TaskStateSegment {
        self.inner.get()
    }
}

static TSS: MutableTss = MutableTss::new();

/// Construye una GDT con los segmentos estándar + el descriptor TSS apuntando
/// al TSS dado. Usada por cada núcleo en modo SMP (la GDT debe viv ir estática,
/// por lo que `smp::init` la asigna en el heap y nunca la libera).
pub fn build_gdt(tss: &'static TaskStateSegment) -> (GlobalDescriptorTable, Selectors) {
    let mut gdt = GlobalDescriptorTable::empty();

    let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
    let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
    let user_data_selector = gdt.append(Descriptor::user_data_segment());
    let user_code_selector = gdt.append(Descriptor::user_code_segment());

    // SAFETY: el TSS es estático y válido durante toda la vida del kernel.
    let tss_selector = unsafe { gdt.append(Descriptor::tss_segment(tss)) };

    (
        gdt,
        Selectors {
            kernel_code_selector,
            kernel_data_selector,
            user_code_selector,
            user_data_selector,
            tss_selector,
        },
    )
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        // SAFETY: El TSS es estático y válido
        let tss_selector = unsafe {
            Descriptor::tss_segment(&*TSS.as_ptr())
        };
        let mut gdt = GlobalDescriptorTable::new();

        let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment());
        let tss_selector = gdt.append(tss_selector);

        (
            gdt,
            Selectors {
                kernel_code_selector,
                kernel_data_selector,
                user_code_selector,
                user_data_selector,
                tss_selector,
            },
        )
    };
}

#[derive(Debug, Clone, Copy)]
pub struct Selectors {
    pub kernel_code_selector: SegmentSelector,
    pub kernel_data_selector: SegmentSelector,
    pub user_code_selector: SegmentSelector,
    pub user_data_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

static SELECTORS: Mutex<Option<Selectors>> = Mutex::new(None);

/// Guarda los selectores usados por el resto del kernel (los valores son los
/// mismos en todos los núcleos, ya que la GDT per-CPU tiene el mismo layout).
pub fn store_selectors(selectors: Selectors) {
    *SELECTORS.lock() = Some(selectors);
}

pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS, DS, ES, SS};
    use x86_64::instructions::tables::load_tss;

    x86_64::instructions::interrupts::disable();

    // Inicializar TSS antes de cargar GDT
    TSS.init();

    GDT.0.load();

    unsafe {
        CS::set_reg(GDT.1.kernel_code_selector);
        DS::set_reg(GDT.1.kernel_data_selector);
        ES::set_reg(GDT.1.kernel_data_selector);
        SS::set_reg(GDT.1.kernel_data_selector);
        load_tss(GDT.1.tss_selector);
    }

    *SELECTORS.lock() = Some(GDT.1);

    crate::serial_println!(
        "[GDT] Kernel CS={:#x} DS={:#x}",
        GDT.1.kernel_code_selector.0,
        GDT.1.kernel_data_selector.0
    );
    crate::serial_println!(
        "[GDT] User CS={:#x} DS={:#x}",
        GDT.1.user_code_selector.0,
        GDT.1.user_data_selector.0
    );
}

pub fn get_selectors() -> Selectors {
    SELECTORS.lock().expect("GDT not initialized")
}

pub fn kernel_code_selector() -> SegmentSelector {
    GDT.1.kernel_code_selector
}

pub fn kernel_data_selector() -> SegmentSelector {
    GDT.1.kernel_data_selector
}

pub fn user_code_selector() -> SegmentSelector {
    GDT.1.user_code_selector
}

pub fn user_data_selector() -> SegmentSelector {
    GDT.1.user_data_selector
}

/// Actualiza RSP0 en el TSS del núcleo actual para el proceso actual
/// Llamado en cada context switch a proceso de usuario.
pub fn set_kernel_stack(stack_top: u64) {
    // Después de smp::init el TSS activo es el per-CPU (cada núcleo el suyo).
    if crate::smp::smp_initialized() {
        let pc = crate::smp::current_cpu();
        unsafe {
            let tss = &mut *pc.tss.get();
            tss.privilege_stack_table[0] = VirtAddr::new(stack_top);
        }
    } else {
        // Arranque temprano: usar el TSS único de esta GDT lazy_static.
        TSS.set_rsp0(stack_top);
    }
}
