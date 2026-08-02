use core::cell::UnsafeCell;
use core::sync::atomic::{fence, Ordering};

pub const SHIM_MAGIC: u32 = 0x4D455341;
pub const SHIM_VERSION: u32 = 1;
pub const SHIM_DATA_POOL_SIZE: usize = 64 * 1024;
pub const SCM_QUEUE_DEPTH: usize = 64;

pub const SCM_NOP: u32 = 0x00;
pub const SCM_USB_CONTROL: u32 = 0x01;
pub const SCM_USB_BULK: u32 = 0x02;
pub const SCM_USB_ALLOC_URB: u32 = 0x03;
pub const SCM_USB_FREE_URB: u32 = 0x04;
pub const SCM_USB_SUBMIT_URB: u32 = 0x05;
pub const SCM_USB_KILL_URB: u32 = 0x06;
pub const SCM_USB_RESET_DEVICE: u32 = 0x07;
pub const SCM_USB_GET_DESCRIPTOR: u32 = 0x08;
pub const SCM_USB_SET_CONFIG: u32 = 0x09;
pub const SCM_USB_CLAIM_INTF: u32 = 0x0A;
pub const SCM_USB_RELEASE_INTF: u32 = 0x0B;
pub const SCM_WIFI_INIT: u32 = 0x10;
pub const SCM_WIFI_SEND_SKB: u32 = 0x11;
pub const SCM_WIFI_RECV_SKB: u32 = 0x12;
pub const SCM_WIFI_SET_CHANNEL: u32 = 0x13;
pub const SCM_WIFI_SET_MAC: u32 = 0x14;
pub const SCM_WIFI_LINK_STATUS: u32 = 0x15;
pub const SCM_WIFI_SCAN: u32 = 0x16;
pub const SCM_SHIM_HEARTBEAT: u32 = 0xFE;
pub const SCM_SHIM_PANIC: u32 = 0xFF;

pub const EVT_NONE: u32 = 0x00;
pub const EVT_URB_COMPLETE: u32 = 0x01;
pub const EVT_URB_ERROR: u32 = 0x02;
pub const EVT_WIFI_RX_PACKET: u32 = 0x10;
pub const EVT_WIFI_LINK_CHANGE: u32 = 0x11;
pub const EVT_WIFI_SCAN_RESULT: u32 = 0x12;
pub const EVT_SHIM_HEARTBEAT_ACK: u32 = 0xFE;
pub const EVT_SHIM_ERROR: u32 = 0xFF;

pub const SCM_OK: i32 = 0;
pub const SCM_ERR_GENERAL: i32 = -1;
pub const SCM_ERR_NOMEM: i32 = -12;
pub const SCM_ERR_TIMEOUT: i32 = -110;
pub const SCM_ERR_SHUTDOWN: i32 = -108;
pub const SCM_ERR_BUSY: i32 = -16;

