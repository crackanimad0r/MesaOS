//! Scheduler con multitarea real y soporte para procesos Ring 3


use alloc::collections::{VecDeque, BTreeMap};
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use spin::Mutex;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

use crate::memory::AddressSpace;
use crate::curr_arch;

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
    kernel_stack: Vec<u8>,
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
}

impl Task {
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
        }
    }
    
    fn default_fd_table() -> BTreeMap<i32, crate::fs::FileHandle> {
        let mut table = BTreeMap::new();
        // Stdin (placeholder)
        table.insert(0, crate::fs::FileHandle { 
            path: String::from("/dev/stdin"), 
            pos: 0, 
            node_type: crate::fs::NodeType::Device 
        });
        // Stdout
        table.insert(1, crate::fs::FileHandle { 
            path: String::from("/dev/stdout"), 
            pos: 0, 
            node_type: crate::fs::NodeType::Device 
        });
        // Stderr
        table.insert(2, crate::fs::FileHandle { 
            path: String::from("/dev/stderr"), 
            pos: 0, 
            node_type: crate::fs::NodeType::Device 
        });
        table
    }
    
    /// Crea una nueva tarea de usuario (Ring 3). Acepta bytecode raw o binario ELF64.
    pub fn new_user(id: TaskId, name: &str, code: &[u8]) -> Result<Self, &'static str> {
        // Crear espacio de direcciones propio
        let mut address_space = AddressSpace::new()?;
        
        // Detectar ELF64 o bytecode
        let (user_entry, user_stack) = if code.len() >= 4 && code[0..4] == [0x7f, b'E', b'L', b'F'] {
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
            name, user_entry, user_stack
        );
        
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
        }
    }
}

// Architecture-specific bootstrap functions moved to src/arch

// Architecture-specific bootstrap logic moved to src/arch

// ══════════════════════════════════════════════════════════════════════════════
// ESTADO GLOBAL
// ══════════════════════════════════════════════════════════════════════════════

static READY_QUEUE: Mutex<VecDeque<Box<Task>>> = Mutex::new(VecDeque::new());
static SLEEP_QUEUE: Mutex<VecDeque<Box<Task>>> = Mutex::new(VecDeque::new());
static CURRENT_TASK: Mutex<Option<Box<Task>>> = Mutex::new(None);
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);
static SCHEDULER_ACTIVE: AtomicBool = AtomicBool::new(false);
static IN_SCHEDULE: AtomicBool = AtomicBool::new(false);

// ══════════════════════════════════════════════════════════════════════════════
// API PÚBLICA
// ══════════════════════════════════════════════════════════════════════════════

/// Inicializa el scheduler
pub fn init() {
    crate::serial_println!("[SCHED] Inicializando scheduler...");
    
    let kernel = Task::kernel_task();
    *CURRENT_TASK.lock() = Some(Box::new(kernel));
    
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
    let task = Task::new_user(id, name, code)?;
    
    crate::serial_println!("[SCHED] Spawn user task '{}' id={}", name, id);
    crate::klog_info!("User task spawned: {} (id={})", name, id);
    
    READY_QUEUE.lock().push_back(Box::new(task));
    Ok(id)
}

/// Alias para compatibilidad
pub fn create_task(name: &str, entry_point: fn()) -> TaskId {
    spawn(name, entry_point)
}

/// Llamado desde timer interrupt
pub fn timer_tick() {
    if !SCHEDULER_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    
    if IN_SCHEDULE.load(Ordering::Relaxed) {
        return;
    }

    // 1. Manejar tareas durmiendo
    {
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
        let mut current = match CURRENT_TASK.try_lock() {
            Some(guard) => guard,
            None => return,
        };
        
        if let Some(ref mut task) = *current {
            task.total_ticks += 1;
            task.ticks_used += 1;
            
            if task.ticks_used >= task.quantum {
                READY_QUEUE.try_lock()
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
    
    if let Some(mut guard) = CURRENT_TASK.try_lock() {
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
        let mut current = CURRENT_TASK.lock();
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

/// Termina la tarea actual
pub fn exit_current() {
    crate::serial_println!("[SCHED] Task exiting...");
    
    {
        if let Some(ref mut task) = *CURRENT_TASK.lock() {
            if task.id == 0 {
                crate::serial_println!("[SCHED] Cannot exit kernel_main task");
                return;
            }
            task.state = TaskState::Terminated;
            crate::klog_info!("Task {} ({}) terminated", task.id, task.name);
        }
    }
    
    schedule();
    
    loop {
        crate::curr_arch::halt();
    }
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

/// Realiza context switch
pub fn schedule() {
    if IN_SCHEDULE.swap(true, Ordering::SeqCst) {
        return;
    }
    
    let was_enabled = curr_arch::are_interrupts_enabled();
    curr_arch::disable_interrupts();
    
    let next = READY_QUEUE.lock().pop_front();
    
    if let Some(mut next_task) = next {
        let mut current_guard = CURRENT_TASK.lock();
        
        if let Some(mut current_task) = current_guard.take() {
            let old_state = current_task.state;
            
            if current_task.state == TaskState::Running {
                current_task.state = TaskState::Ready;
                current_task.ticks_used = 0;
            }
            
            next_task.state = TaskState::Running;
            next_task.ticks_used = 0;
            
            // Actualizar TSS RSP0 si la nueva tarea es de usuario (x86_64)
            #[cfg(target_arch = "x86_64")]
            if next_task.is_user {
                crate::arch::x86_64::gdt::set_kernel_stack(next_task.kernel_stack_top);
            }
            
            let old_ctx = &mut current_task.context as *mut Context;
            let new_ctx = &next_task.context as *const Context;
            
            if current_task.state == TaskState::Ready {
                READY_QUEUE.lock().push_back(current_task);
            } else if let TaskState::Sleeping(_) = current_task.state {
                SLEEP_QUEUE.lock().push_back(current_task);
            }
            
            *current_guard = Some(next_task);
            drop(current_guard);
            
            IN_SCHEDULE.store(false, Ordering::SeqCst);
            
            unsafe {
                curr_arch::context::switch_context(old_ctx, new_ctx);
            }
            
            if was_enabled {
                curr_arch::enable_interrupts();
            }
            return;
        }
    }
    
    IN_SCHEDULE.store(false, Ordering::SeqCst);
    
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
    let current = CURRENT_TASK.lock();
    let (id, name) = current.as_ref()
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
    
    if let Some(ref task) = *CURRENT_TASK.lock() {
        tasks.push((task.id, task.name.clone(), task.state, task.total_ticks));
    }
    
    for task in READY_QUEUE.lock().iter() {
        tasks.push((task.id, task.name.clone(), task.state, task.total_ticks));
    }
    
    tasks
}

pub fn current_task_id() -> Option<TaskId> {
    CURRENT_TASK.lock().as_ref().map(|t| t.id)
}

pub fn current_task_name() -> Option<String> {
    CURRENT_TASK.lock().as_ref().map(|t| t.name.clone())
}

/// Ejecuta una función sobre la tarea actual
pub fn with_current_task<F, R>(f: F) -> Option<R> 
where F: FnOnce(&mut Task) -> R {
    let mut guard = CURRENT_TASK.lock();
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