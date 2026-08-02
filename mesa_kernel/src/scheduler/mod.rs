//! Scheduler con multitarea real y soporte para procesos Ring 3

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::curr_arch;
use crate::memory::AddressSpace;

pub use curr_arch::context::Context;

/// ID de tarea
pub type TaskId = u64;

/// Estado de una tarea
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Sleeping(u64),
    Terminated,
}

/// Tamaño del stack de kernel para cada tarea (16 KB)
const KERNEL_STACK_SIZE: usize = 16 * 1024;

/// Quantum por defecto
const DEFAULT_QUANTUM: u64 = 3;

/// Estructura de una tarea
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub state: TaskState,
    pub context: Context,

    // Stack de kernel (owned)
    pub kernel_stack: Vec<u8>,
    pub kernel_stack_top: u64,

    // Espacio de direcciones (None = usa el del kernel)
    pub address_space: Option<AddressSpace>,

    // ¿Es un proceso de usuario (Ring 3)?
    pub is_user: bool,

    // Para procesos de usuario: entry point y stack
    pub user_entry: u64,
    pub user_stack: u64,

    // Scheduling
    pub priority: u8,
    pub quantum: u64,
    pub ticks_used: u64,
    pub total_ticks: u64,

    // File descriptors (0=stdin, 1=stdout, 2=stderr)
    pub fd_table: Mutex<BTreeMap<i32, crate::fs::FileHandle>>,

    // Linux compatibility: ¿Esta tarea usa ABI Linux?
    pub is_linux: bool,
    // Linux signal blocked mask
    pub linux_sigblock: [u64; 1],

    // Parent process ID (None = kernel task or init)
    pub parent_id: Option<TaskId>,
    // Exit code (set when task exits, valid while zombie)
    pub exit_code: i32,
    // Process group ID (default = own task ID)
    pub pgid: u64,
    // Session ID (default = own task ID if session leader, else parent's)
    pub sid: u64,
}

impl Task {
    fn default_fd_table() -> BTreeMap<i32, crate::fs::FileHandle> {
        let mut table = BTreeMap::new();
        table.insert(
            0,
            crate::fs::FileHandle {
                path: String::from("/dev/stdin"),
                pos: 0,
                node_type: crate::fs::NodeType::Device,
            },
        );
        table.insert(
            1,
            crate::fs::FileHandle {
                path: String::from("/dev/stdout"),
                pos: 0,
                node_type: crate::fs::NodeType::Device,
            },
        );
        table.insert(
            2,
            crate::fs::FileHandle {
                path: String::from("/dev/stderr"),
                pos: 0,
                node_type: crate::fs::NodeType::Device,
            },
        );
        table
    }

    /// Crea una nueva tarea de kernel
    pub fn new(id: TaskId, name: &str, entry_point: fn()) -> Self {
        let kernel_stack = vec![0u8; KERNEL_STACK_SIZE];
        let stack_bottom = kernel_stack.as_ptr() as u64;
        let stack_top = stack_bottom + KERNEL_STACK_SIZE as u64;
        let stack_top_aligned = stack_top & !0xF;

        let sp = unsafe { curr_arch::init_task_stack(stack_top_aligned, entry_point as u64) };

        let mut ctx = Context::with_current_cr3();
        ctx.set_sp(sp);
        ctx.set_entry(entry_point as u64);

        let id_val = id;
        Self {
            id,
            name: String::from(name),
            state: TaskState::Ready,
            context: ctx,
            kernel_stack,
            kernel_stack_top: stack_top_aligned,
            address_space: None,
            is_user: false,
            user_entry: 0,
            user_stack: 0,
            priority: 1,
            quantum: DEFAULT_QUANTUM,
            ticks_used: 0,
            total_ticks: 0,
            fd_table: Mutex::new(Self::default_fd_table()),
            is_linux: false,
            linux_sigblock: [0; 1],
            parent_id: None,
            exit_code: 0,
            pgid: id_val,
            sid: 0,
        }
    }

