// mesa_kernel/src/drivers/net/virtio_net.rs
use crate::pci::PciDevice;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering, fence};
use x86_64::instructions::port::Port;

const VIRTIO_VENDOR_ID: u16 = 0x1AF4;
const VIRTIO_NET_DEVICE_ID: u16 = 0x1000;
const HHDM_OFFSET: u64 = 0xffff800000000000;

static VIRTIO_NET_ACTIVE: AtomicBool = AtomicBool::new(false);
static mut DRIVER: Option<VirtioNetDriver> = None;

#[repr(C, packed)]
struct VirtDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

// Estructuras genéricas para cualquier tamaño de cola
struct VirtQueue {
    size: u16,
    index: u16,
    last_used: u16,
    desc_ptr_virt: *mut VirtDesc,
    avail_ptr_virt: *mut u16, // Puntero al inicio del ring (flags)
    used_ptr_virt: *mut u16,  // Puntero al inicio del ring (flags)
}

struct VirtioNetDriver {
    io_base: u32,
    mac: [u8; 6],
    rx_queue: VirtQueue,
    tx_queue: VirtQueue,
}

pub fn is_active() -> bool {
    VIRTIO_NET_ACTIVE.load(Ordering::SeqCst)
}

pub fn get_mac() -> Option<[u8; 6]> {
    if !is_active() { return None; }
    unsafe { DRIVER.as_ref().map(|d| d.mac) }
}

pub fn init() -> Result<(), &'static str> {
    for dev in crate::pci::devices() {
        if dev.vendor_id == VIRTIO_VENDOR_ID && dev.device_id == VIRTIO_NET_DEVICE_ID {
            crate::serial_println!("[VIRTIO-NET] Inicializando con tamaño nativo...");
            
            crate::pci::pci_enable_io_space(dev.bus, dev.device, dev.function);
            crate::pci::pci_enable_bus_mastering(dev.bus, dev.device, dev.function);

            let (bar0, _) = crate::pci::pci_read_bar(dev.bus, dev.device, dev.function, 0)
                .ok_or("BAR0 no accesible")?;
            let io_base = (bar0 & !0x3) as u32;

            unsafe {
                outb(io_base + 18, 0); // Reset
                outb(io_base + 18, 1 | 2); // ACK + Driver

                let features = inl(io_base + 0);
                outl(io_base + 4, features); 

                let mut mac = [0u8; 6];
                for i in 0..6 { mac[i] = inb(io_base + 20 + i as u32); }
                
                let rx_q = setup_queue(io_base, 0, true)?;
                let tx_q = setup_queue(io_base, 1, false)?;

                outb(io_base + 18, 1 | 2 | 4); // Driver OK

                DRIVER = Some(VirtioNetDriver { io_base, mac, rx_queue: rx_q, tx_queue: tx_q });
                VIRTIO_NET_ACTIVE.store(true, Ordering::SeqCst);
                crate::serial_println!("[VIRTIO-NET] Driver listo y operativo");
                return Ok(());
            }
        }
    }
    Err("Hardware no encontrado")
}

unsafe fn setup_queue(io_base: u32, qidx: u16, is_rx: bool) -> Result<VirtQueue, &'static str> {
    outw(io_base + 14, qidx);
    let qsize = inw(io_base + 12);
    if qsize == 0 || qsize > 512 { return Err("Tamaño de cola invalido"); }

    // VirtIO Legacy: Descriptors + Avail ... (Padding) ... Used
    // Necesitamos 3 páginas para estar 100% seguros de la alineación del Used Ring a 4096
    let phys_page_start = crate::memory::pmm::alloc_frames_32bit(3).ok_or("PMM DMA fail")?;
    let virt_page_start = phys_page_start + HHDM_OFFSET;
    
    core::ptr::write_bytes(virt_page_start as *mut u8, 0, 4096 * 3);

    let desc_ptr_virt = virt_page_start as *mut VirtDesc;
    let avail_ptr_virt = (virt_page_start + (qsize as u64 * 16)) as *mut u16;
    let used_ptr_virt = (virt_page_start + 8192) as *mut u16; // Página 2 (offset 8192) para Used

    if is_rx {
        for i in 0..qsize {
            let phys_buf = crate::memory::pmm::alloc_frame().ok_or("RX buf fail")?;
            let desc = &mut *desc_ptr_virt.add(i as usize);
            desc.addr = phys_buf;
            desc.len = 1536;
            desc.flags = 2; // Writeable
            
            // Escribir en el ring de disponibles (offset 2 es el array ring)
            core::ptr::write_volatile(avail_ptr_virt.add(2 + i as usize), i);
        }
        // Escribir el índice de disponibles (offset 1 es idx)
        core::ptr::write_volatile(avail_ptr_virt.add(1), qsize);
    }

    outl(io_base + 8, (phys_page_start >> 12) as u32);

    Ok(VirtQueue { 
        size: qsize, 
        index: 0, 
        last_used: 0, 
        desc_ptr_virt, 
        avail_ptr_virt, 
        used_ptr_virt 
    })
}

