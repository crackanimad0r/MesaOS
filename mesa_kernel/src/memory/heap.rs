// mesa_kernel/src/memory/heap.rs

//! Heap Allocator para Mesa OS
//! 
//! Implementa GlobalAlloc para permitir el uso de Box, Vec, etc.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use spin::Mutex;
use crate::memory::{pmm, phys_to_virt, PAGE_SIZE};

/// Tamaño inicial del heap: 1 MB
const INITIAL_HEAP_SIZE: usize = 1024 * 1024;

/// Allocator global del kernel
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Linked list allocator simple
pub struct LockedHeap {
    inner: Mutex<LinkedListAllocator>,
}

impl LockedHeap {
    const fn empty() -> Self {
        Self {
            inner: Mutex::new(LinkedListAllocator::empty()),
        }
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner.lock().alloc(layout)
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.lock().dealloc(ptr, layout)
    }
}

/// Nodo de la lista enlazada de bloques libres
struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        Self { size, next: None }
    }
    
    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }
    
    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

/// Allocator de lista enlazada
pub struct LinkedListAllocator {
    head: ListNode,
    initialized: bool,
}

impl LinkedListAllocator {
    const fn empty() -> Self {
        Self {
            head: ListNode::new(0),
            initialized: false,
        }
    }
    
    /// Inicializa el allocator con una región de memoria
    unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
        self.initialized = true;
    }
    
    /// Añade una región libre a la lista
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // Asegurar alineación
        let aligned_addr = align_up(addr, core::mem::align_of::<ListNode>());
        let aligned_size = size - (aligned_addr - addr);
        
        if aligned_size < core::mem::size_of::<ListNode>() {
            return; // Región muy pequeña
        }
        
        // Crear nuevo nodo
        let node = aligned_addr as *mut ListNode;
        node.write(ListNode::new(aligned_size));
        
        // Insertar al inicio de la lista
        (*node).next = self.head.next.take();
        self.head.next = Some(&mut *node);
    }
    
    /// Busca una región libre que satisfaga el layout
    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        let mut current = &mut self.head;
        
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(region, size, align) {
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            }
            current = current.next.as_mut().unwrap();
        }
        
        None
    }
    
    /// Intenta alocar desde una región específica
    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;
        
        if alloc_end > region.end_addr() {
            return Err(()); // Región muy pequeña
        }
        
        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < core::mem::size_of::<ListNode>() {
            return Err(()); // El resto es muy pequeño para un nodo
        }
        
        Ok(alloc_start)
    }
    
    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        if !self.initialized {
            return null_mut();
        }
        
        let (size, align) = Self::size_align(layout);
        
        if let Some((region, alloc_start)) = self.find_region(size, align) {
            let alloc_end = alloc_start + size;
            let excess_size = region.end_addr() - alloc_end;
            
            if excess_size > 0 {
                unsafe {
                    self.add_free_region(alloc_end, excess_size);
                }
            }
            
            alloc_start as *mut u8
        } else {
            // Intentar expandir el heap
            if let Some(ptr) = self.expand_heap(size, align) {
                ptr
            } else {
                null_mut()
            }
        }
    }
    
    fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let (size, _) = Self::size_align(layout);
        unsafe {
            self.add_free_region(ptr as usize, size);
        }
    }
    
    /// Calcula tamaño y alineación ajustados
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(core::mem::align_of::<ListNode>())
            .expect("Alignment failed")
            .pad_to_align();
        
        let size = layout.size().max(core::mem::size_of::<ListNode>());
        (size, layout.align())
    }
    
    /// Expande el heap alocando más frames
    fn expand_heap(&mut self, min_size: usize, _align: usize) -> Option<*mut u8> {
        // Calcular cuántos frames necesitamos
        let pages_needed = (min_size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;
        let pages_needed = pages_needed.max(4); // Mínimo 4 páginas (16 KB)
        
        let mut start_phys: Option<u64> = None;
        let mut current_phys = 0u64;
        let mut allocated = 0usize;
        
        // Intentar alocar frames contiguos
        for _ in 0..pages_needed {
            if let Some(frame) = pmm::alloc_frame() {
                if start_phys.is_none() {
                    start_phys = Some(frame);
                    current_phys = frame;
                } else if frame == current_phys + PAGE_SIZE {
                    current_phys = frame;
                } else {
                    // No son contiguos, liberar y fallar
                    // (simplificación: en producción manejaríamos esto mejor)
                    break;
                }
                allocated += 1;
            } else {
                break;
            }
        }
        
        if allocated >= (min_size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize {
            if let Some(phys) = start_phys {
                let virt = phys_to_virt(phys) as usize;
                let total_size = allocated * PAGE_SIZE as usize;
                
                unsafe {
                    self.add_free_region(virt, total_size);
                }
                
                return self.find_region(min_size, core::mem::align_of::<ListNode>())
                    .map(|(_, addr)| addr as *mut u8);
            }
        }
        
        None
    }
}

/// Alinea hacia arriba
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Inicializa el heap del kernel
pub fn init() -> Result<(), &'static str> {
    // Alocar páginas para el heap inicial
    let pages_needed = INITIAL_HEAP_SIZE / PAGE_SIZE as usize;
    let mut heap_start: Option<u64> = None;
    let mut last_frame = 0u64;
    let mut allocated = 0usize;
    
    for i in 0..pages_needed {
        if let Some(frame) = pmm::alloc_frame() {
            if i == 0 {
                heap_start = Some(frame);
                last_frame = frame;
                allocated += 1;
            } else if frame == last_frame + PAGE_SIZE {
                last_frame = frame;
                allocated += 1;
            } else {
                // Frames no contiguos - usar lo que tenemos
                break;
            }
        } else {
            break;
        }
    }
    
    if allocated == 0 {
        return Err("No se pudo alocar memoria para el heap");
    }
    
    let heap_start = heap_start.unwrap();
    let heap_virt = phys_to_virt(heap_start) as usize;
    let heap_size = allocated * PAGE_SIZE as usize;
    
    unsafe {
        ALLOCATOR.inner.lock().init(heap_virt, heap_size);
    }
    
    crate::mesa_println!("       Heap inicial:    {} KB @ {:#x}", 
        heap_size / 1024, 
        heap_virt
    );
    
    Ok(())
}

/// Handler para errores de alocación
#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    panic!("Fallo de alocación: {:?}", layout);
}