    /// Crea una nueva tarea de usuario (Ring 3). Acepta bytecode raw o binario ELF64.
    pub fn new_user(
        id: TaskId,
        name: &str,
        code: &[u8],
        parent_id: Option<TaskId>,
    ) -> Result<Self, &'static str> {
        // Crear espacio de direcciones propio
        let mut address_space = AddressSpace::new()?;

        // Detectar ELF64 o bytecode
        let (user_entry, user_stack) = if code.len() >= 4 && code[0..4] == [0x7f, b'E', b'L', b'F']
        {
            crate::elf::load_elf(&mut address_space, code)?
        } else {
            address_space.setup_user_process(code)?
        };

        // Stack de kernel para esta tarea
        let kernel_stack = vec![0u8; KERNEL_STACK_SIZE];
        let stack_bottom = kernel_stack.as_ptr() as u64;
        let stack_top = stack_bottom + KERNEL_STACK_SIZE as u64;
        let stack_top_aligned = stack_top & !0xF;

        let sp = unsafe { curr_arch::init_user_stack(stack_top_aligned, user_entry, user_stack) };

        let mut ctx = Context::new();
        ctx.set_sp(sp);
        ctx.set_entry(user_entry);
        #[cfg(target_arch = "x86_64")]
        {
            ctx.set_page_table(address_space.cr3());
        }
        #[cfg(target_arch = "aarch64")]
        {
            ctx.set_page_table(0); // TODO
        }

        crate::serial_println!(
            "[SCHED] User task '{}': entry={:#x}, stack={:#x}",
            name,
            user_entry,
            user_stack
        );

        let id_val = id;
        Ok(Self {
            id,
            name: String::from(name),
            state: TaskState::Ready,
            context: ctx,
            kernel_stack,
            kernel_stack_top: stack_top_aligned,
            address_space: Some(address_space),
            is_user: true,
            user_entry,
            user_stack,
            priority: 1,
            quantum: DEFAULT_QUANTUM,
            ticks_used: 0,
            total_ticks: 0,
            fd_table: Mutex::new(Self::default_fd_table()),
            is_linux: true,
            linux_sigblock: [0; 1],
            parent_id,
            exit_code: 0,
            pgid: id_val,
            sid: id_val,
        })
    }

    /// Crea la tarea inicial del kernel
    fn kernel_task() -> Self {
        Self {
            id: 0,
            name: String::from("kernel_main"),
            state: TaskState::Running,
            context: Context::with_current_cr3(),
            kernel_stack: Vec::new(),
            kernel_stack_top: 0,
            address_space: None,
            is_user: false,
            user_entry: 0,
            user_stack: 0,
            priority: 0,
            quantum: DEFAULT_QUANTUM,
            ticks_used: 0,
            total_ticks: 0,
            fd_table: Mutex::new(Self::default_fd_table()),
            is_linux: false,
            linux_sigblock: [0; 1],
            parent_id: None,
            exit_code: 0,
            pgid: 0,
            sid: 0,
        }
    }
}

/// Zombie tasks: (child_pid, exit_code, parent_pid)
pub static ZOMBIE_TASKS: Mutex<VecDeque<(TaskId, i32, TaskId)>> = Mutex::new(VecDeque::new());

/// Tareas terminadas pendientes de ser liberadas (para evitar use-after-free en switch_context)
pub static DEAD_TASKS: Mutex<Vec<Box<Task>>> = Mutex::new(Vec::new());

// Architecture-specific bootstrap functions moved to src/arch

// Architecture-specific bootstrap logic moved to src/arch

// ══════════════════════════════════════════════════════════════════════════════
// ESTADO GLOBAL
// ══════════════════════════════════════════════════════════════════════════════

static READY_QUEUE: Mutex<VecDeque<Box<Task>>> = Mutex::new(VecDeque::new());
static SLEEP_QUEUE: Mutex<VecDeque<Box<Task>>> = Mutex::new(VecDeque::new());
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);
static SCHEDULER_ACTIVE: AtomicBool = AtomicBool::new(false);

// En SMP el estado per-CPU (tarea actual y flag de reentrada) vive en el área
// per-CPU de cada núcleo (smp::PerCpuData). En single-core se usan globals.

