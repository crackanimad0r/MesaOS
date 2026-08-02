/**
 * UEFI NVRAM Rust FFI Bindings for MesaOS Kernel
 *
 * Provides safe Rust wrappers around the C UEFI NVRAM implementation.
 * The EFI_RUNTIME_SERVICES pointer must be obtained from the bootloader
 * and passed to `init()` before any variable operations.
 */
use core::ffi::c_void;
use core::fmt;

// ============================================================
// UEFI Type Definitions (matching C header)
// ============================================================

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EFI_STATUS(pub usize);

pub type UINTN = usize;
pub type UINT32 = u32;
pub type UINT16 = u16;
pub type CHAR16 = u16;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EFI_GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl EFI_GUID {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self {
            data1,
            data2,
            data3,
            data4,
        }
    }
}

// ============================================================
// EFI Status Codes
// ============================================================

pub const EFI_SUCCESS: EFI_STATUS = EFI_STATUS(0);
pub const EFI_INVALID_PARAMETER: EFI_STATUS = EFI_STATUS(2 | (1usize << 63));
pub const EFI_UNSUPPORTED: EFI_STATUS = EFI_STATUS(3 | (1usize << 63));
pub const EFI_BUFFER_TOO_SMALL: EFI_STATUS = EFI_STATUS(5 | (1usize << 63));
pub const EFI_NOT_READY: EFI_STATUS = EFI_STATUS(6 | (1usize << 63));
pub const EFI_DEVICE_ERROR: EFI_STATUS = EFI_STATUS(7 | (1usize << 63));
pub const EFI_NOT_FOUND: EFI_STATUS = EFI_STATUS(14 | (1usize << 63));
pub const EFI_VOLUME_FULL: EFI_STATUS = EFI_STATUS(11 | (1usize << 63));
pub const EFI_ACCESS_DENIED: EFI_STATUS = EFI_STATUS(15 | (1usize << 63));

// ============================================================
// Variable Attributes
// ============================================================

pub const EFI_VARIABLE_NON_VOLATILE: UINT32 = 0x00000001;
pub const EFI_VARIABLE_BOOTSERVICE_ACCESS: UINT32 = 0x00000002;
pub const EFI_VARIABLE_RUNTIME_ACCESS: UINT32 = 0x00000004;

pub const EFI_VARIABLE_STANDARD_ATTRIBUTES: UINT32 =
    EFI_VARIABLE_NON_VOLATILE | EFI_VARIABLE_BOOTSERVICE_ACCESS | EFI_VARIABLE_RUNTIME_ACCESS;

// ============================================================
// External C Functions (FFI)
// ============================================================

extern "C" {
    pub fn uefi_nvram_init(runtime_services: *mut c_void);
    pub fn uefi_set_var(
        name: *const CHAR16,
        guid: *const EFI_GUID,
        data: *const c_void,
        data_size: UINTN,
    ) -> EFI_STATUS;
    pub fn uefi_get_var(
        name: *const CHAR16,
        guid: *const EFI_GUID,
        buffer: *mut c_void,
        buffer_size: *mut UINTN,
    ) -> EFI_STATUS;
    pub fn uefi_get_var_ex(
        name: *const CHAR16,
        guid: *const EFI_GUID,
        attributes: *mut UINT32,
        buffer: *mut c_void,
        buffer_size: *mut UINTN,
    ) -> EFI_STATUS;
    pub fn uefi_delete_var(name: *const CHAR16, guid: *const EFI_GUID) -> EFI_STATUS;
}

// ============================================================
// Kernel-Specific GUID
// ============================================================

/// MesaOS Kernel Variable Namespace GUID
pub const MESA_KERNEL_GUID: EFI_GUID = EFI_GUID::new(
    0x4d455341, // "MESA" in ASCII
    0x4f53,     // "OS"
    0x0001,
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
);

// ============================================================
// Safe Rust Wrappers
// ============================================================

/// Initialize UEFI NVRAM subsystem
///
/// Must be called once during kernel initialization with a valid
/// EFI_RUNTIME_SERVICES pointer obtained from the bootloader.
///
/// # Safety
/// Caller must ensure `runtime_services` is a valid pointer to
/// the UEFI Runtime Services table.
pub unsafe fn init(runtime_services: *mut c_void) {
    uefi_nvram_init(runtime_services);
}

