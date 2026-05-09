use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{compiler_fence, Ordering};
use spin::Mutex;
use crate::drivers::block::{BlockDevice, SECTOR_SIZE};
use crate::memory::{pmm, vmm};
use crate::pci;

const NVME_CLASS: u8 = 0x01;
const NVME_SUBCLASS: u8 = 0x08;

// MMIO Offsets
const CAP: usize = 0x0000;  // Controller Capabilities
const CC: usize = 0x0014;   // Controller Configuration
const CSTS: usize = 0x001C; // Controller Status
const AQA: usize = 0x0024;  // Admin Queue Attributes
const ASQ: usize = 0x0028;  // Admin Submission Queue Base Address
const ACQ: usize = 0x0030;  // Admin Completion Queue Base Address
const SQ0_TDBL: usize = 0x1000; // Submission Queue 0 Tail Doorbell
const CQ0_HDBL: usize = 0x1004; // Completion Queue 0 Head Doorbell

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct SqEntry {
    pub cdw0: u32,
    pub nsid: u32,
    pub rsvd2: u64,
    pub mptr: u64,
    pub prp1: u64,
    pub prp2: u64,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct CqEntry {
    pub cdw0: u32,
    pub rsvd1: u32,
    pub sq_head: u16,
    pub sq_id: u16,
    pub cid: u16,
    pub status: u16,
}

pub struct NvmeQueue {
    pub sq_virt: *mut SqEntry,
    pub sq_phys: u64,
    pub cq_virt: *mut CqEntry,
    pub cq_phys: u64,
    pub sq_tail: u16,
    pub cq_head: u16,
    pub phase: u16,
    pub size: u16,
    pub qid: u16,
    pub db_stride: usize,
    pub bar_virt: usize,
}

unsafe impl Send for NvmeQueue {}
unsafe impl Sync for NvmeQueue {}

impl NvmeQueue {
    pub fn new(bar_virt: usize, qid: u16, size: u16, db_stride: usize) -> Option<Self> {
        let (sq_phys, sq_virt) = match pmm::alloc_frames(1) {
            Some(p) => (p, vmm::phys_to_virt(p) as *mut SqEntry),
            None => return None,
        };
        let (cq_phys, cq_virt) = match pmm::alloc_frames(1) {
            Some(p) => (p, vmm::phys_to_virt(p) as *mut CqEntry),
            None => return None,
        };

        unsafe {
            core::ptr::write_bytes(sq_virt as *mut u8, 0, 4096);
            core::ptr::write_bytes(cq_virt as *mut u8, 0, 4096);
        }

        Some(Self {
            sq_virt,
            sq_phys,
            cq_virt,
            cq_phys,
            sq_tail: 0,
            cq_head: 0,
            phase: 1,
            size,
            qid,
            db_stride,
            bar_virt,
        })
    }

    pub fn submit(&mut self, mut entry: SqEntry) -> u16 {
        let cid = self.sq_tail;
        entry.cdw0 = (entry.cdw0 & !0xFFFF_0000) | ((cid as u32) << 16);
        
        unsafe {
            core::ptr::write_volatile(self.sq_virt.add(self.sq_tail as usize), entry);
        }
        
        self.sq_tail += 1;
        if self.sq_tail >= self.size {
            self.sq_tail = 0;
        }

        compiler_fence(Ordering::SeqCst);

        // Ring SQ Tail Doorbell
        let offset = 0x1000 + ((self.qid as usize * 2) * (4 << self.db_stride));
        unsafe {
            core::ptr::write_volatile((self.bar_virt + offset) as *mut u32, self.sq_tail as u32);
        }

        cid
    }

    pub fn poll(&mut self, cid: u16) -> Result<CqEntry, &'static str> {
        let mut timeout = 0;
        loop {
            let entry = unsafe { core::ptr::read_volatile(self.cq_virt.add(self.cq_head as usize)) };
            let p = (entry.status & 1) != 0;
            
            if p == (self.phase != 0) {
                compiler_fence(Ordering::Acquire);
                
                self.cq_head += 1;
                if self.cq_head >= self.size {
                    self.cq_head = 0;
                    self.phase ^= 1;
                }

                // Ring CQ Head Doorbell
                let offset = 0x1000 + (((self.qid as usize * 2) + 1) * (4 << self.db_stride));
                unsafe {
                    core::ptr::write_volatile((self.bar_virt + offset) as *mut u32, self.cq_head as u32);
                }

                if entry.cid == cid {
                    let st = entry.status >> 1;
                    if st == 0 {
                        return Ok(entry);
                    } else {
                        crate::serial_println!("[NVMe] Error en cola {}: status={:#x}", self.qid, st);
                        return Err("NVMe command failed");
                    }
                }
            }

            timeout += 1;
            if timeout > 100_000_000 {
                return Err("NVMe CQ poll timeout");
            }
            core::hint::spin_loop();
        }
    }
}

