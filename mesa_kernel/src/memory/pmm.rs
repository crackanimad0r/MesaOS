// mesa_kernel/src/memory/pmm.rs

//! Physical Memory Manager (PMM)
//! 
//! Gestiona frames de memoria física de 4KB usando un bitmap.
//! Solo rastrea memoria usable, ignora regiones reservadas altas.

use spin::Mutex;
use crate::memory::PAGE_SIZE;
use crate::mesa_println;
use limine::memory_map::{Entry, EntryType};

/// Tamaño máximo del bitmap: 512 KB = 4 millones de frames = 16 GB RAM usable
const MAX_BITMAP_SIZE: usize = 512 * 1024;

/// Bitmap estático para el allocator
static mut BITMAP: [u8; MAX_BITMAP_SIZE] = [0xFF; MAX_BITMAP_SIZE];

/// Estado global del PMM
pub static PMM: Mutex<PhysicalMemoryManager> = Mutex::new(PhysicalMemoryManager::new());

pub struct PhysicalMemoryManager {
    hhdm_offset: u64,
    /// Dirección física más alta que rastreamos
    highest_usable: u64,
    total_frames: u64,
    free_frames: u64,
    last_index: u64,
    initialized: bool,
}

impl PhysicalMemoryManager {
    const fn new() -> Self {
        Self {
            hhdm_offset: 0,
            highest_usable: 0,
            total_frames: 0,
            free_frames: 0,
            last_index: 0,
            initialized: false,
        }
    }
    
    fn set_used(&mut self, frame: u64) {
        if frame >= self.total_frames {
            return;
        }
        let byte_idx = (frame / 8) as usize;
        let bit_idx = (frame % 8) as u8;
        
        if byte_idx < MAX_BITMAP_SIZE {
            unsafe {
                BITMAP[byte_idx] |= 1 << bit_idx;
            }
        }
    }
    
    fn set_free(&mut self, frame: u64) {
        if frame >= self.total_frames {
            return;
        }
        let byte_idx = (frame / 8) as usize;
        let bit_idx = (frame % 8) as u8;
        
        if byte_idx < MAX_BITMAP_SIZE {
            unsafe {
                BITMAP[byte_idx] &= !(1 << bit_idx);
            }
        }
    }
    
    fn is_free(&self, frame: u64) -> bool {
        if frame >= self.total_frames {
            return false;
        }
        let byte_idx = (frame / 8) as usize;
        let bit_idx = (frame % 8) as u8;
        
        if byte_idx < MAX_BITMAP_SIZE {
            unsafe {
                (BITMAP[byte_idx] & (1 << bit_idx)) == 0
            }
        } else {
            false
        }
    }
    
    pub fn alloc_frames_32bit(&mut self, count: usize) -> Option<u64> {
        if !self.initialized || self.free_frames < count as u64 {
            return None;
        }
        
        let mut found_count = 0;
        let mut start_frame = 0;
        
        // Limit to first 4GB (1 million frames approx)
        let limit = 1048576u64.min(self.total_frames);
        
        for frame in 0..limit {
            if self.is_free(frame) {
                if found_count == 0 {
                    start_frame = frame;
                }
                found_count += 1;
                if found_count == count {
                    for j in 0..count {
                        self.set_used(start_frame + j as u64);
                    }
                    self.free_frames -= count as u64;
                    return Some(start_frame * PAGE_SIZE);
                }
            } else {
                found_count = 0;
            }
        }
        
        None
    }
    
    pub fn alloc_frames(&mut self, count: usize) -> Option<u64> {
        if !self.initialized || self.free_frames < count as u64 {
            return None;
        }
        
        let mut found_count = 0;
        let mut start_frame = 0;
        
        for frame in 0..self.total_frames {
            if self.is_free(frame) {
                if found_count == 0 {
                    start_frame = frame;
                }
                found_count += 1;
                if found_count == count {
                    for j in 0..count {
                        self.set_used(start_frame + j as u64);
                    }
                    self.free_frames -= count as u64;
                    return Some(start_frame * PAGE_SIZE);
                }
            } else {
                found_count = 0;
            }
        }
        
        None
    }
    
