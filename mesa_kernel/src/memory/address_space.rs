//! Espacios de direcciones por proceso (page tables separadas)

use alloc::vec::Vec;
#[cfg(target_arch = "x86_64")]
use core::arch::asm;
use crate::memory::{pmm, vmm, PAGE_SIZE};

/// Flags de página
pub mod flags {
    pub const PRESENT: u64      = 1 << 0;
    pub const WRITABLE: u64     = 1 << 1;
    pub const USER: u64         = 1 << 2;
    pub const WRITE_THROUGH: u64 = 1 << 3;
    pub const NO_CACHE: u64     = 1 << 4;
    pub const ACCESSED: u64     = 1 << 5;
    pub const DIRTY: u64        = 1 << 6;
    pub const HUGE_PAGE: u64    = 1 << 7;
    pub const GLOBAL: u64       = 1 << 8;
    pub const NO_EXECUTE: u64   = 1 << 63;
    
    pub const KERNEL_RW: u64 = PRESENT | WRITABLE;
    pub const KERNEL_RX: u64 = PRESENT;
    pub const USER_RW: u64   = PRESENT | WRITABLE | USER;
    pub const USER_RX: u64   = PRESENT | USER;
    pub const USER_RWX: u64  = PRESENT | WRITABLE | USER;
}

/// Direcciones estándar para procesos de usuario
pub mod layout {
    /// Base del código de usuario (1 MB)
    pub const USER_CODE_BASE: u64 = 0x0000_0000_0010_0000;
    /// Base del stack de usuario (128 MB, crece hacia abajo)
    pub const USER_STACK_TOP: u64 = 0x0000_0000_0800_0000;
    /// Tamaño del stack de usuario (64 KB)
    pub const USER_STACK_SIZE: u64 = 64 * 1024;
    /// Base del heap de usuario (16 MB)
    pub const USER_HEAP_BASE: u64 = 0x0000_0000_0100_0000;
}

/// Espacio de direcciones de un proceso
pub struct AddressSpace {
    #[cfg(target_arch = "x86_64")]
    pml4_phys: u64,
    #[cfg(target_arch = "aarch64")]
    ttbr0_phys: u64,
    
    allocated_frames: Vec<u64>,
    is_kernel: bool,
}