pub struct NvmeDriver {
    pub bar_virt: usize,
    pub db_stride: usize,
    pub admin_queue: Option<NvmeQueue>,
    pub io_queue: Option<NvmeQueue>,
    pub ns_size_sectors: u64,
    pub ns_id: u32,
    pub max_prp_pages: usize,
}

unsafe impl Send for NvmeDriver {}
unsafe impl Sync for NvmeDriver {}

pub static NVME: Mutex<Option<NvmeDriver>> = Mutex::new(None);

impl NvmeDriver {
    fn read_reg32(&self, offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((self.bar_virt + offset) as *const u32) }
    }
    
    fn write_reg32(&mut self, offset: usize, value: u32) {
        unsafe { core::ptr::write_volatile((self.bar_virt + offset) as *mut u32, value) }
    }

    fn read_reg64(&self, offset: usize) -> u64 {
        unsafe { core::ptr::read_volatile((self.bar_virt + offset) as *const u64) }
    }

    fn write_reg64(&mut self, offset: usize, value: u64) {
        unsafe { core::ptr::write_volatile((self.bar_virt + offset) as *mut u64, value) }
    }

    pub fn init() -> Result<(), &'static str> {
        let dev = pci::devices().into_iter().find(|d| d.class_code == NVME_CLASS && d.subclass == NVME_SUBCLASS);
        let dev = match dev {
            Some(d) => d,
            None => return Err("No NVMe controller found"),
        };

        crate::serial_println!("[NVMe] Encontrado controlador en bus {}, dev {}", dev.bus, dev.device);

        // Habilitar Bus Master y Memory Space
        let cmd = pci::pci_config_read(dev.bus, dev.device, dev.function, 0x04);
        pci::pci_config_write(dev.bus, dev.device, dev.function, 0x04, cmd | 0x06);

        // Leer BAR0
        let bar0 = pci::pci_config_read(dev.bus, dev.device, dev.function, 0x10);
        let bar0_type = bar0 & 0x07; // bits 1-2 define type
        
        let phys_base = if bar0_type == 0x04 { // 64-bit BAR
            let bar0_high = pci::pci_config_read(dev.bus, dev.device, dev.function, 0x14);
            ((bar0_high as u64) << 32) | (bar0 as u64 & !0xF)
        } else {
            bar0 as u64 & !0xF
        };
        
        let bar_virt = vmm::map_mmio(phys_base, 0x4000).unwrap_or(vmm::phys_to_virt(phys_base)) as usize;
        
        let mut driver = NvmeDriver {
            bar_virt,
            db_stride: 0,
            admin_queue: None,
            io_queue: None,
            ns_size_sectors: 0,
            ns_id: 1,
            max_prp_pages: 1,
        };

        let cap = driver.read_reg64(CAP);
        driver.db_stride = ((cap >> 32) & 0xF) as usize;
        let mpsmin = ((cap >> 48) & 0xF) as u32;

        // Deshabilitar controlador
        let mut cc = driver.read_reg32(CC);
        cc &= !1; // EN = 0
        driver.write_reg32(CC, cc);

        // Esperar a CSTS.RDY == 0
        let mut timeout = 0;
        while (driver.read_reg32(CSTS) & 1) != 0 {
            timeout += 1;
            if timeout > 10_000_000 { return Err("Timeout waiting for CSTS.RDY == 0"); }
            core::hint::spin_loop();
        }

        // Crear Admin Queue
        let mut aq = NvmeQueue::new(bar_virt, 0, 64, driver.db_stride).ok_or("Failed to alloc Admin Queue")?;
        
        driver.write_reg32(AQA, ((64 - 1) << 16) | (64 - 1));
        driver.write_reg64(ASQ, aq.sq_phys);
        driver.write_reg64(ACQ, aq.cq_phys);

        // Configurar CC
        cc = driver.read_reg32(CC);
        cc = (cc & 0xFF00000F) | (6 << 16) | (4 << 20); // IOSQES=6(64 bytes), IOCQES=4(16 bytes)
        cc |= (mpsmin << 7); // Page size
        cc |= 1; // Habilitar
        driver.write_reg32(CC, cc);

        // Esperar CSTS.RDY == 1
        timeout = 0;
        while (driver.read_reg32(CSTS) & 1) == 0 {
            timeout += 1;
            if timeout > 10_000_000 { return Err("Timeout waiting for CSTS.RDY == 1"); }
            core::hint::spin_loop();
        }

        driver.admin_queue = Some(aq);

        // Identificar Controlador
        let (id_phys, id_virt) = match pmm::alloc_frames(1) {
            Some(p) => (p, vmm::phys_to_virt(p) as *mut u8),
            None => return Err("Failed to alloc Identify buffer"),
        };

        let mut id_cmd = SqEntry::default();
        id_cmd.cdw0 = 0x06; // Identify
        id_cmd.prp1 = id_phys;
        id_cmd.cdw10 = 1; // Identify Controller
        
        let cid = driver.admin_queue.as_mut().unwrap().submit(id_cmd);
        driver.admin_queue.as_mut().unwrap().poll(cid)?;

        // Identificar Namespace 1
        let mut ns_cmd = SqEntry::default();
        ns_cmd.cdw0 = 0x06;
        ns_cmd.prp1 = id_phys;
        ns_cmd.nsid = 1;
        ns_cmd.cdw10 = 0; // Identify Namespace

        let cid = driver.admin_queue.as_mut().unwrap().submit(ns_cmd);
        driver.admin_queue.as_mut().unwrap().poll(cid)?;

        let nsze = unsafe { core::ptr::read_unaligned(id_virt as *const u64) };
        driver.ns_size_sectors = nsze;
        
        crate::serial_println!("[NVMe] Namespace 1: {} sectors", nsze);

        // Crear I/O CQ
        let io_cq = NvmeQueue::new(bar_virt, 1, 64, driver.db_stride).ok_or("Failed to alloc IO Queue")?;
        
        let mut create_iocq = SqEntry::default();
        create_iocq.cdw0 = 0x05; // Create I/O CQ
        create_iocq.prp1 = io_cq.cq_phys;
        create_iocq.cdw10 = ((64 - 1) << 16) | 1; // Size, QID=1
        create_iocq.cdw11 = 1; // Contiguous, Interrupt Enable

        let cid = driver.admin_queue.as_mut().unwrap().submit(create_iocq);
        driver.admin_queue.as_mut().unwrap().poll(cid)?;

        // Crear I/O SQ
        let mut create_iosq = SqEntry::default();
        create_iosq.cdw0 = 0x01; // Create I/O SQ
        create_iosq.prp1 = io_cq.sq_phys;
        create_iosq.cdw10 = ((64 - 1) << 16) | 1; // Size, QID=1
        create_iosq.cdw11 = (1 << 16) | 1; // CQID=1, Contiguous

        let cid = driver.admin_queue.as_mut().unwrap().submit(create_iosq);
        driver.admin_queue.as_mut().unwrap().poll(cid)?;

        driver.io_queue = Some(io_cq);

        pmm::free_frame(id_phys);

        *NVME.lock() = Some(driver);
        Ok(())
    }

    fn do_io(&mut self, lba: u64, buf_phys: u64, count: u16, is_write: bool) -> Result<(), &'static str> {
        let io_q = self.io_queue.as_mut().ok_or("IO Queue not initialized")?;

        let mut cmd = SqEntry::default();
        cmd.cdw0 = if is_write { 0x01 } else { 0x02 }; // Write = 0x01, Read = 0x02
        cmd.nsid = self.ns_id;
        cmd.prp1 = buf_phys;
        cmd.cdw10 = (lba & 0xFFFFFFFF) as u32;
        cmd.cdw11 = (lba >> 32) as u32;
        cmd.cdw12 = (count - 1) as u32;

        let cid = io_q.submit(cmd);
        io_q.poll(cid)?;
        Ok(())
    }
}