/// Mutex que guarda la tarea actual del núcleo actual.
#[cfg(target_arch = "x86_64")]
fn current_slot() -> &'static Mutex<Option<Box<Task>>> {
    &crate::smp::cpu_data(crate::smp::current_cpu_id()).current
}
#[cfg(not(target_arch = "x86_64"))]
fn current_slot() -> &'static Mutex<Option<Box<Task>>> {
    &CURRENT_TASK
}
#[cfg(not(target_arch = "x86_64"))]
static CURRENT_TASK: Mutex<Option<Box<Task>>> = Mutex::new(None);

/// Flag de reentrada de schedule() del núcleo actual.
#[cfg(target_arch = "x86_64")]
fn in_schedule_flag() -> &'static AtomicBool {
    &crate::smp::cpu_data(crate::smp::current_cpu_id()).in_schedule
}
#[cfg(not(target_arch = "x86_64"))]
fn in_schedule_flag() -> &'static AtomicBool {
    &IN_SCHEDULE
}
#[cfg(not(target_arch = "x86_64"))]
pub(crate) static IN_SCHEDULE: AtomicBool = AtomicBool::new(false);

/// ID del núcleo actual.
#[cfg(target_arch = "x86_64")]
fn cpu_id() -> usize {
    crate::smp::current_cpu_id()
}
#[cfg(not(target_arch = "x86_64"))]
fn cpu_id() -> usize {
    0
}

/// ¿Es una tarea idle (una por núcleo)?
#[cfg(target_arch = "x86_64")]
fn is_idle_task(id: u64) -> bool {
    id >= crate::smp::IDLE_TASK_BASE
}
#[cfg(not(target_arch = "x86_64"))]
fn is_idle_task(_id: u64) -> bool {
    false
}

/// Hueco donde cada núcleo guarda su tarea idle.
#[cfg(target_arch = "x86_64")]
fn idle_slot(cpu: usize) -> &'static Mutex<Option<Box<Task>>> {
    &crate::smp::cpu_data(cpu).idle
}
#[cfg(not(target_arch = "x86_64"))]
fn idle_slot(_cpu: usize) -> &'static Mutex<Option<Box<Task>>> {
    static DUMMY_IDLE: Mutex<Option<Box<Task>>> = Mutex::new(None);
    &DUMMY_IDLE
}

/// Limpia el flag de reentrada del núcleo actual (usado por los bootstraps
/// de tareas al volver de un context switch).
pub fn clear_in_schedule() {
    in_schedule_flag().store(false, Ordering::SeqCst);
    DEAD_TASKS.lock().clear();
}

// ══════════════════════════════════════════════════════════════════════════════
// API PÚBLICA
// ══════════════════════════════════════════════════════════════════════════════

/// Inicializa el scheduler
pub fn init() {
    crate::serial_println!("[SCHED] Inicializando scheduler...");

    let kernel = Task::kernel_task();
    *current_slot().lock() = Some(Box::new(kernel));

    SCHEDULER_ACTIVE.store(true, Ordering::SeqCst);

    crate::klog_info!("Scheduler initialized");
    crate::serial_println!("[SCHED] Scheduler listo");
}

/// Crea una nueva tarea de kernel
pub fn spawn(name: &str, entry_point: fn()) -> TaskId {
    let id = NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst);
    let task = Task::new(id, name, entry_point);

    crate::serial_println!("[SCHED] Spawn kernel task '{}' id={}", name, id);
    crate::klog_info!("Task spawned: {} (id={})", name, id);

    READY_QUEUE.lock().push_back(Box::new(task));
    id
}

/// Crea una nueva tarea de usuario (Ring 3)
pub fn spawn_user(name: &str, code: &[u8]) -> Result<TaskId, &'static str> {
    let id = NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst);
    let parent_id = current_slot().lock().as_ref().map(|t| t.id);
    let task = Task::new_user(id, name, code, parent_id)?;

    crate::serial_println!("[SCHED] Spawn user task '{}' id={}", name, id);
    crate::klog_info!("User task spawned: {} (id={})", name, id);

    READY_QUEUE.lock().push_back(Box::new(task));
    Ok(id)
}

