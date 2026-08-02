use core::ffi::CStr;

#[repr(C)]
pub struct ShimUsbContext {
    pub region: *mut u8,
    pub xhci_mmio: *mut u8,
    pub xhci_mmio_sz: u64,
    pub irq_handle: i32,
    pub driver_instance: *mut u8,
    pub driver_probe: Option<unsafe extern "C" fn(*mut ShimUsbContext) -> i32>,
    pub driver_disconnect: Option<unsafe extern "C" fn(*mut ShimUsbContext)>,
}

impl ShimUsbContext {
    pub const fn new() -> Self {
        Self {
            region: core::ptr::null_mut(),
            xhci_mmio: core::ptr::null_mut(),
            xhci_mmio_sz: 0,
            irq_handle: -1,
            driver_instance: core::ptr::null_mut(),
            driver_probe: None,
            driver_disconnect: None,
        }
    }
}

pub type ShimEntryFn =
    unsafe extern "C" fn(region_phys: u64, mmio_phys: u64, mmio_size: u64, pci_bdf: u32);

pub type ShimHandleCommandFn = unsafe extern "C" fn(ctx: *mut ShimUsbContext, cmd: *const u8);

pub type ShimSendEventFn = unsafe extern "C" fn(ctx: *mut ShimUsbContext, event: *const u8) -> i32;

pub type ShimPollIrqFn = unsafe extern "C" fn(ctx: *mut ShimUsbContext);

pub fn get_shim_entry_fn() -> Option<ShimEntryFn> {
    None
}
