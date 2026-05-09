// mesa_kernel/src/limine_req.rs
//! Requests y helpers para el bootloader Limine

use limine::request::{
    DateAtBootRequest, FramebufferRequest, HhdmRequest, ExecutableAddressRequest,
    MemoryMapRequest, RsdpRequest, MpRequest,
};
use limine::memory_map::Entry;

// ══════════════════════════════════════════════════════════════════════════════
// LIMINE REQUESTS
// ══════════════════════════════════════════════════════════════════════════════

#[used]
#[link_section = ".limine_requests"]
static BOOTLOADER_INFO: limine::request::BootloaderInfoRequest =
    limine::request::BootloaderInfoRequest::new();

#[used]
#[link_section = ".limine_requests"]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[link_section = ".limine_requests"]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[used]
#[link_section = ".limine_requests"]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[link_section = ".limine_requests"]
static KERNEL_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

#[used]
#[link_section = ".limine_requests"]
static SMP_REQUEST: MpRequest = MpRequest::new();

#[used]
#[link_section = ".limine_requests"]
static BOOT_TIME_REQUEST: DateAtBootRequest = DateAtBootRequest::new();

#[used]
#[link_section = ".limine_requests"]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

// Marker para que Limine sepa que es compatible
#[used]
#[link_section = ".limine_requests_start"]
static _START_MARKER: [u64; 2] = [0xf6b8f4b39de7d1ae, 0xfab91fe462c7e4e0];

#[used]
#[link_section = ".limine_requests_end"]
static _END_MARKER: [u64; 2] = [0xf6b8f4b39de7d1ae, 0xfab91fe462c7e4e0];

// ══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ══════════════════════════════════════════════════════════════════════════════

/// Verifica que Limine haya respondido correctamente
pub fn is_supported() -> bool {
    FRAMEBUFFER_REQUEST.get_response().is_some()
        && MEMORY_MAP_REQUEST.get_response().is_some()
        && HHDM_REQUEST.get_response().is_some()
}

/// Obtiene la respuesta del framebuffer
pub fn framebuffer_response() -> Option<&'static limine::response::FramebufferResponse> {
    FRAMEBUFFER_REQUEST.get_response()
}

/// Obtiene las entradas del memory map
pub fn memory_map_entries() -> Option<&'static [&'static Entry]> {
    MEMORY_MAP_REQUEST
        .get_response()
        .map(|r| r.entries())
}

/// Obtiene el offset del Higher Half Direct Map
pub fn hhdm_offset() -> Option<u64> {
    HHDM_REQUEST
        .get_response()
        .map(|r| r.offset())
}

/// Obtiene la dirección física y virtual del kernel
pub fn kernel_address() -> Option<(u64, u64)> {
    KERNEL_ADDRESS_REQUEST.get_response().map(|r| {
        (r.physical_base(), r.virtual_base())
    })
}

/// Obtiene el número de CPUs detectados
pub fn cpu_count() -> usize {
    SMP_REQUEST
        .get_response()
        .map(|r| r.cpus().len())
        .unwrap_or(1)
}

/// Obtiene la información del bootloader
pub fn bootloader_info() -> Option<(&'static str, &'static str)> {
    BOOTLOADER_INFO.get_response().map(|r| {
        (r.name(), r.version())
    })
}

/// Obtiene el boot time (microsegundos desde epoch)
pub fn boot_time() -> Option<core::time::Duration> {
    BOOT_TIME_REQUEST
        .get_response()
        .map(|r| r.timestamp())
}

/// Obtiene la dirección del RSDP (ACPI)
pub fn rsdp_address() -> Option<u64> {
    RSDP_REQUEST
        .get_response()
        .map(|r| r.address() as u64)
}

/// Obtiene la respuesta de SMP (para iniciar otros cores)
pub fn smp_response() -> Option<&'static limine::response::MpResponse> {
    SMP_REQUEST.get_response()
}