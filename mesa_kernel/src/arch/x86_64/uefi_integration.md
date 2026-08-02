# UEFI NVRAM Integration Guide for MesaOS Kernel

## Overview

This document explains how to integrate the UEFI NVRAM C implementation with the MesaOS Rust kernel, including how to obtain and map the EFI_SYSTEM_TABLE to access Runtime Services.

## Files Created

1. **`uefi_nvram.h`** - Header with all UEFI type definitions, GUID, status codes, and function prototypes
2. **`uefi_nvram.c`** - Implementation of UEFI variable operations (SetVariable, GetVariable, etc.)

## Architecture Integration

### 1. Obtaining EFI_SYSTEM_TABLE at Boot

When booting via Limine (UEFI), the bootloader passes a pointer to `EFI_SYSTEM_TABLE` in the boot information. You need to capture this early in your boot process.

#### In Rust (Limine Request):

```rust
// Add to limine_req.rs or create a new UEFI request
use limine::request::UefiBootServicesRequest;

#[used]
#[link_section = ".limine_requests"]
static UEFI_BOOT_SERVICES: UefiBootServicesRequest = UefiBootServicesRequest::new();

// After Limine responses are available
fn get_uefi_system_table() -> Option<*const EfiSystemTable> {
    UEFI_BOOT_SERVICES.get_response()?.system_table()
}
```

**Note**: Limine's `UefiBootServicesRequest` provides access to both Boot Services and Runtime Services. The system table pointer remains valid after `ExitBootServices()`.

### 2. Extracting Runtime Services Pointer

The `EFI_SYSTEM_TABLE` structure contains a pointer to `EFI_RUNTIME_SERVICES`:

```c
// C structure (from UEFI spec)
typedef struct {
    EFI_TABLE_HEADER                Hdr;
    CHAR16*                         FirmwareVendor;
    UINT32                          FirmwareRevision;
    EFI_HANDLE                      ConsoleInHandle;
    EFI_SIMPLE_TEXT_INPUT_PROTOCOL* ConIn;
    EFI_HANDLE                      ConsoleOutHandle;
    EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL* ConOut;
    EFI_HANDLE                      StandardErrorHandle;
    EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL* StdErr;
    EFI_RUNTIME_SERVICES*           RuntimeServices;  // <-- THIS IS WHAT WE NEED
    EFI_BOOT_SERVICES*              BootServices;
    UINTN                           NumberOfTableEntries;
    EFI_CONFIGURATION_TABLE*        ConfigurationTable;
} EFI_SYSTEM_TABLE;
```

### 3. Initialization in Kernel Entry Point

```rust
// In kernel_start() or early init (before ExitBootServices if possible, or after)

// Option A: Get RuntimeServices directly from Limine's UEFI request
if let Some(uefi_response) = UEFI_BOOT_SERVICES.get_response() {
    let runtime_services = uefi_response.runtime_services(); // EFI_RUNTIME_SERVICES*
    
    // Initialize our C NVRAM module
    unsafe {
        uefi_nvram_init(runtime_services as *mut c_void);
    }
}

// Option B: If you have EFI_SYSTEM_TABLE pointer directly
// let sys_table: *const EfiSystemTable = ...;
// let runtime_services = (*sys_table).RuntimeServices;
// unsafe { uefi_nvram_init(runtime_services as *mut c_void); }
```

### 4. C Function Declarations for Rust FFI

```rust
// In a Rust module (e.g., arch/x86_64/uefi_nvram.rs)

use core::ffi::{c_void, c_char, c_uint, c_ulonglong, c_ushort};

// UEFI Types matching C definitions
pub type EFI_STATUS = usize;
pub type UINTN = usize;
pub type UINT32 = u32;
pub type UINT16 = u16;
pub type CHAR16 = u16;
pub type VOID = c_void;

#[repr(C)]
pub struct EFI_GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

extern "C" {
    pub fn uefi_nvram_init(runtime_services: *mut c_void);
    pub fn uefi_set_var(
        name: *const CHAR16,
        guid: *const EFI_GUID,
        data: *const VOID,
        data_size: UINTN,
    ) -> EFI_STATUS;
    pub fn uefi_get_var(
        name: *const CHAR16,
        guid: *const EFI_GUID,
        buffer: *mut VOID,
        buffer_size: *mut UINTN,
    ) -> EFI_STATUS;
    pub fn uefi_get_var_ex(
        name: *const CHAR16,
        guid: *const EFI_GUID,
        attributes: *mut UINT32,
        buffer: *mut VOID,
        buffer_size: *mut UINTN,
    ) -> EFI_STATUS;
    pub fn uefi_delete_var(
        name: *const CHAR16,
        guid: *const EFI_GUID,
    ) -> EFI_STATUS;
}
```