    pub fn alloc_frame(&mut self) -> Option<u64> {
        self.alloc_frames(1)
    }

    pub fn free_frame(&mut self, phys_addr: u64) {
        if !self.initialized {
            return;
        }
        
        let frame = phys_addr / PAGE_SIZE;
        
        if frame < self.total_frames && !self.is_free(frame) {
            self.set_free(frame);
            self.free_frames += 1;
        }
    }
    
    pub fn free_count(&self) -> u64 {
        self.free_frames
    }
    
    pub fn total_count(&self) -> u64 {
        self.total_frames
    }
}

pub fn init(entries: &[&Entry], hhdm_offset: u64) -> Result<(), &'static str> {
    let mut pmm = PMM.lock();
    
    if pmm.initialized {
        return Err("PMM ya inicializado");
    }
    
    pmm.hhdm_offset = hhdm_offset;
    
    // Encontrar la dirección más alta de memoria USABLE (no reservada)
    let mut highest_usable: u64 = 0;
    
    for entry in entries.iter() {
        // Solo considerar regiones que podemos usar
        match entry.entry_type {
            EntryType::USABLE | EntryType::BOOTLOADER_RECLAIMABLE => {
                let end = entry.base + entry.length;
                if end > highest_usable {
                    highest_usable = end;
                }
            }
            _ => {}
        }
    }
    
    if highest_usable == 0 {
        return Err("No se encontró memoria usable");
    }
    
    pmm.highest_usable = highest_usable;
    pmm.total_frames = highest_usable / PAGE_SIZE;
    
    // Verificar que cabe en el bitmap
    let required_bytes = (pmm.total_frames as usize + 7) / 8;
    if required_bytes > MAX_BITMAP_SIZE {
        mesa_println!("       Advertencia: RAM truncada a {} GB", 
            (MAX_BITMAP_SIZE * 8) as u64 * PAGE_SIZE / 1024 / 1024 / 1024);
        pmm.total_frames = (MAX_BITMAP_SIZE * 8) as u64;
    }
    
    mesa_println!("       Highest usable:  {:#x}", highest_usable);
    mesa_println!("       Total frames:    {}", pmm.total_frames);
    
    // Inicialmente todo está usado (0xFF ya está puesto)
    // Marcar regiones usables como libres
    for entry in entries.iter() {
        if entry.entry_type == EntryType::USABLE {
            let start_frame = entry.base / PAGE_SIZE;
            let end_frame = (entry.base + entry.length) / PAGE_SIZE;
            
            // Solo marcar frames dentro de nuestro rango
            let end_frame = end_frame.min(pmm.total_frames);
            
            for frame in start_frame..end_frame {
                pmm.set_free(frame);
                pmm.free_frames += 1;
            }
        }
    }
    
    // Proteger frame 0
    if pmm.is_free(0) {
        pmm.set_used(0);
        pmm.free_frames -= 1;
    }
    
    pmm.initialized = true;
    
    mesa_println!("       Frames libres:   {} ({} MB)", 
        pmm.free_frames, 
        (pmm.free_frames * PAGE_SIZE) / 1024 / 1024
    );
    
    Ok(())
}

pub fn alloc_frame() -> Option<u64> {
    PMM.lock().alloc_frame()
}

pub fn alloc_frames_32bit(count: usize) -> Option<u64> {
    PMM.lock().alloc_frames_32bit(count)
}

pub fn alloc_frames(count: usize) -> Option<u64> {
    PMM.lock().alloc_frames(count)
}

pub fn free_frame(phys_addr: u64) {
    PMM.lock().free_frame(phys_addr)
}

pub fn stats() -> (u64, u64) {
    let pmm = PMM.lock();
    (pmm.free_count(), pmm.total_count())
}