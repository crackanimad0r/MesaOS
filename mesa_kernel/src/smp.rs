// mesa_kernel/src/smp.rs
//! Soporte SMP: arranque de CPUs secundarias (APs) y estado por-CPU.
//!
//! Cada núcleo tiene un área [`PerCpuData`] dentro de `CPU_DATA`, apuntada por
//! la base del registro GS (IA32_GS_BASE). Así el ensamblador de syscall puede
//! acceder rápido al stack de kernel de la CPU actual vía `gs:[offset]`.

#![cfg(target_arch = "x86_64")]

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use spin::Mutex;
use x86_64::registers::model_specific::GsBase;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

use crate::arch::x86_64::limine_req;
use crate::scheduler::Task;

/// Número máximo de CPUs soportado.
pub const MAX_CPUS: usize = 32;

/// Base de IDs reservados para las tareas idle (una por CPU).
pub const IDLE_TASK_BASE: u64 = 0xFFFF_FFFF_FFFF_FF00;

/// Tamaño de los stacks estáticos por-CPU (para IST / RSP0 inicial).
const STACK_SIZE: usize = 4096 * 5;

// Stacks estáticos por-CPU. Se accede solo desde setup_cpu() (una vez por núcleo).
static mut DOUBLE_FAULT_STACKS: [[u8; STACK_SIZE]; MAX_CPUS] = [[0; STACK_SIZE]; MAX_CPUS];
static mut INIT_KERNEL_STACKS: [[u8; STACK_SIZE]; MAX_CPUS] = [[0; STACK_SIZE]; MAX_CPUS];

/// Área de datos por-CPU. La base GS apunta a una instancia por núcleo.
#[repr(C)]
pub struct PerCpuData {
    pub syscall_kstack: AtomicU64,
    pub syscall_usr_rsp: AtomicU64,
    pub cpu_id: AtomicUsize,
    pub lapic_id: AtomicU32,
    pub in_schedule: AtomicBool,
    pub current: Mutex<Option<alloc::boxed::Box<Task>>>,
    pub idle: Mutex<Option<alloc::boxed::Box<Task>>>,
    pub tss: UnsafeCell<TaskStateSegment>,

    // Medición de carga: ticks de timer totales y ticks ociosos de este núcleo.
    // Los incrementa el scheduler en cada timer_tick().
    pub total_ticks: AtomicU64,
    pub idle_ticks: AtomicU64,
}

// SAFETY: cada CPU accede a su propia entrada; las estructuras internas (Mutex,
// atomics, UnsafeCell) proveen la sincronización necesaria.
unsafe impl Sync for PerCpuData {}

impl PerCpuData {
    const fn empty() -> Self {
        Self {
            syscall_kstack: AtomicU64::new(0),
            syscall_usr_rsp: AtomicU64::new(0),
            cpu_id: AtomicUsize::new(0),
            lapic_id: AtomicU32::new(0),
            in_schedule: AtomicBool::new(false),
            current: Mutex::new(None),
            idle: Mutex::new(None),
            tss: UnsafeCell::new(TaskStateSegment::new()),
            total_ticks: AtomicU64::new(0),
            idle_ticks: AtomicU64::new(0),
        }
    }
}

static CPU_DATA: [PerCpuData; MAX_CPUS] = [const { PerCpuData::empty() }; MAX_CPUS];

static SMP_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Número de núcleos que han arrancado (BSP + APs).
static ACTIVE_CPUS: AtomicUsize = AtomicUsize::new(1);

/// `true` una vez que `smp::init` ha preparado al menos el área del BSP.
pub fn smp_initialized() -> bool {
    SMP_INITIALIZED.load(Ordering::Relaxed)
}

/// Offsets para el ensamblador de syscall (campos al principio del struct).
pub const SYSCALL_KSTACK_OFFSET: usize = core::mem::offset_of!(PerCpuData, syscall_kstack);
pub const SYSCALL_USR_RSP_OFFSET: usize = core::mem::offset_of!(PerCpuData, syscall_usr_rsp);

/// Devuelve el área per-CPU del núcleo dado.
pub fn cpu_data(id: usize) -> &'static PerCpuData {
    &CPU_DATA[id.min(MAX_CPUS - 1)]
}