impl AddressSpace {
    #[cfg(target_arch = "x86_64")]
    pub fn new() -> Result<Self, &'static str> {
        // x86_64 implementation
        let pml4_phys = pmm::alloc_frame().ok_or("No memory for PML4")?;
        let pml4_virt = vmm::phys_to_virt(pml4_phys);
        unsafe { core::ptr::write_bytes(pml4_virt as *mut u8, 0, PAGE_SIZE as usize); }
        let current_cr3 = Self::read_cr3();
        let current_pml4_virt = vmm::phys_to_virt(current_cr3 & !0xFFF);
        unsafe {
            let src = current_pml4_virt as *const u64;
            let dst = pml4_virt as *mut u64;
            for i in 256..512 {
                let entry = src.add(i).read();
                dst.add(i).write(entry);
            }
        }
        Ok(Self { pml4_phys, allocated_frames: alloc::vec![pml4_phys], is_kernel: false })
    }

    #[cfg(target_arch = "aarch64")]
    pub fn new() -> Result<Self, &'static str> {
        Ok(Self { ttbr0_phys: 0, allocated_frames: Vec::new(), is_kernel: false })
    }

    #[cfg(target_arch = "x86_64")]
    pub fn kernel() -> Self {
        Self {
            pml4_phys: Self::read_cr3() & !0xFFF,
            allocated_frames: Vec::new(),
            is_kernel: true,
        }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn kernel() -> Self {
        Self {
            ttbr0_phys: 0,
            allocated_frames: Vec::new(),
            is_kernel: true,
        }
    }

    #[inline]
    pub fn cr3(&self) -> u64 {
        #[cfg(target_arch = "x86_64")]
        { self.pml4_phys }
        #[cfg(target_arch = "aarch64")]
        { self.ttbr0_phys }
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    pub fn read_cr3() -> u64 {
        let cr3: u64;
        unsafe { asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack)); }
        cr3
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    pub unsafe fn activate(&self) {
        let current = Self::read_cr3() & !0xFFF;
        if current != self.pml4_phys {
            asm!("mov cr3, {}", in(reg) self.pml4_phys, options(nostack));
        }
    }
    
    #[cfg(target_arch = "aarch64")]
    pub unsafe fn activate(&self) {}
    
    /// Mapea una página virtual a una física
    pub fn map_page(&mut self, virt: u64, phys: u64, flags: u64) -> Result<(), &'static str> {
        #[cfg(target_arch = "x86_64")]
        {
            let pml4_idx = (virt >> 39) & 0x1FF;
            let pdpt_idx = (virt >> 30) & 0x1FF;
            let pd_idx = (virt >> 21) & 0x1FF;
            let pt_idx = (virt >> 12) & 0x1FF;
            
            let hhdm = vmm::hhdm_offset();
            
            unsafe {
                let pml4 = (hhdm + self.pml4_phys) as *mut u64;
                let pdpt_phys = self.ensure_table(pml4, pml4_idx as usize, flags)?;
                let pdpt = (hhdm + pdpt_phys) as *mut u64;
                let pd_phys = self.ensure_table(pdpt, pdpt_idx as usize, flags)?;
                let pd = (hhdm + pd_phys) as *mut u64;
                let pt_phys = self.ensure_table(pd, pd_idx as usize, flags)?;
                let pt = (hhdm + pt_phys) as *mut u64;
                let pte = pt.add(pt_idx as usize);
                *pte = (phys & !0xFFF) | flags;
                
                asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
            }
        }
        Ok(())
    }
    
    /// Asegura que existe una tabla en el índice dado
    unsafe fn ensure_table(&mut self, table: *mut u64, idx: usize, flags: u64) -> Result<u64, &'static str> {
        let entry = table.add(idx);
        
        if *entry & flags::PRESENT == 0 {
            let new_table = pmm::alloc_frame().ok_or("No memory for page table")?;
            self.allocated_frames.push(new_table);
            
            let new_table_virt = vmm::phys_to_virt(new_table);
            core::ptr::write_bytes(new_table_virt as *mut u8, 0, PAGE_SIZE as usize);
            
            // USER bit debe propagarse en tablas intermedias
            let table_flags = if flags & flags::USER != 0 {
                flags::PRESENT | flags::WRITABLE | flags::USER
            } else {
                flags::PRESENT | flags::WRITABLE
            };
            
            *entry = new_table | table_flags;
        }
        
        Ok(*entry & !0xFFF)
    }
    
    /// Mapea un rango de páginas allocando frames físicos
    pub fn map_range(&mut self, virt_start: u64, size: u64, flags: u64) -> Result<(), &'static str> {
        let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        
        for i in 0..pages {
            let virt = virt_start + i * PAGE_SIZE;
            let phys = pmm::alloc_frame().ok_or("No memory for user page")?;
            self.allocated_frames.push(phys);
            
            // Limpiar la página
            let phys_virt = vmm::phys_to_virt(phys);
            unsafe {
                core::ptr::write_bytes(phys_virt as *mut u8, 0, PAGE_SIZE as usize);
            }
            
            self.map_page(virt, phys, flags)?;
        }
        
        Ok(())
    }
    
    /// Mapea un rango usando frames físicos específicos (para código)
    pub fn map_range_phys(&mut self, virt_start: u64, phys_start: u64, size: u64, flags: u64) -> Result<(), &'static str> {
        let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        
        for i in 0..pages {
            let virt = virt_start + i * PAGE_SIZE;
            let phys = phys_start + i * PAGE_SIZE;
            self.map_page(virt, phys, flags)?;
        }
        
        Ok(())
    }
    
    /// Copia datos a una dirección virtual en este espacio
    pub fn write_to(&self, virt: u64, data: &[u8]) -> Result<(), &'static str> {
        let hhdm = vmm::hhdm_offset();
        let mut offset = 0usize;
        
        while offset < data.len() {
            let page_virt = (virt + offset as u64) & !0xFFF;
            let page_offset = ((virt + offset as u64) & 0xFFF) as usize;
            let bytes_in_page = core::cmp::min(
                PAGE_SIZE as usize - page_offset,
                data.len() - offset
            );
            
            let phys = self.translate(page_virt).ok_or("Page not mapped")?;
            let dest = (hhdm + phys + page_offset as u64) as *mut u8;
            
            unsafe {
                core::ptr::copy_nonoverlapping(
                    data.as_ptr().add(offset),
                    dest,
                    bytes_in_page
                );
            }
            
            offset += bytes_in_page;
        }
        
        Ok(())
    }
    
    /// Traduce una dirección virtual a física
    pub fn translate(&self, virt: u64) -> Option<u64> {
        #[cfg(target_arch = "x86_64")]
        {
            let pml4_idx = (virt >> 39) & 0x1FF;
            let pdpt_idx = (virt >> 30) & 0x1FF;
            let pd_idx = (virt >> 21) & 0x1FF;
            let pt_idx = (virt >> 12) & 0x1FF;
            
            let hhdm = vmm::hhdm_offset();
            
            unsafe {
                let pml4 = (hhdm + self.pml4_phys) as *const u64;
                let pml4e = pml4.add(pml4_idx as usize).read();
                if pml4e & flags::PRESENT == 0 { return None; }
                
                let pdpt = (hhdm + (pml4e & !0xFFF)) as *const u64;
                let pdpte = pdpt.add(pdpt_idx as usize).read();
                if pdpte & flags::PRESENT == 0 { return None; }
                
                let pd = (hhdm + (pdpte & !0xFFF)) as *const u64;
                let pde = pd.add(pd_idx as usize).read();
                if pde & flags::PRESENT == 0 { return None; }
                
                let pt = (hhdm + (pde & !0xFFF)) as *const u64;
                let pte = pt.add(pt_idx as usize).read();
                if pte & flags::PRESENT == 0 { return None; }
                
                return Some(pte & !0xFFF);
            }
        }
        #[cfg(target_arch = "aarch64")]
        { None }
    }
    
    /// Prepara un espacio de usuario con código y stack
    pub fn setup_user_process(&mut self, code: &[u8]) -> Result<(u64, u64), &'static str> {
        use layout::*;
        
        // Mapear código de usuario
        let code_size = ((code.len() as u64 + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;
        self.map_range(USER_CODE_BASE, code_size, flags::USER_RWX)?;
        
        // Copiar código
        self.write_to(USER_CODE_BASE, code)?;
        
        // Mapear stack de usuario (crece hacia abajo)
        let stack_bottom = USER_STACK_TOP - USER_STACK_SIZE;
        self.map_range(stack_bottom, USER_STACK_SIZE, flags::USER_RW)?;
        
        crate::serial_println!(
            "[ADDR_SPACE] User process: code={:#x}-{:#x}, stack={:#x}-{:#x}",
            USER_CODE_BASE, USER_CODE_BASE + code_size,
            stack_bottom, USER_STACK_TOP
        );
        
        // Entry point y stack top
        Ok((USER_CODE_BASE, USER_STACK_TOP - 8))
    }
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        if !self.is_kernel && !self.allocated_frames.is_empty() {
            for &frame in &self.allocated_frames {
                pmm::free_frame(frame);
            }
            crate::serial_println!("[ADDR_SPACE] Freed {} frames", self.allocated_frames.len());
        }
    }
}

// Necesario para mover AddressSpace entre tareas
unsafe impl Send for AddressSpace {}
unsafe impl Sync for AddressSpace {}