/// Alias para compatibilidad
pub fn create_task(name: &str, entry_point: fn()) -> TaskId {
    spawn(name, entry_point)
}

/// Llamado desde timer interrupt (en cada núcleo)
pub fn timer_tick() {
    if !SCHEDULER_ACTIVE.load(Ordering::Relaxed) {
        return;
    }

    if in_schedule_flag().load(Ordering::Relaxed) {
        return;
    }

    // 0. Medición de carga por-CPU: un tick total y, si el núcleo está ocioso,
    //    un tick ocioso. Un núcleo está ocioso cuando su tarea actual es la
    //    tarea idle o la shell (kernel_main, id 0) sin nada en la cola.
    #[cfg(target_arch = "x86_64")]
    {
        let pc = crate::smp::cpu_data(crate::smp::current_cpu_id());
        pc.total_ticks.fetch_add(1, Ordering::Relaxed);

        let q_empty = READY_QUEUE.try_lock().map_or(false, |q| q.is_empty());
        let cur_idle = match current_slot().try_lock() {
            Some(guard) => guard
                .as_ref()
                .map_or(false, |t| is_idle_task(t.id) || (t.id == 0 && q_empty)),
            None => false,
        };
        if cur_idle {
            pc.idle_ticks.fetch_add(1, Ordering::Relaxed);
        }
    }

    // 1. Manejar tareas durmiendo. Solo el BSP decrementa los contadores para
    //    mantener la frecuencia de reloj estable en SMP.
    let bsp_should_manage_sleep = cfg!(not(target_arch = "x86_64")) || cpu_id() == 0;
    if bsp_should_manage_sleep {
        let mut sleep_q = SLEEP_QUEUE.lock();
        let mut ready_q = READY_QUEUE.lock();
        let mut i = 0;
        while i < sleep_q.len() {
            let mut wakeup = false;
            if let TaskState::Sleeping(ref mut ticks) = sleep_q[i].state {
                if *ticks > 0 {
                    *ticks -= 1;
                }
                if *ticks == 0 {
                    wakeup = true;
                }
            }

            if wakeup {
                let mut task = sleep_q.remove(i).unwrap();
                task.state = TaskState::Ready;
                ready_q.push_back(task);
            } else {
                i += 1;
            }
        }
    }

    let should_switch = {
        let mut current = match current_slot().try_lock() {
            Some(guard) => guard,
            None => return,
        };

        if let Some(ref mut task) = *current {
            task.total_ticks += 1;
            task.ticks_used += 1;

            if task.ticks_used >= task.quantum {
                READY_QUEUE
                    .try_lock()
                    .map(|q| !q.is_empty())
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        }
    };

    if should_switch {
        schedule();
    }
}

/// Timer tick simplificado
pub fn tick() {
    if !SCHEDULER_ACTIVE.load(Ordering::Relaxed) {
        return;
    }

    if let Some(mut guard) = current_slot().try_lock() {
        if let Some(ref mut current) = *guard {
            current.total_ticks += 1;
            current.ticks_used += 1;
        }
    }
}

/// Cede el CPU voluntariamente
pub fn yield_now() {
    if SCHEDULER_ACTIVE.load(Ordering::Relaxed) {
        schedule();
    }
}

/// Duerme la tarea actual por un número de ticks
pub fn sleep(ticks: u64) {
    if ticks == 0 {
        yield_now();
        return;
    }

    {
        let mut current = current_slot().lock();
        if let Some(ref mut task) = *current {
            task.state = TaskState::Sleeping(ticks);
        }
    }

    schedule();
}

/// Duerme la tarea actual por milisegundos (aprox)
pub fn sleep_ms(ms: u64) {
    // Asumiendo timer de 100Hz (1 tick = 10ms)
    // TODO: Usar constante de frecuencia real si está disponible
    let ticks = ms / 10;
    sleep(ticks.max(1));
}

/// Termina la tarea actual. Esta función **nunca retorna**.
pub fn exit_current() -> ! {
    crate::serial_println!("[SCHED] Task exiting...");

    // Deshabilitar interrupciones para todo el proceso.
    // La tarea destino las re-habilitará cuando reanude desde schedule().
    curr_arch::disable_interrupts();

    let cpu = cpu_id();

    // ── 1. Extraer la tarea moribunda del slot actual ──────────────────────
    let mut dying = {
        let mut guard = current_slot().lock();
        match guard.take() {
            Some(t) => t,
            None => {
                drop(guard);
                loop { curr_arch::halt(); }
            }
        }
    };

    if dying.id == 0 {
        *current_slot().lock() = Some(dying);
        crate::serial_println!("[SCHED] Cannot exit kernel_main task");
        curr_arch::enable_interrupts();
        loop { curr_arch::halt(); }
    }

    // ── 2. Registrar zombie ────────────────────────────────────────────────
    dying.state = TaskState::Terminated;
    let exit_code = dying.exit_code;
    crate::klog_info!(
        "Task {} ({}) terminated with code {}",
        dying.id,
        dying.name,
        exit_code
    );
    if let Some(ppid) = dying.parent_id {
        ZOMBIE_TASKS.lock().push_back((dying.id, exit_code, ppid));
    }

    // ── 3. Guardar la dirección del contexto scratch ANTES de mover a DEAD_TASKS
    //   El contexto de dying se usará como destino "basura" del switch_context:
    //   el CPU guardará ahí los registros de salida pero nunca volveremos a leerlos.
    //   IMPORTANTE: mover dying a DEAD_TASKS ANTES del switch, pero NO llamar a
    //   clear_in_schedule() hasta que estemos en el nuevo contexto. Las interrupciones
    //   permanecen deshabilitadas para que ningún timer interrupt limpie DEAD_TASKS
    //   antes de que switch_context haya terminado de escribir en old_ctx_scratch.
    let old_ctx_scratch: *mut Context = &mut dying.context as *mut Context;
    DEAD_TASKS.lock().push(dying);

    // ── 4. Elegir la siguiente tarea (READY_QUEUE → idle) ─────────────────
    let next = READY_QUEUE.lock().pop_front();

    if let Some(mut next_task) = next {
        next_task.state = TaskState::Running;
        next_task.ticks_used = 0;

        #[cfg(target_arch = "x86_64")]
        if next_task.is_user {
            crate::arch::x86_64::gdt::set_kernel_stack(next_task.kernel_stack_top);
            crate::smp::set_syscall_kstack(next_task.kernel_stack_top);
        }
        #[cfg(target_arch = "x86_64")]
        if let Some(ref addr) = next_task.address_space {
            unsafe { addr.activate(); }
        } else {
            // Tarea de kernel: restaurar CR3 del kernel
            unsafe {
                use core::arch::asm;
                let k_cr3 = crate::memory::vmm::kernel_cr3();
                if k_cr3 != 0 {
                    asm!("mov cr3, {}", in(reg) k_cr3, options(nostack));
                }
            }
        }

        let new_ctx: *const Context = &next_task.context as *const Context;
        *current_slot().lock() = Some(next_task);

        // Limpiar el flag de schedule ANTES del switch para que la nueva tarea
        // pueda llamar a schedule() / timer_tick() sin bloquearse.
        in_schedule_flag().store(false, Ordering::SeqCst);

        // Interrupciones SIGUEN deshabilitadas: la tarea destino las re-habilitará
        // en su propia ruta de regreso de schedule().
        unsafe {
            curr_arch::context::switch_context(old_ctx_scratch, new_ctx);
        }
        // Nunca llegamos aquí.
        loop { curr_arch::halt(); }
    }

    // ── 5. Sin tarea lista: ir al idle de este núcleo ─────────────────────
    {
        let mut idle_guard = idle_slot(cpu).lock();
        if let Some(mut idle_task) = idle_guard.take() {
            idle_task.state = TaskState::Running;

            #[cfg(target_arch = "x86_64")]
            {
                unsafe {
                    use core::arch::asm;
                    let k_cr3 = crate::memory::vmm::kernel_cr3();
                    if k_cr3 != 0 {
                        asm!("mov cr3, {}", in(reg) k_cr3, options(nostack));
                    }
                }
                crate::arch::x86_64::gdt::set_kernel_stack(idle_task.kernel_stack_top);
                crate::smp::set_syscall_kstack(idle_task.kernel_stack_top);
            }

            let new_ctx: *const Context = &idle_task.context as *const Context;
            *current_slot().lock() = Some(idle_task);
            in_schedule_flag().store(false, Ordering::SeqCst);

            unsafe {
                curr_arch::context::switch_context(old_ctx_scratch, new_ctx);
            }
            loop { curr_arch::halt(); }
        }
    }

    // ── 6. Último recurso ─────────────────────────────────────────────────
    curr_arch::enable_interrupts();
    loop { curr_arch::halt(); }
}


/// Check if a child of the given parent has exited.
/// Returns (child_pid, exit_code) if found, None otherwise.
pub fn collect_zombie(child_pid: Option<TaskId>, parent_pid: TaskId) -> Option<(TaskId, i32)> {
    let mut zombies = ZOMBIE_TASKS.lock();
    let idx = if let Some(pid) = child_pid {
        zombies
            .iter()
            .position(|(c, _, p)| *c == pid && *p == parent_pid)
    } else {
        zombies.iter().position(|(_, _, p)| *p == parent_pid)
    };
    idx.map(|i| {
        let (cid, code, _) = zombies.remove(i).unwrap();
        (cid, code)
    })
}

/// Mata una tarea por ID
pub fn kill(id: TaskId) -> Result<(), &'static str> {
    if id == 0 {
        return Err("Cannot kill kernel task");
    }

    let mut queue = READY_QUEUE.lock();

    if let Some(pos) = queue.iter().position(|t| t.id == id) {
        let task = queue.remove(pos).unwrap();
        crate::klog_info!("Task {} ({}) killed", task.id, task.name);
        crate::serial_println!("[SCHED] Killed task {}", id);
        return Ok(());
    }

    Err("Task not found")
}