/// ID del núcleo actual (0 = BSP).
pub fn current_cpu_id() -> usize {
    let gs = GsBase::read().as_u64();
    if gs == 0 {
        // GS aún no configurado (inicio temprano): somos el BSP.
        return 0;
    }
    let base = &CPU_DATA as *const _ as u64;
    let idx = ((gs - base) as usize) / core::mem::size_of::<PerCpuData>();
    idx.min(MAX_CPUS - 1)
}

/// Área per-CPU del núcleo actual.
pub fn current_cpu() -> &'static PerCpuData {
    cpu_data(current_cpu_id())
}

/// Número de núcleos activos (BSP + APs arrancados).
pub fn active_cpu_count() -> usize {
    ACTIVE_CPUS.load(Ordering::Relaxed)
}

/// Carga (%) del núcleo `id` según los ticks de timer registrados por el
/// scheduler: 100 * (total - ocio) / total. Devuelve 0.0 si aún no hay datos.
pub fn cpu_load(id: usize) -> f32 {
    let pc = cpu_data(id);
    let total = pc.total_ticks.load(Ordering::Relaxed);
    if total == 0 {
        return 0.0;
    }
    let idle = pc.idle_ticks.load(Ordering::Relaxed).min(total);
    ((total - idle) as f32 / total as f32) * 100.0
}

/// Establece el top del stack de kernel usado por SYSCALL en la CPU actual.
pub fn set_syscall_kstack(top: u64) {
    current_cpu().syscall_kstack.store(top, Ordering::SeqCst);
}

/// Lee el stack de kernel de syscall de la CPU actual.
pub fn syscall_kstack() -> u64 {
    current_cpu().syscall_kstack.load(Ordering::Relaxed)
}

/// Guarda el RSP de usuario durante un syscall en la CPU actual.
pub fn set_usr_rsp(value: u64) {
    current_cpu().syscall_usr_rsp.store(value, Ordering::SeqCst);
}

/// Lee el RSP de usuario guardado de la CPU actual.
pub fn usr_rsp() -> u64 {
    current_cpu().syscall_usr_rsp.load(Ordering::Relaxed)
}

/// Prepara el área per-CPU `idx`, construye y carga la GDT/TSS de ese núcleo
/// y apunta la base GS a su área.
fn setup_cpu(idx: usize, lapic_id: u32) {
    let pc = cpu_data(idx);
    pc.cpu_id.store(idx, Ordering::SeqCst);
    pc.lapic_id.store(lapic_id, Ordering::SeqCst);

    // Configurar TSS: IST0 (double fault) y RSP0 inicial.
    unsafe {
        let tss = &mut *pc.tss.get();
        let df_top = core::ptr::addr_of!(DOUBLE_FAULT_STACKS[idx]) as u64 + STACK_SIZE as u64;
        tss.interrupt_stack_table[crate::arch::x86_64::gdt::DOUBLE_FAULT_IST_INDEX as usize] =
            VirtAddr::new(df_top);
        let init_top = core::ptr::addr_of!(INIT_KERNEL_STACKS[idx]) as u64 + STACK_SIZE as u64;
        tss.privilege_stack_table[0] = VirtAddr::new(init_top);
    }

    // Construir GDT + TSS per-CPU (la GDT codifica la base del TSS, así que cada
    // núcleo necesita la suya). Se asigna en el heap y se deja viva para siempre.
    let tss_ref: &'static TaskStateSegment = unsafe { &*pc.tss.get() };
    let (gdt, selectors) = crate::arch::x86_64::gdt::build_gdt(tss_ref);
    let gdt = alloc::boxed::Box::leak(alloc::boxed::Box::new(gdt));
    crate::arch::x86_64::gdt::store_selectors(selectors);

    unsafe {
        use x86_64::instructions::segmentation::{Segment, CS, DS, ES, SS};
        use x86_64::instructions::tables::load_tss;

        gdt.load();
        CS::set_reg(selectors.kernel_code_selector);
        DS::set_reg(selectors.kernel_data_selector);
        ES::set_reg(selectors.kernel_data_selector);
        SS::set_reg(selectors.kernel_data_selector);
        load_tss(selectors.tss_selector);
    }

    // Apuntar GS base al área per-CPU (para syscalls y current_cpu_id).
    // IMPORTANTE: esto debe ejecutarse SOLO en el núcleo cuyo índice es `idx`
    // (BSP en smp::init, cada AP en su ap_entry). Llamarlo desde otro núcleo
    // corrompería la base GS de ese núcleo.
    unsafe {
        GsBase::write(VirtAddr::new(&CPU_DATA[idx] as *const _ as u64));
    }
}