### 5. Build System Integration

Update `build.rs` to compile the C files:

```rust
// mesa_kernel/build.rs
fn main() {
    cc::Build::new()
        .file("src/arch/x86_64/uefi_nvram.c")
        .include("src/arch/x86_64")
        .compile("uefi_nvram");
    
    println!("cargo:rerun-if-changed=src/arch/x86_64/uefi_nvram.c");
    println!("cargo:rerun-if-changed=src/arch/x86_64/uefi_nvram.h");
}
```

Update `Cargo.toml`:
```toml
[build-dependencies]
cc = "1.0"
```

## Usage Examples

### Define a Kernel-Specific GUID

```rust
// Use a unique GUID for your kernel variables
// Generate with: uuidgen (Linux) or [System.Guid]::NewGuid() (PowerShell)
const MESA_KERNEL_GUID: EFI_GUID = EFI_GUID {
    data1: 0x12345678,
    data2: 0x9ABC,
    data3: 0xDEF0,
    data4: [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
};
```

### String Conversion Helpers

```rust
// Convert Rust string to UTF-16 (CHAR16*) for UEFI
fn to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect() // Null-terminated
}

// Convert UTF-16 back to Rust String
fn from_utf16(ptr: *const u16, max_len: usize) -> String {
    let mut result = String::new();
    unsafe {
        for i in 0..max_len {
            let c = *ptr.add(i);
            if c == 0 { break; }
            if let Some(ch) = char::from_u32(c as u32) {
                result.push(ch);
            }
        }
    }
    result
}
```

### Store Kernel Configuration

```rust
fn save_boot_config(config: &BootConfig) -> Result<(), &'static str> {
    let name = to_utf16("MesaBootConfig");
    let data = bincode::serialize(config).map_err(|_| "Serialization failed")?;
    
    let status = unsafe {
        uefi_set_var(
            name.as_ptr(),
            &MESA_KERNEL_GUID,
            data.as_ptr() as *const c_void,
            data.len(),
        )
    };
    
    if status == EFI_SUCCESS {
        Ok(())
    } else {
        Err("Failed to save boot config")
    }
}
```

### Load Kernel Configuration

```rust
fn load_boot_config() -> Result<BootConfig, &'static str> {
    let name = to_utf16("MesaBootConfig");
    let mut buffer_size: usize = 0;
    
    // First call: get required size
    let status = unsafe {
        uefi_get_var(
            name.as_ptr(),
            &MESA_KERNEL_GUID,
            core::ptr::null_mut(),
            &mut buffer_size,
        )
    };
    
    if status != EFI_BUFFER_TOO_SMALL {
        return Err("Variable not found or error");
    }
    
    // Allocate buffer and read
    let mut buffer = vec![0u8; buffer_size];
    let status = unsafe {
        uefi_get_var(
            name.as_ptr(),
            &MESA_KERNEL_GUID,
            buffer.as_mut_ptr() as *mut c_void,
            &mut buffer_size,
        )
    };
    
    if status != EFI_SUCCESS {
        return Err("Failed to read boot config");
    }
    
    let config: BootConfig = bincode::deserialize(&buffer[..buffer_size])
        .map_err(|_| "Deserialization failed")?;
    
    Ok(config)
}
```

## Variable Attributes Explained

### Standard Attributes for Persistent Data (0x00000007)

```c
#define EFI_VARIABLE_STANDARD_ATTRIBUTES \
    (EFI_VARIABLE_NON_VOLATILE | EFI_VARIABLE_BOOTSERVICE_ACCESS | EFI_VARIABLE_RUNTIME_ACCESS)
```

| Attribute | Value | Purpose |
|-----------|-------|---------|
| `NON_VOLATILE` | 0x00000001 | **Critical**: Data stored in NVRAM (flash), survives power cycles/reboots |
| `BOOTSERVICE_ACCESS` | 0x00000002 | Accessible during boot services phase (before ExitBootServices) |
| `RUNTIME_ACCESS` | 0x00000004 | **Critical**: Accessible after ExitBootServices() at kernel runtime |

### Why These Three Attributes?