/// Realiza context switch. En SMP cada núcleo independientemente saca tareas
/// de la cola global READY_QUEUE y las ejecuta; su tarea actual vive en su
/// área per-CPU. Cuando no hay nada que ejecutar, el núcleo sigue con su
/// tarea actual (kernel_main en el BSP, idle en los APs), que hará halt.
pub fn schedule() {
    if in_schedule_flag().swap(true, Ordering::SeqCst) {
        return;
    }

    let was_enabled = curr_arch::are_interrupts_enabled();
    curr_arch::disable_interrupts();

    let cpu = cpu_id();

    let next = READY_QUEUE.lock().pop_front();

    if let Some(mut next_task) = next {
        let mut current_guard = current_slot().lock();

        if let Some(mut current_task) = current_guard.take() {
            // old_ctx debe calcularse ANTES de mover la tarea a una cola.
            let old_ctx: *mut Context = &mut current_task.context as *mut Context;

            if is_idle_task(current_task.id) {
                // Tarea idle: devolverla a su hueco (nunca se encola).
                *idle_slot(cpu).lock() = Some(current_task);
            } else {
                if current_task.state == TaskState::Running {
                    current_task.state = TaskState::Ready;
                    current_task.ticks_used = 0;
                }

                if current_task.state == TaskState::Ready {
                    READY_QUEUE.lock().push_back(current_task);
                } else if let TaskState::Sleeping(_) = current_task.state {
                    SLEEP_QUEUE.lock().push_back(current_task);
                } else if current_task.state == TaskState::Blocked {
                    READY_QUEUE.lock().push_back(current_task);
                } else {
                    DEAD_TASKS.lock().push(current_task);
                }
                // Terminated u otros: se descarta (el zombie ya se registró).
            }

            next_task.state = TaskState::Running;
            next_task.ticks_used = 0;

            // Actualizar TSS RSP0 y kernel stack para syscalls (x86_64)
            #[cfg(target_arch = "x86_64")]
            if next_task.is_user {
                crate::arch::x86_64::gdt::set_kernel_stack(next_task.kernel_stack_top);
                crate::smp::set_syscall_kstack(next_task.kernel_stack_top);
            }

            let new_ctx: *const Context = &next_task.context as *const Context;

            *current_guard = Some(next_task);
            drop(current_guard);

            unsafe {
                curr_arch::context::switch_context(old_ctx, new_ctx);
            }

            // === Este código solo se ejecuta al VOLVER a esta tarea ===
            clear_in_schedule();
            if was_enabled {
                curr_arch::enable_interrupts();
            }
            return;
        } else {
            // No hay tarea actual (caso extremo: núcleo sin idle asignado).
            // Cambiar desde el contexto idle si existe.
            let mut idle_guard = idle_slot(cpu).lock();
            if let Some(ref mut idle_task) = *idle_guard {
                let old_ctx: *mut Context = &mut idle_task.context as *mut Context;
                next_task.state = TaskState::Running;
                next_task.ticks_used = 0;
                #[cfg(target_arch = "x86_64")]
                if next_task.is_user {
                    crate::arch::x86_64::gdt::set_kernel_stack(next_task.kernel_stack_top);
                    crate::smp::set_syscall_kstack(next_task.kernel_stack_top);
                }
                let new_ctx: *const Context = &next_task.context as *const Context;
                *current_guard = Some(next_task);
                drop(current_guard);
                drop(idle_guard);
                unsafe {
                    curr_arch::context::switch_context(old_ctx, new_ctx);
                }
                clear_in_schedule();
                if was_enabled {
                    curr_arch::enable_interrupts();
                }
                return;
            } else {
                // Sin contexto idle: devolver la tarea a la cola y salir.
                READY_QUEUE.lock().push_front(next_task);
                *current_guard = None;
                drop(current_guard);
                clear_in_schedule();
                if was_enabled {
                    curr_arch::enable_interrupts();
                }
                return;
            }
        }
    }

    // No hay tarea lista: mantener la tarea actual ejecutándose.
    clear_in_schedule();

    if was_enabled {
        curr_arch::enable_interrupts();
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// INFORMACIÓN
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct SchedulerInfo {
    pub current_task_id: TaskId,
    pub current_task_name: String,
    pub ready_tasks: usize,
    pub total_tasks: usize,
    pub scheduler_ready: bool,
}

pub fn get_info() -> SchedulerInfo {
    let current = current_slot().lock();
    let (id, name) = current
        .as_ref()
        .map(|t| (t.id, t.name.clone()))
        .unwrap_or((0, String::from("none")));

    let ready = READY_QUEUE.lock().len();

    SchedulerInfo {
        current_task_id: id,
        current_task_name: name,
        ready_tasks: ready,
        total_tasks: ready + 1,
        scheduler_ready: SCHEDULER_ACTIVE.load(Ordering::SeqCst),
    }
}

pub fn list_tasks() -> Vec<(TaskId, String, TaskState, u64)> {
    let mut tasks = Vec::new();

    if let Some(ref task) = *current_slot().lock() {
        tasks.push((task.id, task.name.clone(), task.state, task.total_ticks));
    }

    for task in READY_QUEUE.lock().iter() {
        tasks.push((task.id, task.name.clone(), task.state, task.total_ticks));
    }

    tasks
}

pub fn current_task_id() -> Option<TaskId> {
    current_slot().lock().as_ref().map(|t| t.id)
}

pub fn current_task_name() -> Option<String> {
    current_slot().lock().as_ref().map(|t| t.name.clone())
}

/// Ejecuta una función sobre la tarea actual
pub fn with_current_task<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Task) -> R,
{
    let mut guard = current_slot().lock();
    guard.as_mut().map(|t| f(t))
}

pub fn current_kernel_stack_top() -> Option<u64> {
    with_current_task(|t| t.kernel_stack_top)
}

pub fn task_count() -> usize {
    READY_QUEUE.lock().len() + 1
}

pub fn ready_count() -> usize {
    READY_QUEUE.lock().len()
}

/// Allocate a new task ID
pub fn new_task_id() -> TaskId {
    NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst)
}

/// Add a pre-built task to the ready queue
pub fn add_ready_task(task: Box<Task>) {
    READY_QUEUE.lock().push_back(task);
}
