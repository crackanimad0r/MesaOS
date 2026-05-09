// mesa_kernel/src/arch/aarch64/limine_req.rs
//! Requests y helpers para el bootloader Limine (AArch64 Stub)

use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest, SmpRequest,
};

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
static SMP_REQUEST: SmpRequest = SmpRequest::new();

#[used]
#[link_section = ".limine_requests_start"]
static _START_MARKER: [u64; 2] = [0xf6b8f4b39de7d1ae, 0xfab91fe462c7e4e0];

#[used]
#[link_section = ".limine_requests_end"]
static _END_MARKER: [u64; 2] = [0xf6b8f4b39de7d1ae, 0xfab91fe462c7e4e0];

pub fn is_supported() -> bool {
    false // TODO
}

pub fn framebuffer_response() -> Option<&'static limine::response::FramebufferResponse> {
    FRAMEBUFFER_REQUEST.get_response()
}

pub fn memory_map_entries() -> Option<&'static [&'static limine::memory_map::Entry]> {
    MEMORY_MAP_REQUEST.get_response().map(|r| r.entries())
}

pub fn hhdm_offset() -> Option<u64> {
    HHDM_REQUEST.get_response().map(|r| r.offset())
}

pub fn cpu_count() -> usize {
    SMP_REQUEST.get_response().map(|r| r.cpus().len()).unwrap_or(1)
}

pub fn kernel_address() -> Option<(u64, u64)> {
    None // TODO
}

pub fn rsdp_address() -> Option<u64> {
    None // TODO
}