1. **NON_VOLATILE**: Without this, variables are stored in volatile memory and lost on reboot
2. **BOOTSERVICE_ACCESS**: Allows bootloader/early boot code to read/write the variable
3. **RUNTIME_ACCESS**: Allows your kernel to read/write after taking control of the system

### Other Useful Attributes

| Attribute | Value | Use Case |
|-----------|-------|----------|
| `HARDWARE_ERROR_RECORD` | 0x00000008 | For hardware error logs (ACPI HEST) |
| `AUTHENTICATED_WRITE_ACCESS` | 0x00000010 | Signed variables (Secure Boot) |
| `TIME_BASED_AUTHENTICATED_WRITE_ACCESS` | 0x00000020 | Time-based auth (Key rotation) |
| `APPEND_WRITE` | 0x00000040 | Append-only variables (logs) |

## Error Handling

```rust
fn efi_status_to_str(status: EFI_STATUS) -> &'static str {
    match status {
        0 => "EFI_SUCCESS",
        e if e & (1 << 63) != 0 => match e & 0xFF {
            1 => "EFI_LOAD_ERROR",
            2 => "EFI_INVALID_PARAMETER",
            3 => "EFI_UNSUPPORTED",
            4 => "EFI_BAD_BUFFER_SIZE",
            5 => "EFI_BUFFER_TOO_SMALL",
            6 => "EFI_NOT_READY",
            7 => "EFI_DEVICE_ERROR",
            8 => "EFI_WRITE_PROTECTED",
            9 => "EFI_OUT_OF_RESOURCES",
            10 => "EFI_VOLUME_CORRUPTED",
            11 => "EFI_VOLUME_FULL",
            14 => "EFI_NOT_FOUND",
            15 => "EFI_ACCESS_DENIED",
            _ => "EFI_ERROR (unknown)",
        },
        _ => "Unknown status",
    }
}
```

## Important Notes

### 1. Runtime Services Availability
- Call `uefi_nvram_init()` **after** `ExitBootServices()` or from a runtime context
- The `EFI_RUNTIME_SERVICES` pointer remains valid after ExitBootServices
- Some firmware requires virtual address mapping via `SetVirtualAddressMap()` before using runtime services at virtual addresses

### 2. Variable Size Limits
- UEFI spec doesn't mandate minimum size, but typical limits:
  - **NVRAM total**: 64KB - 1024KB depending on firmware
  - **Per variable**: Typically 1KB - 64KB
  - Use `QueryVariableInfo()` to check available space

### 3. String Encoding
- Variable names are **UTF-16 (CHAR16*)** null-terminated
- Use `to_utf16()` helper from Rust

### 4. Thread Safety
- UEFI Runtime Services are not thread-safe
- In a multi-core kernel, serialize access with a mutex/spinlock

### 5. Time Services
The header also includes time service definitions (`GetTime`, `SetTime`, etc.) if you need RTC access through UEFI instead of direct CMOS/RTC port access.

## Testing

```rust
#[cfg(test)]
fn test_nvram_roundtrip() {
    let test_guid = EFI_GUID { ... };
    let test_name = to_utf16("MesaTestVar");
    let test_data = b"Hello NVRAM!";
    
    // Write
    let status = unsafe { uefi_set_var(test_name.as_ptr(), &test_guid, test_data.as_ptr() as _, test_data.len()) };
    assert_eq!(status, EFI_SUCCESS);
    
    // Read
    let mut buf = vec![0u8; 64];
    let mut size = buf.len();
    let status = unsafe { uefi_get_var(test_name.as_ptr(), &test_guid, buf.as_mut_ptr() as _, &mut size) };
    assert_eq!(status, EFI_SUCCESS);
    assert_eq!(&buf[..size], test_data);
    
    // Delete
    let status = unsafe { uefi_delete_var(test_name.as_ptr(), &test_guid) };
    assert_eq!(status, EFI_SUCCESS);
}
```

## Summary

1. **Capture EFI_SYSTEM_TABLE** from Limine's UEFI request
2. **Extract RuntimeServices** pointer from system table
3. **Initialize** with `uefi_nvram_init(runtime_services)`
4. **Use** `uefi_set_var()` / `uefi_get_var()` with your kernel GUID
5. **Build** with cc crate in build.rs

The implementation uses standard attributes (NON_VOLATILE | BOOTSERVICE_ACCESS | RUNTIME_ACCESS = 0x07) ensuring data persists across reboots and is accessible at kernel runtime.