pub const SHIM_FLAG_RUNNING: u32 = 1 << 0;
pub const SHIM_FLAG_CRASHED: u32 = 1 << 1;
pub const SHIM_FLAG_SUSPENDED: u32 = 1 << 2;
pub const SHIM_FLAG_NEED_RESPAWN: u32 = 1 << 3;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ScmCommand {
    pub cmd_type: u32,
    pub id: u32,
    pub flags: u32,
    pub _rsvd1: u32,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
    pub data_len: u32,
    pub data_ofs: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ScmEvent {
    pub evt_type: u32,
    pub id: u32,
    pub status: i32,
    pub actual_len: u32,
    pub data_ofs: u32,
    pub data_len: u32,
    pub reserved: u64,
}

#[repr(C)]
pub struct ScmQueue {
    pub head: UnsafeCell<u32>,
    pub tail: UnsafeCell<u32>,
    pub _pad: [u8; 56],
    pub entries: [ScmCommand; SCM_QUEUE_DEPTH],
}

#[repr(C)]
pub struct ShimRegion {
    pub magic: u32,
    pub version: u32,
    pub flags: u32,
    pub heartbeat_counter: u32,
    pub kernel_private: u64,
    pub shim_private: u64,
    pub _pad0: [u8; 32],
    pub cmd_queue: ScmQueue,
    pub _pad1: [u8; 128],
    pub evt_queue: ScmQueue,
    pub data_pool: [u8; SHIM_DATA_POOL_SIZE],
}

impl ShimRegion {
    pub unsafe fn init(&mut self) {
        self.magic = SHIM_MAGIC;
        self.version = SHIM_VERSION;
        self.flags = SHIM_FLAG_RUNNING;
        self.heartbeat_counter = 0;
        self.kernel_private = 0;
        self.shim_private = 0;
        *self.cmd_queue.head.get_mut() = 0;
        *self.cmd_queue.tail.get_mut() = 0;
        *self.evt_queue.head.get_mut() = 0;
        *self.evt_queue.tail.get_mut() = 0;
    }
}

pub unsafe fn scm_queue_push(queue: &ScmQueue, cmd: &ScmCommand) -> i32 {
    let head = core::ptr::read_volatile(queue.head.get());
    let tail = core::ptr::read_volatile(queue.tail.get());
    let next = (head + 1) % SCM_QUEUE_DEPTH as u32;
    if next == tail {
        return -1;
    }
    let entry = &queue.entries[head as usize] as *const ScmCommand as *mut ScmCommand;
    core::ptr::copy_nonoverlapping(cmd, entry, 1);
    fence(Ordering::Release);
    core::ptr::write_volatile(queue.head.get(), next);
    0
}

pub unsafe fn scm_queue_pop(queue: &ScmQueue, cmd: &mut ScmCommand) -> i32 {
    let head = core::ptr::read_volatile(queue.head.get());
    let tail = core::ptr::read_volatile(queue.tail.get());
    if head == tail {
        return -1;
    }
    let entry = &queue.entries[tail as usize];
    core::ptr::copy_nonoverlapping(entry, cmd, 1);
    fence(Ordering::Release);
    core::ptr::write_volatile(queue.tail.get(), (tail + 1) % SCM_QUEUE_DEPTH as u32);
    0
}

pub unsafe fn scm_event_push(queue: &ScmQueue, evt: &ScmEvent) -> i32 {
    let head = core::ptr::read_volatile(queue.head.get());
    let tail = core::ptr::read_volatile(queue.tail.get());
    let next = (head + 1) % SCM_QUEUE_DEPTH as u32;
    if next == tail {
        return -1;
    }
    let entry = &queue.entries[head as usize] as *const ScmCommand as *mut ScmCommand;
    core::ptr::copy_nonoverlapping(evt as *const ScmEvent as *const ScmCommand, entry, 1);
    fence(Ordering::Release);
    core::ptr::write_volatile(queue.head.get(), next);
    0
}

pub unsafe fn scm_event_pop(queue: &ScmQueue, evt: &mut ScmEvent) -> i32 {
    let head = core::ptr::read_volatile(queue.head.get());
    let tail = core::ptr::read_volatile(queue.tail.get());
    if head == tail {
        return -1;
    }
    let entry = &queue.entries[tail as usize] as *const ScmCommand as *const ScmEvent;
    core::ptr::copy_nonoverlapping(entry, evt, 1);
    fence(Ordering::Acquire);
    core::ptr::write_volatile(queue.tail.get(), (tail + 1) % SCM_QUEUE_DEPTH as u32);
    0
}

pub unsafe fn scm_data_pool_alloc(_region: &ShimRegion, size: usize) -> i32 {
    static mut POOL_OFFSET: usize = 0;
    let offset = POOL_OFFSET;
    if offset + size > SHIM_DATA_POOL_SIZE {
        return -1;
    }
    POOL_OFFSET = offset + size;
    offset as i32
}