pub struct NvmeBlockDevice;

impl BlockDevice for NvmeBlockDevice {
    fn read(&self, lba: u64, count: u16, buffer: &mut [u8]) -> Result<(), &'static str> {
        let mut drv = NVME.lock();
        let driver = drv.as_mut().ok_or("NVMe no inicializado")?;
        
        let expected_len = count as usize * SECTOR_SIZE;
        if buffer.len() < expected_len {
            return Err("Buffer too small");
        }

        let pages_needed = (expected_len + 4095) / 4096;
        if pages_needed > 1 {
            // Simplificación: Leer por partes de 1 sector para no requerir listas PRP complejas
            for i in 0..count {
                let (phys, virt) = match pmm::alloc_frames(1) {
                    Some(p) => (p, vmm::phys_to_virt(p) as *mut u8),
                    None => return Err("Out of memory for DMA"),
                };
                
                let res = driver.do_io(lba + i as u64, phys, 1, false);
                if res.is_ok() {
                    unsafe {
                        core::ptr::copy_nonoverlapping(virt, buffer.as_mut_ptr().add(i as usize * SECTOR_SIZE), SECTOR_SIZE);
                    }
                }
                pmm::free_frame(phys);
                res?;
            }
            Ok(())
        } else {
            let (phys, virt) = match pmm::alloc_frames(1) {
                Some(p) => (p, vmm::phys_to_virt(p) as *mut u8),
                None => return Err("Out of memory for DMA"),
            };

            let res = driver.do_io(lba, phys, count, false);
            if res.is_ok() {
                unsafe {
                    core::ptr::copy_nonoverlapping(virt, buffer.as_mut_ptr(), expected_len);
                }
            }
            pmm::free_frame(phys);
            res
        }
    }

    fn write(&self, lba: u64, count: u16, buffer: &[u8]) -> Result<(), &'static str> {
        let mut drv = NVME.lock();
        let driver = drv.as_mut().ok_or("NVMe no inicializado")?;
        
        let expected_len = count as usize * SECTOR_SIZE;
        if buffer.len() < expected_len {
            return Err("Buffer too small");
        }

        let pages_needed = (expected_len + 4095) / 4096;
        if pages_needed > 1 {
            for i in 0..count {
                let (phys, virt) = match pmm::alloc_frames(1) {
                    Some(p) => (p, vmm::phys_to_virt(p) as *mut u8),
                    None => return Err("Out of memory for DMA"),
                };
                
                unsafe {
                    core::ptr::copy_nonoverlapping(buffer.as_ptr().add(i as usize * SECTOR_SIZE), virt, SECTOR_SIZE);
                }
                let res = driver.do_io(lba + i as u64, phys, 1, true);
                pmm::free_frame(phys);
                res?;
            }
            Ok(())
        } else {
            let (phys, virt) = match pmm::alloc_frames(1) {
                Some(p) => (p, vmm::phys_to_virt(p) as *mut u8),
                None => return Err("Out of memory for DMA"),
            };

            unsafe {
                core::ptr::copy_nonoverlapping(buffer.as_ptr(), virt, expected_len);
            }

            let res = driver.do_io(lba, phys, count, true);
            pmm::free_frame(phys);
            res
        }
    }

    fn capacity(&self) -> u64 {
        let drv = NVME.lock();
        if let Some(ref d) = *drv {
            d.ns_size_sectors
        } else {
            0
        }
    }
}

pub fn init() -> Result<(), &'static str> {
    NvmeDriver::init()
}