/// Inicializa el área del BSP y arranca todos los APs vía Limine MP.
/// Debe llamarse DESPUÉS de: memory::init (heap), acpi::init, init_apic,
/// scheduler::init y syscall::init.
pub fn init() {
    let response = match limine_req::smp_response() {
        Some(r) => r,
        None => {
            crate::serial_println!("[SMP] No se pudo obtener la respuesta MP de Limine");
            return;
        }
    };

    let bsp_lapic = response.bsp_lapic_id();
    let cpus = response.cpus();

    setup_cpu(0, bsp_lapic);
    crate::serial_println!("[SMP] BSP: LAPIC ID {}", bsp_lapic);
    SMP_INITIALIZED.store(true, Ordering::SeqCst);

    let mut next_idx = 1usize;
    for cpu_ref in cpus {
        let cpu: &limine::mp::Cpu = cpu_ref;
        if cpu.lapic_id == bsp_lapic {
            continue;
        }
        if next_idx >= MAX_CPUS {
            crate::serial_println!(
                "[SMP] Máximo de {} CPUs alcanzado, ignorando el resto",
                MAX_CPUS
            );
            break;
        }
        let idx = next_idx;
        next_idx += 1;

        // IMPORTANTE: NO llamar setup_cpu() aquí. Ese código carga la GDT/TSS
        // y escribe la base GS del NÚCLEO ACTUAL (el BSP), por lo que hacerlo
        // por cada AP corrompería la base GS del BSP. Cada AP ejecuta su propio
        // setup_cpu(idx, ...) al entrar en ap_entry.
        cpu.extra.store(idx as u64, Ordering::SeqCst);
        crate::serial_println!("[SMP] Arrancando AP {} (LAPIC ID {})...", idx, cpu.lapic_id);
        cpu.goto_address.write(ap_entry);
    }

    crate::serial_println!("[SMP] {} núcleo(s) activo(s)", next_idx);
    ACTIVE_CPUS.store(next_idx, Ordering::SeqCst);
}

/// Bucle de la tarea idle de cada núcleo.
fn ap_idle_loop() {
    loop {
        crate::scheduler::yield_now();
        x86_64::instructions::hlt();
    }
}

/// Entry point de los APs (saltado por Limine con un stack propio de 64 KiB).
extern "C" fn ap_entry(cpu: &limine::mp::Cpu) -> ! {
    let idx = cpu.extra.load(Ordering::SeqCst) as usize;
    crate::serial_println!("[SMP] AP {} entrando en ap_entry", idx);

    // Configurar GDT/TSS, selectores y GS base para ESTE núcleo.
    setup_cpu(idx, cpu.lapic_id);

    // MSRs de syscall son por-CPU: EFER.SCE, STAR, LSTAR, SFMASK.
    crate::syscall::init_cpu();

    // Cargar la IDT (compartida, pero cada CPU tiene su IDTR).
    crate::arch::x86_64::interrupts::init_idt();

    // Habilitar y limpiar el Local APIC de ESTE núcleo (SVR enable + TPR 0).
    unsafe {
        crate::arch::x86_64::interrupts::apic::init_local_apic_cpu();
    }

    // Programar el timer LAPIC de este núcleo.
    unsafe {
        crate::arch::x86_64::interrupts::apic::init_lapic_timer(
            crate::arch::x86_64::interrupts::apic::lapic_timer_hz(),
        );
    }

    // Crear la tarea idle para este núcleo y hacerla la tarea actual.
    let idle_id = IDLE_TASK_BASE + idx as u64;
    let idle_task = alloc::boxed::Box::new(Task::new(idle_id, "idle", ap_idle_loop));
    *cpu_data(idx).idle.lock() = Some(idle_task);
    *cpu_data(idx).current.lock() = cpu_data(idx).idle.lock().take();

    // Habilitar interrupciones en este núcleo.
    unsafe {
        x86_64::instructions::interrupts::enable();
    }

    crate::serial_println!("[SMP] AP {} listo (LAPIC ID {})", idx, cpu.lapic_id);

    // Bucle principal del AP: ceder el CPU y hacer halt cuando no haya trabajo.
    loop {
        crate::scheduler::yield_now();
        x86_64::instructions::hlt();
    }
}
