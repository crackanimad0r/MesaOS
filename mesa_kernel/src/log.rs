// mesa_kernel/src/log.rs

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::format;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn prefix(&self) -> &'static str {
        match self {
            LogLevel::Debug => "[D]",
            LogLevel::Info => "[I]",
            LogLevel::Warn => "[W]",
            LogLevel::Error => "[E]",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub timestamp: u64,
    pub message: String,
}

pub struct KernelLog {
    entries: VecDeque<LogEntry>,
    capacity: usize,
    initialized: bool,
}

impl KernelLog {
    pub const fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: 256,
            initialized: false,
        }
    }
    
    pub fn init(&mut self, capacity: usize) {
        self.capacity = capacity;
        self.entries = VecDeque::with_capacity(capacity);
        self.initialized = true;
    }
    
    pub fn log(&mut self, level: LogLevel, message: String) {
        if !self.initialized {
            return;
        }
        
        let timestamp = crate::curr_arch::get_ticks();
        
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        
        self.entries.push_back(LogEntry {
            level,
            timestamp,
            message,
        });
    }
    
    pub fn last_n(&self, n: usize) -> impl Iterator<Item = &LogEntry> {
        let skip = if self.entries.len() > n {
            self.entries.len() - n
        } else {
            0
        };
        self.entries.iter().skip(skip)
    }
    
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub static KERNEL_LOG: Mutex<KernelLog> = Mutex::new(KernelLog::new());

pub fn init() {
    KERNEL_LOG.lock().init(256);
}

pub fn log_fmt(level: LogLevel, args: core::fmt::Arguments) {
    if let Some(mut log) = KERNEL_LOG.try_lock() {
        log.log(level, format!("{}", args));
    }
}

#[macro_export]
macro_rules! klog_info {
    ($($arg:tt)*) => {
        $crate::log::log_fmt($crate::log::LogLevel::Info, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! klog_warn {
    ($($arg:tt)*) => {
        $crate::log::log_fmt($crate::log::LogLevel::Warn, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! klog_error {
    ($($arg:tt)*) => {
        $crate::log::log_fmt($crate::log::LogLevel::Error, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! klog_debug {
    ($($arg:tt)*) => {
        $crate::log::log_fmt($crate::log::LogLevel::Debug, format_args!($($arg)*))
    };
}