/// Convert a Rust string to UTF-16 (null-terminated) for UEFI
pub fn str_to_utf16(s: &str) -> alloc::vec::Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Set a UEFI variable (persist to NVRAM)
///
/// Uses standard attributes: NON_VOLATILE | BOOTSERVICE_ACCESS | RUNTIME_ACCESS
pub fn set_var(name: &[u16], guid: &EFI_GUID, data: &[u8]) -> Result<(), EFI_STATUS> {
    let status = unsafe {
        uefi_set_var(
            name.as_ptr(),
            guid as *const EFI_GUID,
            data.as_ptr() as *const c_void,
            data.len(),
        )
    };
    if status == EFI_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

/// Get a UEFI variable (read from NVRAM)
pub fn get_var(name: &[u16], guid: &EFI_GUID) -> Result<alloc::vec::Vec<u8>, EFI_STATUS> {
    let mut buffer_size: UINTN = 0;
    let status = unsafe {
        uefi_get_var(
            name.as_ptr(),
            guid as *const EFI_GUID,
            core::ptr::null_mut(),
            &mut buffer_size,
        )
    };
    if status != EFI_BUFFER_TOO_SMALL {
        return Err(status);
    }
    let mut buffer = alloc::vec![0u8; buffer_size];
    let status = unsafe {
        uefi_get_var(
            name.as_ptr(),
            guid as *const EFI_GUID,
            buffer.as_mut_ptr() as *mut c_void,
            &mut buffer_size,
        )
    };
    if status == EFI_SUCCESS {
        buffer.truncate(buffer_size);
        Ok(buffer)
    } else {
        Err(status)
    }
}

/// Get a UEFI variable with attributes
pub fn get_var_ex(
    name: &[u16],
    guid: &EFI_GUID,
) -> Result<(alloc::vec::Vec<u8>, UINT32), EFI_STATUS> {
    let mut buffer_size: UINTN = 0;
    let mut attributes: UINT32 = 0;
    let status = unsafe {
        uefi_get_var_ex(
            name.as_ptr(),
            guid as *const EFI_GUID,
            &mut attributes,
            core::ptr::null_mut(),
            &mut buffer_size,
        )
    };
    if status != EFI_BUFFER_TOO_SMALL {
        return Err(status);
    }
    let mut buffer = alloc::vec![0u8; buffer_size];
    let status = unsafe {
        uefi_get_var_ex(
            name.as_ptr(),
            guid as *const EFI_GUID,
            &mut attributes,
            buffer.as_mut_ptr() as *mut c_void,
            &mut buffer_size,
        )
    };
    if status == EFI_SUCCESS {
        buffer.truncate(buffer_size);
        Ok((buffer, attributes))
    } else {
        Err(status)
    }
}

/// Delete a UEFI variable
pub fn delete_var(name: &[u16], guid: &EFI_GUID) -> Result<(), EFI_STATUS> {
    let status = unsafe { uefi_delete_var(name.as_ptr(), guid as *const EFI_GUID) };
    if status == EFI_SUCCESS {
        Ok(())
    } else {
        Err(status)
    }
}

// ============================================================
// Status Code Display
// ============================================================

impl fmt::Display for EFI_STATUS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val = self.0;
        let msg = match val {
            0 => "EFI_SUCCESS",
            v if v == EFI_INVALID_PARAMETER.0 => "EFI_INVALID_PARAMETER",
            v if v == EFI_UNSUPPORTED.0 => "EFI_UNSUPPORTED",
            v if v == EFI_BUFFER_TOO_SMALL.0 => "EFI_BUFFER_TOO_SMALL",
            v if v == EFI_NOT_READY.0 => "EFI_NOT_READY",
            v if v == EFI_DEVICE_ERROR.0 => "EFI_DEVICE_ERROR",
            v if v == EFI_NOT_FOUND.0 => "EFI_NOT_FOUND",
            v if v == EFI_VOLUME_FULL.0 => "EFI_VOLUME_FULL",
            v if v == EFI_ACCESS_DENIED.0 => "EFI_ACCESS_DENIED",
            v if (v as isize) < 0 => "EFI_ERROR (high bit set)",
            _ => "EFI_SUCCESS (unknown)",
        };
        write!(f, "{} (0x{:X})", msg, val)
    }
}

impl fmt::Debug for EFI_STATUS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
