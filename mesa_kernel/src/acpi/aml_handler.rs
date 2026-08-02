// mesa_kernel/src/acpi/aml_handler.rs
//! Implementación de `aml::Handler` para MesaOS.
//!
//! El trait distingue tres espacios de direccionamiento:
//!  - `read_u8/write_u8` (y 16/32/64) → SystemMemory (MMIO con HHDM)
//!  - `read_io_u8/write_io_u8` (y 16/32) → SystemIO (puertos x86)
//!  - `read_pci_u8/write_pci_u8` (y 16/32) → PCI Config Space (CF8/CFC)

use aml::Handler;
use x86_64::instructions::port::Port;

pub struct MesaAmlHandler;

// ── Helpers internos ──────────────────────────────────────────────────────────

#[inline]
fn phys_to_virt(addr: usize) -> usize {
    let hhdm = crate::limine_req::hhdm_offset().unwrap_or(0) as usize;
    if addr < hhdm {
        addr + hhdm
    } else {
        addr
    }
}

fn pci_cfg_read_u32(bus: u8, dev: u8, func: u8, off: u16) -> u32 {
    let addr: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    unsafe {
        Port::<u32>::new(0xCF8).write(addr);
        Port::<u32>::new(0xCFC).read()
    }
}

fn pci_cfg_write_u32(bus: u8, dev: u8, func: u8, off: u16, val: u32) {
    let addr: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    unsafe {
        Port::<u32>::new(0xCF8).write(addr);
        Port::<u32>::new(0xCFC).write(val);
    }
}

// ── Implementación del trait ──────────────────────────────────────────────────

impl Handler for MesaAmlHandler {
    // ── SystemMemory (MMIO) ──────────────────────────────────────────────────

    fn read_u8(&self, address: usize) -> u8 {
        unsafe { core::ptr::read_volatile(phys_to_virt(address) as *const u8) }
    }
    fn read_u16(&self, address: usize) -> u16 {
        unsafe { core::ptr::read_volatile(phys_to_virt(address) as *const u16) }
    }
    fn read_u32(&self, address: usize) -> u32 {
        unsafe { core::ptr::read_volatile(phys_to_virt(address) as *const u32) }
    }
    fn read_u64(&self, address: usize) -> u64 {
        unsafe { core::ptr::read_volatile(phys_to_virt(address) as *const u64) }
    }

    fn write_u8(&mut self, address: usize, value: u8) {
        unsafe { core::ptr::write_volatile(phys_to_virt(address) as *mut u8, value) }
    }
    fn write_u16(&mut self, address: usize, value: u16) {
        unsafe { core::ptr::write_volatile(phys_to_virt(address) as *mut u16, value) }
    }
    fn write_u32(&mut self, address: usize, value: u32) {
        unsafe { core::ptr::write_volatile(phys_to_virt(address) as *mut u32, value) }
    }
    fn write_u64(&mut self, address: usize, value: u64) {
        unsafe { core::ptr::write_volatile(phys_to_virt(address) as *mut u64, value) }
    }

    // ── SystemIO (puertos x86) ───────────────────────────────────────────────

    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { Port::<u8>::new(port).read() }
    }
    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { Port::<u16>::new(port).read() }
    }
    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { Port::<u32>::new(port).read() }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { Port::<u8>::new(port).write(value) }
    }
    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { Port::<u16>::new(port).write(value) }
    }
    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { Port::<u32>::new(port).write(value) }
    }

    // ── PCI Config Space (CF8/CFC) ───────────────────────────────────────────

    fn read_pci_u8(&self, _seg: u16, bus: u8, dev: u8, func: u8, off: u16) -> u8 {
        let w = pci_cfg_read_u32(bus, dev, func, off);
        (w >> ((off & 3) * 8)) as u8
    }
    fn read_pci_u16(&self, _seg: u16, bus: u8, dev: u8, func: u8, off: u16) -> u16 {
        let w = pci_cfg_read_u32(bus, dev, func, off);
        (w >> ((off & 2) * 8)) as u16
    }
    fn read_pci_u32(&self, _seg: u16, bus: u8, dev: u8, func: u8, off: u16) -> u32 {
        pci_cfg_read_u32(bus, dev, func, off)
    }

    fn write_pci_u8(&self, _seg: u16, bus: u8, dev: u8, func: u8, off: u16, val: u8) {
        let shift = (off & 3) * 8;
        let mut w = pci_cfg_read_u32(bus, dev, func, off);
        w = (w & !(0xFF << shift)) | ((val as u32) << shift);
        pci_cfg_write_u32(bus, dev, func, off, w);
    }
    fn write_pci_u16(&self, _seg: u16, bus: u8, dev: u8, func: u8, off: u16, val: u16) {
        let shift = (off & 2) * 8;
        let mut w = pci_cfg_read_u32(bus, dev, func, off);
        w = (w & !(0xFFFF << shift)) | ((val as u32) << shift);
        pci_cfg_write_u32(bus, dev, func, off, w);
    }
    fn write_pci_u32(&self, _seg: u16, bus: u8, dev: u8, func: u8, off: u16, val: u32) {
        pci_cfg_write_u32(bus, dev, func, off, val);
    }
}