pub fn send_packet(packet: &[u8]) -> Result<(), &'static str> {
    unsafe {
        let drv = DRIVER.as_mut().ok_or("Offline")?;
        let q = &mut drv.tx_queue;
        let head = (q.index % q.size) as usize;

        let phys_tx_buf = crate::memory::pmm::alloc_frame().ok_or("TX fail")?;
        let virt_tx_buf = phys_tx_buf + HHDM_OFFSET;
        
        core::ptr::write_bytes(virt_tx_buf as *mut u8, 0, 10); 
        core::ptr::copy_nonoverlapping(packet.as_ptr(), (virt_tx_buf + 10) as *mut u8, packet.len());

        let desc = &mut *q.desc_ptr_virt.add(head);
        desc.addr = phys_tx_buf;
        desc.len = (packet.len() + 10) as u32;
        desc.flags = 0;

        fence(Ordering::SeqCst);

        // Avail Ring: [flags(u16), idx(u16), ring(u16 * size)]
        core::ptr::write_volatile(q.avail_ptr_virt.add(2 + head), head as u16);
        
        fence(Ordering::SeqCst);
        
        let old_idx = core::ptr::read_volatile(q.avail_ptr_virt.add(1));
        core::ptr::write_volatile(q.avail_ptr_virt.add(1), old_idx.wrapping_add(1));
        
        q.index = q.index.wrapping_add(1);

        // crate::serial_println!("[VIRTIO-NET] TX: Enviando paquete de {} bytes", packet.len());
        outw(drv.io_base + 16, 1); 
        Ok(())
    }
}

pub fn poll_rx() -> Option<Vec<u8>> {
    unsafe {
        let drv = DRIVER.as_mut()?;
        let q = &mut drv.rx_queue;
        
        let _ = inb(drv.io_base + 19);
        fence(Ordering::SeqCst);
        
        // Used Ring: [flags(u16), idx(u16), ring([id(u32), len(u32)] * size)]
        let used_idx = core::ptr::read_volatile(q.used_ptr_virt.add(1));
        if q.last_used == used_idx { return None; }

        let ring_idx = (q.last_used % q.size) as usize;
        // Cada elemento del ring usado son 2 u32 (id y len). En u16 son 4 slots.
        let base_ptr = q.used_ptr_virt.add(2 + ring_idx * 4);
        let elem_id = core::ptr::read_volatile(base_ptr as *const u32) as u16;
        let elem_len = core::ptr::read_volatile((base_ptr as *const u32).add(1)) as usize;
        
        let desc = &*q.desc_ptr_virt.add(elem_id as usize);
        
        let mut result = None;
        if elem_len > 10 {
            let data_len = elem_len - 10;
            let mut pkt = Vec::with_capacity(data_len);
            let virt_src = (desc.addr + HHDM_OFFSET + 10) as *const u8;
            for i in 0..data_len {
                pkt.push(core::ptr::read_volatile(virt_src.add(i)));
            }
            // crate::serial_println!("[VIRTIO-NET] RX: ¡Paquete recibido! ({} bytes)", data_len);
            result = Some(pkt);
        }

        // Reciclar
        let cur_avail_idx = core::ptr::read_volatile(q.avail_ptr_virt.add(1));
        core::ptr::write_volatile(q.avail_ptr_virt.add(2 + (cur_avail_idx % q.size) as usize), elem_id);
        
        fence(Ordering::SeqCst);
        core::ptr::write_volatile(q.avail_ptr_virt.add(1), cur_avail_idx.wrapping_add(1));
        
        q.last_used = q.last_used.wrapping_add(1);
        outw(drv.io_base + 16, 0); 
        
        result
    }
}

unsafe fn outb(p: u32, v: u8) { Port::new(p as u16).write(v); }
unsafe fn outw(p: u32, v: u16) { Port::new(p as u16).write(v); }
unsafe fn outl(p: u32, v: u32) { Port::new(p as u16).write(v); }
unsafe fn inb(p: u32) -> u8 { Port::new(p as u16).read() }
unsafe fn inw(p: u32) -> u16 { Port::new(p as u16).read() }
unsafe fn inl(p: u32) -> u32 { Port::new(p as u16).read() }
