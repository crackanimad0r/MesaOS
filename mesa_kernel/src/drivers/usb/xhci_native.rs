use crate::memory::xhci_access::{MmioPtr, PmmXhciAdapter, XhciMapper, XhciMemory};
use crate::pci::PciDevice;
use alloc::vec::Vec;
use core::sync::atomic::{compiler_fence, Ordering};
use xhci::Registers;

pub static mut XHCI_BAR0: Option<u64> = None;

// ---------------------------------------------------------------------------
// Sistema de logging dual (pantalla + archivo) para xHCI
// ---------------------------------------------------------------------------
static mut XHCI_LOG_BUFFER: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
static mut XHCI_LOG_ENABLED: bool = false;

fn xhci_log_init() {
    unsafe {
        XHCI_LOG_BUFFER.clear();
        XHCI_LOG_ENABLED = true;
    }
}

pub fn xhci_log_flush_to_file() {
    unsafe {
        crate::mesa_println!("═══════════════════════════════════════════════════════════════");
        crate::mesa_println!("[xHCI] INICIANDO FLUSH DE LOGS A ARCHIVO");
        crate::mesa_println!("═══════════════════════════════════════════════════════════════");

        if !XHCI_LOG_ENABLED {
            crate::mesa_println!("[xHCI] ❌ ERROR: Logging no está habilitado");
            crate::mesa_println!("[xHCI] XHCI_LOG_ENABLED = {}", XHCI_LOG_ENABLED);
            return;
        }

        crate::mesa_println!("[xHCI] ✅ Logging está habilitado");
        crate::mesa_println!(
            "[xHCI] 📊 Buffer de logs tiene {} bytes",
            XHCI_LOG_BUFFER.len()
        );
        crate::mesa_println!(
            "[xHCI] 📊 Capacidad del buffer: {} bytes",
            XHCI_LOG_BUFFER.capacity()
        );

        if XHCI_LOG_BUFFER.is_empty() {
            crate::mesa_println!("[xHCI] ⚠️  ADVERTENCIA: Buffer de logs está vacío");
            crate::mesa_println!("[xHCI] Esto indica que xhci_log() no se ha llamado");
        } else {
            crate::mesa_println!("[xHCI] ✅ Buffer contiene datos");
            crate::mesa_println!("[xHCI] Primeros 100 bytes del buffer:");
            let preview_len = XHCI_LOG_BUFFER.len().min(100);
            let preview = alloc::string::String::from_utf8_lossy(&XHCI_LOG_BUFFER[..preview_len]);
            crate::mesa_println!("[xHCI] {}", preview);
        }

        // Verificar si el sistema de archivos está disponible
        crate::mesa_println!("[xHCI] 🔍 Verificando disponibilidad del sistema de archivos...");

        // Crear directorio /tmp si no existe
        crate::mesa_println!("[xHCI] 📁 Creando directorio /tmp...");
        let mkdir_result = crate::fs::mkdir("/tmp");
        crate::mesa_println!("[xHCI] 📁 mkdir /tmp result: {:?}", mkdir_result);

        // Verificar si el directorio se creó
        match crate::fs::readdir("/tmp") {
            Ok(entries) => {
                crate::mesa_println!(
                    "[xHCI] 📁 Directorio /tmp existe, contiene {} entradas",
                    entries.len()
                );
                for entry in &entries {
                    crate::mesa_println!("[xHCI] 📄 - {:?}", entry);
                }
            }
            Err(e) => {
                crate::mesa_println!("[xHCI] ❌ ERROR: No se puede leer /tmp: {:?}", e);
            }
        }

        // Escribir logs a archivo
        crate::mesa_println!("[xHCI] 💾 Escribiendo logs a /tmp/xhci_debug.txt...");
        let log_content = alloc::string::String::from_utf8_lossy(&XHCI_LOG_BUFFER);
        crate::mesa_println!(
            "[xHCI] 💾 Tamaño del contenido a escribir: {} bytes",
            log_content.len()
        );

        let write_result = crate::fs::write("/tmp/xhci_debug.txt", log_content.as_bytes());
        crate::mesa_println!("[xHCI] 💾 write result: {:?}", write_result);

        match write_result {
            Ok(_) => {
                crate::mesa_println!(
                    "[xHCI] ✅ Logs guardados exitosamente en /tmp/xhci_debug.txt"
                );

                // Verificar que el archivo se creó
                match crate::fs::stat("/tmp/xhci_debug.txt") {
                    Ok(meta) => {
                        crate::mesa_println!(
                            "[xHCI] ✅ Archivo verificado: tamaño = {} bytes",
                            meta.size
                        );
                    }
                    Err(e) => {
                        crate::mesa_println!(
                            "[xHCI] ❌ ERROR: No se puede verificar el archivo: {:?}",
                            e
                        );
                    }
                }
            }
            Err(e) => {
                crate::mesa_println!("[xHCI] ❌ ERROR: No se pudo escribir el archivo: {:?}", e);
                crate::mesa_println!("[xHCI] ❌ Tipo de error: {:?}", e);
            }
        }

        crate::mesa_println!("═══════════════════════════════════════════════════════════════");
        crate::mesa_println!("[xHCI] FLUSH DE LOGS COMPLETADO");
        crate::mesa_println!("═══════════════════════════════════════════════════════════════");
    }
}

fn xhci_log(args: core::fmt::Arguments) {
    // Imprimir en pantalla
    crate::mesa_println!("{}", args);

    // Guardar en buffer para archivo
    unsafe {
        if XHCI_LOG_ENABLED {
            use alloc::format;
            let log_line = format!("{}\n", args);
            XHCI_LOG_BUFFER.extend_from_slice(log_line.as_bytes());
        }
    }
}

// ---------------------------------------------------------------------------
// Sistema de medición de tiempo usando TSC (Time Stamp Counter)
// ---------------------------------------------------------------------------
#[cfg(target_arch = "x86_64")]
fn read_tsc() -> u64 {
    unsafe {
        let mut low: u32;
        let mut high: u32;
        core::arch::asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );
        ((high as u64) << 32) | (low as u64)
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn read_tsc() -> u64 {
    0
}

// Asumimos una frecuencia de TSC de aproximadamente 2.5 GHz (ajustable según hardware)
// Para mayor precisión, esto debería calibrarse al inicio
const TSC_MHZ: u64 = 2100; // 2.5 GHz = 2500 MHz

fn ms_to_spins(ms: u64) -> u64 {
    ms * TSC_MHZ * 1000
}

fn delay_ms(ms: u64) {
    let start = read_tsc();
    let target = ms_to_spins(ms);
    while read_tsc() - start < target {
        core::hint::spin_loop();
    }
}

// ---------------------------------------------------------------------------
// Estado global de dispositivos USB enumerados
// ---------------------------------------------------------------------------
#[derive(Copy, Clone)]
pub enum UsbPhase {
    Detected,
    SlotEnabled,
    AddressAssigned,
    DeviceDescriptorRead,
    ConfigDescriptorRead,
    EndpointsConfigured,
    SetConfigurationDone,
    Failed,
}

#[derive(Copy, Clone)]
pub struct UsbDeviceState {
    pub active: bool,
    pub slot_id: u8,
    pub port: u8,
    pub speed: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub class: u8,
    pub num_interfaces: u8,
    pub num_endpoints: u8,
    pub config_val: u8,
    pub phase: UsbPhase,
    pub transfers_ok: u32,
    pub transfers_err: u32,
    pub bulk_out_dci: u8,
    pub bulk_in_dci: u8,
    pub bulk_out_ring_phys: u64,
    pub bulk_out_ring_virt: u64,
    pub bulk_in_ring_phys: u64,
    pub bulk_in_ring_virt: u64,
    pub num_partitions: u8,
    pub is_gpt: bool,
    pub part_type: [u8; 8],
    pub part_lba_start: [u64; 8],
    pub part_lba_end: [u64; 8],
    pub part_size_mb: [u32; 8],
    pub free_start_lba: u64,
    pub free_sectors: u64,
}

impl UsbDeviceState {
    const fn empty() -> Self {
        Self {
            active: false,
            slot_id: 0,
            port: 0,
            speed: 0,
            vendor_id: 0,
            product_id: 0,
            class: 0,
            num_interfaces: 0,
            num_endpoints: 0,
            config_val: 0,
            phase: UsbPhase::Detected,
            transfers_ok: 0,
            transfers_err: 0,
            bulk_out_dci: 0,
            bulk_in_dci: 0,
            bulk_out_ring_phys: 0,
            bulk_out_ring_virt: 0,
            bulk_in_ring_phys: 0,
            bulk_in_ring_virt: 0,
            num_partitions: 0,
            is_gpt: false,
            part_type: [0; 8],
            part_lba_start: [0; 8],
            part_lba_end: [0; 8],
            part_size_mb: [0; 8],
            free_start_lba: 0,
            free_sectors: 0,
        }
    }
}

const MAX_USB_DEVICES: usize = 16;
pub static mut USB_DEVICES: [UsbDeviceState; MAX_USB_DEVICES] =
    [UsbDeviceState::empty(); MAX_USB_DEVICES];
pub static mut USB_HC_RUNNING: bool = false;
pub static mut XHCI_CONTEXT_SIZE: usize = 32;

// ---------------------------------------------------------------------------
// Estructuras BOT (Bulk-Only Transport) para Mass Storage
// ---------------------------------------------------------------------------
#[repr(C, packed)]
pub struct CommandBlockWrapper {
    pub d_cbw_signature: u32,
    pub d_cbw_tag: u32,
    pub d_cbw_data_transfer_length: u32,
    pub bm_cbw_flags: u8,
    pub b_cbw_lun: u8,
    pub b_cbw_cb_length: u8,
    pub cbw_cb: [u8; 16],
}

#[repr(C, packed)]
pub struct CommandStatusWrapper {
    pub d_csw_signature: u32,
    pub d_csw_tag: u32,
    pub d_csw_data_residue: u32,
    pub b_csw_status: u8,
}

const SCSI_READ_CAPACITY_10: u8 = 0x25;
const CBW_SIGNATURE: u32 = 0x43425355;
const CSW_SIGNATURE: u32 = 0x53425355;

// ---------------------------------------------------------------------------
// Estructuras GPT (GUID Partition Table)
// ---------------------------------------------------------------------------
#[repr(C, packed)]
pub struct GptHeader {
    pub signature: [u8; 8],
    pub revision: u32,
    pub header_size: u32,
    pub header_crc32: u32,
    pub reserved: u32,
    pub my_lba: u64,
    pub alternate_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: [u8; 16],
    pub partition_entry_lba: u64,
    pub num_partition_entries: u32,
    pub size_partition_entry: u32,
    pub partition_array_crc32: u32,
}

#[repr(C, packed)]
pub struct GptEntry {
    pub partition_type_guid: [u8; 16],
    pub unique_partition_guid: [u8; 16],
    pub starting_lba: u64,
    pub ending_lba: u64,
    pub attributes: u64,
    pub partition_name: [u16; 36],
}

// ---------------------------------------------------------------------------
// MemoryMapper
// ---------------------------------------------------------------------------
#[derive(Clone)]
pub struct MemoryMapper;

impl XhciMapper for MemoryMapper {
    unsafe fn map(&mut self, phys_base: usize, _bytes: usize) -> core::num::NonZeroUsize {
        let virt = crate::memory::vmm::phys_to_virt(phys_base as u64);
        core::num::NonZeroUsize::new(virt as usize).unwrap()
    }
    fn unmap(&mut self, _virt_base: usize, _bytes: usize) {}
}

// ---------------------------------------------------------------------------
// Command Ring (gestión de TRBs de comandos)
// ---------------------------------------------------------------------------
const RING_SIZE: usize = 64; // TRBs por anillo (incluido el Link TRB)

struct TrbRing {
    base_virt: *mut u32,
    base_phys: u64,
    enqueue: usize,
    cycle: bool,
}

impl TrbRing {
    fn new(base_phys: u64, base_virt: *mut u32) -> Self {
        unsafe { core::ptr::write_bytes(base_virt as *mut u8, 0, RING_SIZE * 16) };
        // Link TRB en la última posición → vuelve al inicio
        let link = unsafe { base_virt.add((RING_SIZE - 1) * 4) };
        unsafe {
            core::ptr::write_volatile(link, (base_phys & 0xFFFF_FFF0) as u32);
            core::ptr::write_volatile(link.add(1), (base_phys >> 32) as u32);
            core::ptr::write_volatile(link.add(2), 0u32);
            // Type=6 (Link), TC=1, Cycle=1
            core::ptr::write_volatile(link.add(3), (6u32 << 10) | (1 << 1) | 1);
        }
        Self {
            base_virt,
            base_phys,
            enqueue: 0,
            cycle: true,
        }
    }

    #[inline(never)]
    fn enqueue_trb(&mut self, mut raw: [u32; 4]) {
        crate::mesa_println!(
            "[DEBUG ENQUEUE ENTRY] entering enqueue_trb enqueue_idx={} cycle={}",
            self.enqueue,
            self.cycle
        );
        assert!(self.enqueue < RING_SIZE - 1, "TrbRing lleno");
        if self.cycle {
            raw[3] |= 1;
        } else {
            raw[3] &= !1u32;
        }
        let trb_phys = self.base_phys + (self.enqueue as u64) * 16;
        crate::mesa_println!(
            "[DEBUG ENQUEUE] writing TRB at phys={:#x} enqueue_idx={} cycle={} raw=[{:#x},{:#x},{:#x},{:#x}]",
            trb_phys, self.enqueue, self.cycle, raw[0], raw[1], raw[2], raw[3]
        );
        let p = unsafe { self.base_virt.add(self.enqueue * 4) };
        unsafe {
            core::ptr::write_volatile(p, raw[0]);
            core::ptr::write_volatile(p.add(1), raw[1]);
            core::ptr::write_volatile(p.add(2), raw[2]);
            core::ptr::write_volatile(p.add(3), raw[3]);
        }
        self.enqueue += 1;
        if self.enqueue == RING_SIZE - 1 {
            self.enqueue = 0;
            self.cycle = !self.cycle;
        }
    }
}

// ---------------------------------------------------------------------------
// Event Ring (sondeo del Event Ring sin interrupciones HW)
// ---------------------------------------------------------------------------
struct EventRing {
    base_virt: *const u32,
    base_phys: u64,
    dequeue: usize,
    cycle: bool,
}

impl EventRing {
    fn new(base_phys: u64, base_virt: *const u32) -> Self {
        unsafe { core::ptr::write_bytes(base_virt as *mut u8, 0, RING_SIZE * 16) };
        Self {
            base_virt,
            base_phys,
            dequeue: 0,
            cycle: true,
        }
    }

    fn poll(&mut self) -> Option<[u32; 4]> {
        let p = unsafe { self.base_virt.add(self.dequeue * 4) };
        let dw3 = unsafe { core::ptr::read_volatile(p.add(3)) };
        if (dw3 & 1) != self.cycle as u32 {
            return None;
        }
        let raw = unsafe {
            [
                core::ptr::read_volatile(p),
                core::ptr::read_volatile(p.add(1)),
                core::ptr::read_volatile(p.add(2)),
                core::ptr::read_volatile(p.add(3)),
            ]
        };
        self.dequeue += 1;
        if self.dequeue >= RING_SIZE {
            self.dequeue = 0;
            self.cycle = !self.cycle;
        }
        Some(raw)
    }

    fn dequeue_phys(&self) -> u64 {
        self.base_phys + (self.dequeue as u64 * 16)
    }
}

// ---------------------------------------------------------------------------
// Espera un Command Completion Event en el Event Ring
// Devuelve (slot_id, completion_code) o None si timeout
// Timeout en milisegundos
// ---------------------------------------------------------------------------
fn wait_for_completion(
    evt: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    timeout_ms: u64,
) -> Option<(u8, u8, u8)> {
    // Returns (slot_id, completion_code, trb_type)
    let start = read_tsc();
    let target = ms_to_spins(timeout_ms);

    loop {
        if let Some(raw) = evt.poll() {
            let trb_type = (raw[3] >> 10) & 0x3F;
            let slot_id = (raw[3] >> 24) as u8;
            let cc = ((raw[2] >> 24) & 0xFF) as u8;
            crate::mesa_println!(
                "[xHCI-DBG] Evento dequeued: Type={}, CC={}, Slot={}",
                trb_type,
                cc,
                slot_id
            );

            if trb_type == 33 || trb_type == 32 {
                // Command Completion (33) or Transfer Event (32)
                let remaining = raw[2] & 0x00FFFFFF;
                crate::mesa_println!(
                    "[DEBUG EVENT] Type={} CC={} Slot={} DW0={:#010x} DW1={:#010x} DW2={:#010x} DW3={:#010x} Remaining={}",
                    trb_type, cc, slot_id,
                    raw[0], raw[1], raw[2], raw[3], remaining
                );
                // Actualizar ERDP
                let new_erdp = evt.dequeue_phys() & !0xFu64;
                let mut ir = regs.interrupter_register_set.interrupter_mut(0);
                let mut erdp = ir.erdp.read_volatile();
                erdp.set_event_ring_dequeue_pointer(new_erdp);
                ir.erdp.write_volatile(erdp);
                return Some((slot_id, cc, trb_type as u8));
            }
            // Evento de otro tipo (Port Status Change, etc.) — consumir y seguir
            let new_erdp = evt.dequeue_phys() & !0xFu64;
            let mut ir = regs.interrupter_register_set.interrupter_mut(0);
            let mut erdp = ir.erdp.read_volatile();
            erdp.set_event_ring_dequeue_pointer(new_erdp);
            ir.erdp.write_volatile(erdp);
        }

        // Verificar timeout
        if read_tsc() - start >= target {
            return None;
        }

        core::hint::spin_loop();
    }
}
// ---------------------------------------------------------------------------
// Toca el timbre del Host Controller (Doorbell 0)
// ---------------------------------------------------------------------------
fn ring_hc_doorbell_target(bar0: u64, dboff: u32, db_idx: u8, target: u8) {
    let db_virt = crate::memory::vmm::phys_to_virt(bar0 + dboff as u64) as *mut u32;
    let val = target as u32;
    unsafe {
        core::ptr::write_volatile(db_virt.add(db_idx as usize), val);
    }
}

// ---------------------------------------------------------------------------
// Address Device: construye Input Context, Transfer Ring de EP0
// y envía el comando Address Device (Type=11)
// ---------------------------------------------------------------------------
fn address_device(
    slot_id: u8,
    port_idx: u8,   // índice 0-based del puerto
    port_speed: u8, // 1=FS, 2=LS, 3=HS, 4=SS
    dcbaa_virt: *mut u64,
    adapter: &mut PmmXhciAdapter,
    cmd_ring: &mut TrbRing,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
) {
    use crate::memory::xhci_access::XhciMemory;

    crate::mesa_println!("[xHCI] Iniciando Address Device para slot {}...", slot_id);

    let max_pkt: u32 = match port_speed {
        3 => 64,
        1 | 2 => 8,
        4 => 512,
        _ => 64,
    };

    let ep0_ring_phys = match adapter.alloc_64byte(RING_SIZE * 16) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ERROR: no hay memoria para Transfer Ring EP0");
            return;
        }
    };
    let ep0_ring_virt = adapter.virt_from_phys(ep0_ring_phys) as *mut u32;
    unsafe {
        core::ptr::write_bytes(ep0_ring_virt as *mut u8, 0, RING_SIZE * 16);
    }
    let ep0_link = unsafe { ep0_ring_virt.add((RING_SIZE - 1) * 4) };
    unsafe {
        core::ptr::write_volatile(ep0_link, (ep0_ring_phys.as_u64() & 0xFFFF_FFF0) as u32);
        core::ptr::write_volatile(ep0_link.add(1), (ep0_ring_phys.as_u64() >> 32) as u32);
        core::ptr::write_volatile(ep0_link.add(2), 0u32);
        core::ptr::write_volatile(ep0_link.add(3), (6u32 << 10) | (1 << 1) | 1);
    }
    crate::mesa_println!(
        "[xHCI] EP0 Transfer Ring @ phys={:#x}",
        ep0_ring_phys.as_u64()
    );

    let dev_ctx_phys = match adapter.alloc_64byte(512) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ERROR: no hay memoria para Device Context");
            return;
        }
    };
    let dev_ctx_virt = adapter.virt_from_phys(dev_ctx_phys) as *mut u8;
    unsafe {
        core::ptr::write_bytes(dev_ctx_virt, 0, 512);
    }
    unsafe {
        core::ptr::write_volatile(dcbaa_virt.add(slot_id as usize), dev_ctx_phys.as_u64());
    }
    crate::mesa_println!(
        "[xHCI] Device Context @ phys={:#x}  DCBAA[{}] actualizado",
        dev_ctx_phys.as_u64(),
        slot_id
    );

    let ctx_size = unsafe { XHCI_CONTEXT_SIZE };
    let ctx_dwords = ctx_size / 4;
    let slot_ctx_dw = 1 * ctx_dwords;
    let ep0_ctx_dw = 2 * ctx_dwords;
    let in_ctx_phys = match adapter.alloc_64byte(512) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ERROR: no hay memoria para Input Context");
            return;
        }
    };
    let in_ctx_virt = adapter.virt_from_phys(in_ctx_phys) as *mut u32;
    unsafe {
        core::ptr::write_bytes(in_ctx_virt as *mut u8, 0, 512);
    }

    unsafe {
        core::ptr::write_volatile(in_ctx_virt.add(0), 0u32);
        core::ptr::write_volatile(in_ctx_virt.add(1), 0b01u32);
        let slot_dw0: u32 = ((port_speed as u32) << 20) | (1u32 << 27);
        let slot_dw1: u32 = ((port_idx as u32 + 1) << 16);
        core::ptr::write_volatile(in_ctx_virt.add(slot_ctx_dw), slot_dw0);
        core::ptr::write_volatile(in_ctx_virt.add(slot_ctx_dw + 1), slot_dw1);
        core::ptr::write_volatile(in_ctx_virt.add(slot_ctx_dw + 2), 0u32);
        core::ptr::write_volatile(in_ctx_virt.add(slot_ctx_dw + 3), 0u32);
        let ep0_dw1: u32 = (3u32 << 1) | (4u32 << 3) | (0u32 << 8) | (max_pkt << 16);
        let ep0_dw2: u32 = (ep0_ring_phys.as_u64() as u32 & 0xFFFF_FFF0) | 1;
        let ep0_dw3: u32 = (ep0_ring_phys.as_u64() >> 32) as u32;
        let ep0_dw4: u32 = 8;
        core::ptr::write_volatile(in_ctx_virt.add(ep0_ctx_dw), 0u32);
        core::ptr::write_volatile(in_ctx_virt.add(ep0_ctx_dw + 1), ep0_dw1);
        core::ptr::write_volatile(in_ctx_virt.add(ep0_ctx_dw + 2), ep0_dw2);
        core::ptr::write_volatile(in_ctx_virt.add(ep0_ctx_dw + 3), ep0_dw3);
        core::ptr::write_volatile(in_ctx_virt.add(ep0_ctx_dw + 4), ep0_dw4);
    }
    crate::mesa_println!(
        "[xHCI] Input Context @ phys={:#x}  (MaxPkt={})",
        in_ctx_phys.as_u64(),
        max_pkt
    );

    crate::mesa_println!("[xHCI] Fase 1: Address Device con BSR=1 (validación)...");
    let bsr1_trb: [u32; 4] = [
        (in_ctx_phys.as_u64() & 0xFFFF_FFF0) as u32,
        (in_ctx_phys.as_u64() >> 32) as u32,
        0u32,
        ((slot_id as u32) << 24) | (11u32 << 10) | (1 << 9),
    ];
    cmd_ring.enqueue_trb(bsr1_trb);
    ring_hc_doorbell_target(bar0, dboff, 0, 0);
    let bsr1_ok = match wait_for_completion(evt_ring, regs, 5000) {
        Some((_, 1, _)) => {
            crate::mesa_println!("[xHCI] ✓ BSR=1 exitoso (CC=1) — contextos 100% correctos");
            true
        }
        Some((_, cc, _)) => {
            crate::mesa_println!(
                "[xHCI] ✗ BSR=1 falló CC={} — error en layout de contextos",
                cc
            );
            false
        }
        None => {
            crate::mesa_println!("[xHCI] ✗ BSR=1 timeout");
            false
        }
    };
    if !bsr1_ok {
        crate::mesa_println!("[xHCI] ✗ Address Device abortado — contextos inválidos");
        return;
    }

    let mut address_success = false;
    let mut last_cc = 0u8;
    crate::mesa_println!("[xHCI] Fase 2: Address Device con BSR=0 (asignar dirección)...");
    for attempt in 0..3 {
        if attempt > 0 {
            delay_ms(50 * attempt as u64);
        }
        let addr_trb: [u32; 4] = [
            (in_ctx_phys.as_u64() & 0xFFFF_FFF0) as u32,
            (in_ctx_phys.as_u64() >> 32) as u32,
            0u32,
            ((slot_id as u32) << 24) | (11u32 << 10),
        ];
        cmd_ring.enqueue_trb(addr_trb);
        ring_hc_doorbell_target(bar0, dboff, 0, 0);
        match wait_for_completion(evt_ring, regs, 5000) {
            Some((_, cc, _)) if cc == 1 => {
                address_success = true;
                crate::mesa_println!("[xHCI] ✓ Address Device BSR=0 exitoso! (CC=1)");
                unsafe {
                    let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
                    USB_DEVICES[idx].phase = UsbPhase::AddressAssigned;
                }
                break;
            }
            Some((_, cc, _)) => {
                last_cc = cc;
                crate::mesa_println!(
                    "[xHCI] ✗ BSR=0 falló CC={} ({})",
                    cc,
                    completion_code_to_string(cc)
                );
                if cc != 17 {
                    break;
                }
            }
            None => {
                crate::mesa_println!("[xHCI] ✗ Timeout BSR=0");
                break;
            }
        }
    }

    if address_success {
        // Re-crear struct TrbRing para EP0 y hacer Control Transfer
        let mut ep0_ring = TrbRing {
            base_virt: ep0_ring_virt,
            base_phys: ep0_ring_phys.as_u64(),
            enqueue: 0,
            cycle: true,
        };
        get_device_descriptor(
            slot_id,
            &mut ep0_ring,
            cmd_ring,
            evt_ring,
            regs,
            bar0,
            dboff,
            adapter,
        );
    } else {
        crate::mesa_println!(
            "[xHCI] ✗ Address Device falló definitivamente. Último CC = {} ({})",
            last_cc,
            completion_code_to_string(last_cc)
        );
    }
}

// Función auxiliar para convertir códigos de completion a strings legibles
fn completion_code_to_string(cc: u8) -> &'static str {
    match cc {
        1 => "Success",
        2 => "Data Buffer Error",
        3 => "Babble Detected",
        4 => "USB Transaction Error",
        5 => "TRB Error",
        6 => "Stall Error",
        7 => "Resource Error",
        8 => "Bandwidth Error",
        9 => "No Slots Available",
        10 => "Invalid Stream Type",
        11 => "Slot Not Enabled",
        12 => "Endpoint Not Enabled",
        13 => "Short Packet",
        14 => "Ring Underrun",
        15 => "Ring Overrun",
        16 => "VF Event Ring Full",
        17 => "Parameter Error",
        18 => "Bandwidth Overrun",
        19 => "Context State Error",
        20 => "No Ping Response",
        21 => "Event Ring Full",
        22 => "Incompatible Device",
        23 => "Missed Service",
        24 => "Command Ring Stopped",
        25 => "Command Aborted",
        26 => "Stopped",
        27 => "Stopped - Length Invalid",
        28 => "Stopped - Short Packet",
        29 => "Max Exit Latency Too Large",
        31 => "Isoch Buffer Overrun",
        32 => "Event Lost",
        33 => "Undefined Error",
        34 => "Invalid Stream ID",
        35 => "Secondary Bandwidth Error",
        36 => "Split Transaction Error",
        _ => "Unknown",
    }
}

// ---------------------------------------------------------------------------
// Obtener el Device Descriptor del dispositivo (Vendor ID, Product ID)
// ---------------------------------------------------------------------------
fn get_device_descriptor(
    slot_id: u8,
    ep0_ring: &mut TrbRing,
    cmd_ring: &mut TrbRing,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
    adapter: &mut PmmXhciAdapter,
) {
    use crate::memory::xhci_access::XhciMemory;

    crate::mesa_println!("[xHCI] Obteniendo Device Descriptor...");

    // Reintentos para obtener Device Descriptor
    let mut success = false;
    let mut last_cc = 0u8;

    for attempt in 0..5 {
        crate::mesa_println!("[xHCI] Intento {} de Get Device Descriptor...", attempt + 1);

        // Delay entre reintentos
        if attempt > 0 {
            crate::mesa_println!("[xHCI] Esperando antes del reintento {}...", attempt);
            delay_ms(10);
        }

        // Buffer para 18 bytes del Descriptor
        let buf_phys = match adapter.alloc_64byte(64) {
            Some(p) => p,
            None => {
                crate::mesa_println!("[xHCI] Error alocando buffer para Descriptor");
                continue;
            }
        };
        let buf_virt = adapter.virt_from_phys(buf_phys) as *mut u8;

        // 1. Setup Stage TRB
        let req = 0x80 | (0x06 << 8) | (0x0100 << 16);
        let len = 0x0012 << 16;
        let setup_trb: [u32; 4] = [req, len, 8, (2 << 10) | (1 << 6) | (3 << 16)];
        ep0_ring.enqueue_trb(setup_trb);

        // 2. Data Stage TRB (Type 3)
        let data_trb: [u32; 4] = [
            (buf_phys.as_u64() & 0xFFFF_FFF0) as u32,
            (buf_phys.as_u64() >> 32) as u32,
            18,
            (3 << 10) | (1 << 16),
        ];
        ep0_ring.enqueue_trb(data_trb);

        // 3. Status Stage TRB (Type 4)
        let status_trb: [u32; 4] = [0, 0, 0, (4 << 10) | (1 << 5)];
        ep0_ring.enqueue_trb(status_trb);

        // Ring Doorbell
        ring_hc_doorbell_target(bar0, dboff, slot_id, 1);

        // Esperar Transfer Event (5 segundos según spec xHCI)
        crate::mesa_println!("[xHCI] Esperando Transfer Event (Device Descriptor)...");
        if let Some((_, cc, trb_type)) = wait_for_completion(evt_ring, regs, 5000) {
            if trb_type == 32 {
                if cc == 1 || cc == 13 {
                    success = true;
                    let vendor_id =
                        unsafe { core::ptr::read_volatile(buf_virt.add(8) as *const u16) };
                    let product_id =
                        unsafe { core::ptr::read_volatile(buf_virt.add(10) as *const u16) };
                    crate::mesa_println!("[xHCI] ✓ Device Descriptor leído!");
                    crate::mesa_println!(
                        "[xHCI] >>> Vendor ID: {:#06X}, Product ID: {:#06X} <<<",
                        vendor_id,
                        product_id
                    );
                    unsafe {
                        let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
                        USB_DEVICES[idx].vendor_id = vendor_id;
                        USB_DEVICES[idx].product_id = product_id;
                        USB_DEVICES[idx].phase = UsbPhase::DeviceDescriptorRead;
                    }
                    break;
                } else {
                    last_cc = cc;
                    crate::mesa_println!(
                        "[xHCI] Error en Data/Status TRB. Code = {} ({})",
                        cc,
                        completion_code_to_string(cc)
                    );
                    // Reintentar en errores recuperables
                    if cc == 17 || cc == 1 || cc == 13 {
                        continue;
                    } else {
                        break;
                    }
                }
            }
        } else {
            crate::mesa_println!(
                "[xHCI] Timeout esperando Evento de Transferencia (Device Descriptor) - intento {}",
                attempt + 1
            );
        }
    }

    if success {
        // Fetch Configuration Descriptor
        get_config_descriptor(
            slot_id, ep0_ring, cmd_ring, evt_ring, regs, bar0, dboff, adapter,
        );
    } else {
        crate::mesa_println!("[xHCI] ✗ No se pudo leer el Device Descriptor después de 5 intentos. Último CC = {} ({})", last_cc, completion_code_to_string(last_cc));
        unsafe {
            let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
            USB_DEVICES[idx].phase = UsbPhase::Failed;
        }
    }
}

// ---------------------------------------------------------------------------
// Obtener el Configuration Descriptor del dispositivo
// ---------------------------------------------------------------------------
fn get_config_descriptor(
    slot_id: u8,
    ep0_ring: &mut TrbRing,
    cmd_ring: &mut TrbRing,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
    adapter: &mut PmmXhciAdapter,
) {
    use crate::memory::xhci_access::XhciMemory;

    crate::mesa_println!("[xHCI] Obteniendo Configuration Descriptor...");

    // Reintentos para obtener Config Descriptor
    let mut success = false;
    let mut last_cc = 0u8;

    for attempt in 0..5 {
        crate::mesa_println!("[xHCI] Intento {} de Get Config Descriptor...", attempt + 1);

        // Delay entre reintentos
        if attempt > 0 {
            crate::mesa_println!("[xHCI] Esperando antes del reintento {}...", attempt);
            delay_ms(10);
        }

        // Buffer for full descriptor (up to 256 bytes usually sufficient)
        let buf_phys = match adapter.alloc_64byte(256) {
            Some(p) => p,
            None => {
                crate::mesa_println!("[xHCI] Error alocando buffer para Config Descriptor");
                continue;
            }
        };
        let buf_virt = adapter.virt_from_phys(buf_phys) as *mut u8;
        unsafe {
            core::ptr::write_bytes(buf_virt, 0, 256);
        }

        // 1. Setup Stage TRB
        let req = 0x80 | (0x06 << 8) | (0x0200 << 16);
        let len = 0x0100 << 16; // 256

        let setup_trb: [u32; 4] = [req, len, 8, (2 << 10) | (1 << 6) | (3 << 16)];
        ep0_ring.enqueue_trb(setup_trb);

        // 2. Data Stage TRB (Type 3)
        let data_trb: [u32; 4] = [
            (buf_phys.as_u64() & 0xFFFF_FFF0) as u32,
            (buf_phys.as_u64() >> 32) as u32,
            256,
            (3 << 10) | (1 << 16),
        ];
        ep0_ring.enqueue_trb(data_trb);

        // 3. Status Stage TRB (Type 4)
        let status_trb: [u32; 4] = [0, 0, 0, (4 << 10) | (1 << 5)];
        ep0_ring.enqueue_trb(status_trb);

        // Ring Doorbell
        ring_hc_doorbell_target(bar0, dboff, slot_id, 1);

        // Esperar eventos (5 segundos según spec xHCI)
        crate::mesa_println!("[xHCI] Esperando Transfer Event (Config Descriptor)...");
        if let Some((_, cc, trb_type)) = wait_for_completion(evt_ring, regs, 5000) {
            if trb_type == 32 {
                if cc == 1 || cc == 13 {
                    success = true;
                    let total_length =
                        unsafe { core::ptr::read_volatile(buf_virt.add(2) as *const u16) };
                    let num_interfaces = unsafe { core::ptr::read_volatile(buf_virt.add(4)) };
                    crate::mesa_println!(
                        "[xHCI] ✓ Config Descriptor leído! Total length: {}, Interfaces: {}",
                        total_length,
                        num_interfaces
                    );

                    let config_val = unsafe { core::ptr::read_volatile(buf_virt.add(5)) };
                    let endpoints = parse_config_descriptor(buf_virt, total_length as usize);

                    // Extraer clase del primer interfaz
                    let if_class = {
                        let data =
                            unsafe { core::slice::from_raw_parts(buf_virt, total_length as usize) };
                        let mut c = 0u8;
                        let mut i = if data.len() > 0 { data[0] as usize } else { 0 };
                        while i + 7 < data.len() {
                            if data[i + 1] == 4 {
                                c = data[i + 5];
                                break;
                            }
                            let len = data[i] as usize;
                            if len == 0 {
                                break;
                            }
                            i += len;
                        }
                        c
                    };

                    unsafe {
                        let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
                        USB_DEVICES[idx].num_interfaces = num_interfaces;
                        USB_DEVICES[idx].num_endpoints = endpoints.len() as u8;
                        USB_DEVICES[idx].class = if_class;
                        USB_DEVICES[idx].phase = UsbPhase::ConfigDescriptorRead;
                    }

                    // Configure Endpoints
                    if configure_endpoints(
                        slot_id, &endpoints, adapter, cmd_ring, evt_ring, regs, bar0, dboff,
                    ) {
                        set_configuration(
                            slot_id, config_val, ep0_ring, evt_ring, regs, bar0, dboff,
                        );
                        // Iniciar diálogo BOT si es Mass Storage
                        if if_class == 0x08 {
                            crate::mesa_println!(
                                "[xHCI] 🖴  Dispositivo Mass Storage detectado! Iniciando BOT..."
                            );
                            mass_storage_init(slot_id, adapter, evt_ring, regs, bar0, dboff);
                        }
                    }
                    break;
                } else {
                    last_cc = cc;
                    crate::mesa_println!(
                        "[xHCI] Error al leer Config Descriptor. Code = {} ({})",
                        cc,
                        completion_code_to_string(cc)
                    );
                    // Reintentar en errores recuperables
                    if cc == 17 || cc == 1 || cc == 13 {
                        continue;
                    } else {
                        break;
                    }
                }
            }
        } else {
            crate::mesa_println!(
                "[xHCI] Timeout esperando Evento de Transferencia (Config Descriptor) - intento {}",
                attempt + 1
            );
        }
    }

    if !success {
        crate::mesa_println!("[xHCI] ✗ No se pudo leer el Config Descriptor después de 5 intentos. Último CC = {} ({})", last_cc, completion_code_to_string(last_cc));
        unsafe {
            let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
            USB_DEVICES[idx].phase = UsbPhase::Failed;
        }
    }
}

struct ParsedEndpoint {
    address: u8,
    attributes: u8,
    max_packet_size: u16,
}

fn parse_config_descriptor(buf_virt: *const u8, len: usize) -> alloc::vec::Vec<ParsedEndpoint> {
    let data = unsafe { core::slice::from_raw_parts(buf_virt, len) };
    let mut iter = crate::drivers::usb::descriptors::DescriptorIter::new(data);
    let mut endpoints = alloc::vec::Vec::new();

    while let Some((dtype, slice)) = iter.next() {
        match dtype {
            2 => {
                // Configuration
                crate::mesa_println!("  - Configuration Descriptor");
            }
            4 => {
                // Interface
                let intf_num = slice[2];
                let num_eps = slice[4];
                let class = slice[5];
                crate::mesa_println!(
                    "  - Interface {} (Class: {}, Endpoints: {})",
                    intf_num,
                    class,
                    num_eps
                );
            }
            5 => {
                // Endpoint
                let addr = slice[2];
                let attr = slice[3];
                let max_pkt = u16::from_le_bytes([slice[4], slice[5]]);
                crate::mesa_println!(
                    "    - Endpoint {:#04X} (Attr: {}, MaxPkt: {})",
                    addr,
                    attr,
                    max_pkt
                );
                endpoints.push(ParsedEndpoint {
                    address: addr,
                    attributes: attr,
                    max_packet_size: max_pkt,
                });
            }
            _ => {}
        }
    }
    endpoints
}

// ---------------------------------------------------------------------------
// Configure Endpoints
// ---------------------------------------------------------------------------
fn configure_endpoints(
    slot_id: u8,
    endpoints: &[ParsedEndpoint],
    adapter: &mut PmmXhciAdapter,
    cmd_ring: &mut TrbRing,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
) -> bool {
    use crate::memory::xhci_access::XhciMemory;
    crate::mesa_println!("[xHCI] Configurando Endpoints...");

    // Reintentos para Configure Endpoint
    let mut success = false;
    let mut last_cc = 0u8;

    for attempt in 0..3 {
        crate::mesa_println!("[xHCI] Intento {} de Configure Endpoint...", attempt + 1);

        // Delay entre reintentos
        if attempt > 0 {
            crate::mesa_println!("[xHCI] Esperando antes del reintento {}...", attempt);
            delay_ms(10);
        }

        let in_ctx_phys = match adapter.alloc_64byte(1024) {
            Some(p) => p,
            None => {
                crate::mesa_println!("[xHCI] ERROR: no hay memoria para Input Context");
                continue;
            }
        };
        let in_ctx_virt = adapter.virt_from_phys(in_ctx_phys) as *mut u32;
        unsafe {
            core::ptr::write_bytes(in_ctx_virt as *mut u8, 0, 1024);
        }

        let mut add_flags = 0u32;
        let mut max_dci = 1u32;
        let ctx_dwords = unsafe { XHCI_CONTEXT_SIZE } / 4;

        // Calculate DCI for each endpoint and setup context
        for ep in endpoints {
            let ep_num = ep.address & 0x0F;
            let is_in = (ep.address & 0x80) != 0;
            let dci = (ep_num as u32 * 2) + if is_in { 1 } else { 0 };

            if dci > max_dci {
                max_dci = dci;
            }
            add_flags |= 1 << dci;

            // Allocate Transfer Ring for this endpoint
            let ep_ring_phys = match adapter.alloc_64byte(RING_SIZE * 16) {
                Some(p) => p,
                None => {
                    crate::mesa_println!("[xHCI] ERROR: no hay memoria para Transfer Ring EP");
                    continue;
                }
            };
            let ep_ring_virt = adapter.virt_from_phys(ep_ring_phys) as *mut u32;
            unsafe {
                core::ptr::write_bytes(ep_ring_virt as *mut u8, 0, RING_SIZE * 16);
            }
            // Link TRB
            let ep_link = unsafe { ep_ring_virt.add((RING_SIZE - 1) * 4) };
            unsafe {
                core::ptr::write_volatile(ep_link, (ep_ring_phys.as_u64() & 0xFFFF_FFF0) as u32);
                core::ptr::write_volatile(ep_link.add(1), (ep_ring_phys.as_u64() >> 32) as u32);
                core::ptr::write_volatile(ep_link.add(2), 0u32);
                core::ptr::write_volatile(ep_link.add(3), (6u32 << 10) | (1 << 1) | 1);
            }

            let ep_type = ep.attributes & 0x03;
            let xhci_ep_type = match ep_type {
                1 => {
                    if is_in {
                        5
                    } else {
                        1
                    }
                }
                2 => {
                    if is_in {
                        6
                    } else {
                        2
                    }
                }
                3 => {
                    if is_in {
                        7
                    } else {
                        3
                    }
                }
                _ => 4,
            };

            // Save ring addresses for Bulk endpoints (mass storage BOT)
            if ep_type == 0x02 {
                let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
                if !is_in {
                    unsafe {
                        USB_DEVICES[idx].bulk_out_dci = dci as u8;
                    }
                    unsafe {
                        USB_DEVICES[idx].bulk_out_ring_phys = ep_ring_phys.as_u64();
                    }
                    unsafe {
                        USB_DEVICES[idx].bulk_out_ring_virt = ep_ring_virt as u64;
                    }
                } else {
                    unsafe {
                        USB_DEVICES[idx].bulk_in_dci = dci as u8;
                    }
                    unsafe {
                        USB_DEVICES[idx].bulk_in_ring_phys = ep_ring_phys.as_u64();
                    }
                    unsafe {
                        USB_DEVICES[idx].bulk_in_ring_virt = ep_ring_virt as u64;
                    }
                }
            }

            let ep_ctx_offset = (dci as usize + 1) * ctx_dwords;
            unsafe {
                let ep_dw1 = (3u32 << 1)
                    | ((xhci_ep_type as u32) << 3)
                    | ((ep.max_packet_size as u32) << 16);
                let ep_dw2 = (ep_ring_phys.as_u64() as u32 & 0xFFFF_FFF0) | 1;
                let ep_dw3 = (ep_ring_phys.as_u64() >> 32) as u32;
                let ep_dw4 = 8;

                core::ptr::write_volatile(in_ctx_virt.add(ep_ctx_offset + 1), ep_dw1);
                core::ptr::write_volatile(in_ctx_virt.add(ep_ctx_offset + 2), ep_dw2);
                core::ptr::write_volatile(in_ctx_virt.add(ep_ctx_offset + 3), ep_dw3);
                core::ptr::write_volatile(in_ctx_virt.add(ep_ctx_offset + 4), ep_dw4);
                crate::mesa_println!(
                    "[DEBUG ENDPOINT CTX] DCI={} ep_ctx_offset={} DW1={:#010x} DW2={:#010x} DW3={:#010x} DW4={:#010x} ring_addr={:#x}",
                    dci, ep_ctx_offset, ep_dw1, ep_dw2, ep_dw3, ep_dw4, ep_ring_phys.as_u64()
                );
            }
        }

        unsafe {
            core::ptr::write_volatile(in_ctx_virt.add(0), 0);
            core::ptr::write_volatile(in_ctx_virt.add(1), add_flags);
            crate::mesa_println!(
                "[DEBUG INPUT CTX] add_flags={:#b} max_dci={} ctx_dwords={}",
                add_flags,
                max_dci,
                ctx_dwords
            );
            let slot_ctx_offset = 1 * ctx_dwords;
            // Context Entries = max_dci (bits 31:27), Speed (bits 23:20), Root Hub Port (bits 23:16)
            let dev_info = &USB_DEVICES[(slot_id as usize).min(MAX_USB_DEVICES - 1)];
            let slot_dw0 = (max_dci << 27) | ((dev_info.speed as u32) << 20);
            let slot_dw1 = ((dev_info.port as u32 + 1) << 16);
            core::ptr::write_volatile(in_ctx_virt.add(slot_ctx_offset), slot_dw0);
            core::ptr::write_volatile(in_ctx_virt.add(slot_ctx_offset + 1), slot_dw1);
        }

        // Configure Endpoint TRB (Type 12)
        let conf_trb: [u32; 4] = [
            (in_ctx_phys.as_u64() & 0xFFFF_FFF0) as u32,
            (in_ctx_phys.as_u64() >> 32) as u32,
            0,
            ((slot_id as u32) << 24) | (12u32 << 10),
        ];
        cmd_ring.enqueue_trb(conf_trb);
        ring_hc_doorbell_target(bar0, dboff, 0, 0);

        crate::mesa_println!("[xHCI] Esperando Command Completion (Configure Endpoint)...");
        match wait_for_completion(evt_ring, regs, 5000) {
            Some((_, cc, _)) if cc == 1 => {
                crate::mesa_println!("[xHCI] ✓ Configure Endpoint exitoso! (CC=1)");
                unsafe {
                    let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
                    USB_DEVICES[idx].phase = UsbPhase::EndpointsConfigured;
                }
                success = true;
                break;
            }
            Some((_, cc, _)) => {
                last_cc = cc;
                crate::mesa_println!(
                    "[xHCI] ✗ Configure Endpoint falló en intento {}. CC = {} ({})",
                    attempt + 1,
                    cc,
                    completion_code_to_string(cc)
                );
                // Reintentar en errores recuperables
                if cc == 17 || cc == 1 {
                    continue;
                } else {
                    break;
                }
            }
            None => {
                crate::mesa_println!(
                    "[xHCI] ✗ Timeout en Configure Endpoint (200M spins) - intento {}",
                    attempt + 1
                );
                break;
            }
        }
    }

    if !success {
        crate::mesa_println!(
            "[xHCI] ✗ Configure Endpoint falló después de 3 intentos. Último CC = {} ({})",
            last_cc,
            completion_code_to_string(last_cc)
        );
        unsafe {
            let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
            USB_DEVICES[idx].phase = UsbPhase::Failed;
        }
    }
    success
}

// ---------------------------------------------------------------------------
// Set Configuration
// ---------------------------------------------------------------------------
fn set_configuration(
    slot_id: u8,
    config_val: u8,
    ep0_ring: &mut TrbRing,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
) {
    crate::mesa_println!("[xHCI] Estableciendo configuración {}...", config_val);

    // Reintentos para Set Configuration
    let mut success = false;
    let mut last_cc = 0u8;

    for attempt in 0..5 {
        crate::mesa_println!("[xHCI] Intento {} de Set Configuration...", attempt + 1);

        // Delay entre reintentos
        if attempt > 0 {
            crate::mesa_println!("[xHCI] Esperando antes del reintento {}...", attempt);
            delay_ms(10);
        }

        // Setup Stage TRB: SET_CONFIGURATION
        let req = 0x00 | (0x09 << 8) | ((config_val as u32) << 16);
        let setup_trb: [u32; 4] = [req, 0, 8, (2 << 10) | (1 << 6)];
        ep0_ring.enqueue_trb(setup_trb);

        // Status Stage TRB (Type 4)
        let status_trb: [u32; 4] = [0, 0, 0, (4 << 10) | (1 << 5) | (1 << 16)];
        ep0_ring.enqueue_trb(status_trb);

        ring_hc_doorbell_target(bar0, dboff, slot_id, 1);

        crate::mesa_println!("[xHCI] Esperando Transfer Event (Set Configuration)...");
        if let Some((_, cc, trb_type)) = wait_for_completion(evt_ring, regs, 5000) {
            if trb_type == 32 && cc == 1 {
                crate::mesa_println!("[xHCI] ✓ Configuración establecida.");
                unsafe {
                    let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
                    USB_DEVICES[idx].config_val = config_val;
                    USB_DEVICES[idx].phase = UsbPhase::SetConfigurationDone;
                }
                success = true;
                break;
            } else {
                last_cc = cc;
                crate::mesa_println!(
                    "[xHCI] ✗ Set Configuration falló en intento {}. CC = {} ({})",
                    attempt + 1,
                    cc,
                    completion_code_to_string(cc)
                );
                // Reintentar en errores recuperables
                if cc == 17 || cc == 1 || cc == 13 {
                    continue;
                } else {
                    break;
                }
            }
        } else {
            crate::mesa_println!(
                "[xHCI] Timeout esperando Transfer Event (Set Configuration) - intento {}",
                attempt + 1
            );
        }
    }

    if !success {
        crate::mesa_println!(
            "[xHCI] ✗ Set Configuration falló después de 5 intentos. Último CC = {} ({})",
            last_cc,
            completion_code_to_string(last_cc)
        );
        unsafe {
            let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
            USB_DEVICES[idx].phase = UsbPhase::Failed;
        }
    }
}

// ---------------------------------------------------------------------------
// Mass Storage BOT: READ CAPACITY (10) vía Bulk endpoints
// ---------------------------------------------------------------------------
fn mass_storage_init(
    slot_id: u8,
    adapter: &mut PmmXhciAdapter,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
) {
    use crate::memory::xhci_access::XhciMemory;

    let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
    let dev = unsafe { &USB_DEVICES[idx] };
    let bulk_out_virt = dev.bulk_out_ring_virt as *mut u32;
    let bulk_in_virt = dev.bulk_in_ring_virt as *mut u32;

    if bulk_out_virt.is_null() || bulk_in_virt.is_null() {
        crate::mesa_println!("[xHCI] ❌ mass_storage: Bulk rings no configurados");
        return;
    }

    let out_dci = dev.bulk_out_dci;
    let in_dci = dev.bulk_in_dci;
    crate::mesa_println!(
        "[xHCI] ⚡ Mass Storage detectado! Bulk OUT DCI={}, IN DCI={}",
        out_dci,
        in_dci
    );

    // Construir TrbRing wrappers sobre los rings ya configurados
    let mut out_ring = TrbRing {
        base_virt: bulk_out_virt,
        base_phys: dev.bulk_out_ring_phys,
        enqueue: 0,
        cycle: true,
    };
    let mut in_ring = TrbRing {
        base_virt: bulk_in_virt,
        base_phys: dev.bulk_in_ring_phys,
        enqueue: 0,
        cycle: true,
    };

    // Buffers físicos: CBW (31), datos (8), CSW (13)
    let cbw_buf = match adapter.alloc_64byte(64) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ❌ mass_storage: no mem CBW");
            return;
        }
    };
    let data_buf = match adapter.alloc_64byte(64) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ❌ mass_storage: no mem DATA");
            return;
        }
    };
    let csw_buf = match adapter.alloc_64byte(64) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ❌ mass_storage: no mem CSW");
            return;
        }
    };

    let cbw_virt = adapter.virt_from_phys(cbw_buf) as *mut u8;
    let data_virt = adapter.virt_from_phys(data_buf) as *mut u8;
    let csw_virt = adapter.virt_from_phys(csw_buf) as *mut u8;

    unsafe {
        core::ptr::write_bytes(cbw_virt, 0, 64);
    }
    unsafe {
        core::ptr::write_bytes(data_virt, 0, 64);
    }
    unsafe {
        core::ptr::write_bytes(csw_virt, 0, 64);
    }

    // Construir CBW: READ CAPACITY (10)
    let cbw = cbw_virt as *mut CommandBlockWrapper;
    unsafe {
        (*cbw).d_cbw_signature = CBW_SIGNATURE.to_le();
        (*cbw).d_cbw_tag = 1u32.to_le();
        (*cbw).d_cbw_data_transfer_length = 8u32.to_le();
        (*cbw).bm_cbw_flags = 0x80;
        (*cbw).b_cbw_lun = 0;
        (*cbw).b_cbw_cb_length = 10;
        (*cbw).cbw_cb = [0u8; 16];
        (*cbw).cbw_cb[0] = SCSI_READ_CAPACITY_10;
    }
    crate::mesa_println!(
        "[xHCI] CBW construido (READ CAPACITY 10) @ {:#x}",
        cbw_buf.as_u64()
    );

    // ── Fase 1: Enviar CBW por Bulk OUT (DCI = out_dci) ──
    let out_trb: [u32; 4] = [
        (cbw_buf.as_u64() & 0xFFFF_FFF0) as u32,
        (cbw_buf.as_u64() >> 32) as u32,
        31,
        (1 << 10) | (1 << 5),
    ];
    crate::mesa_println!(
        "[DEBUG EP] CBW → out_ring (phys={:#x}) doorbell target={}",
        out_ring.base_phys,
        out_dci
    );
    out_ring.enqueue_trb(out_trb);
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    ring_hc_doorbell_target(bar0, dboff, slot_id, out_dci);
    crate::mesa_println!("[xHCI] Fase 1: CBW enviado, esperando Transfer Event...");
    if wait_for_completion(evt_ring, regs, 5000).is_none() {
        crate::mesa_println!("[xHCI] ❌ Fase 1: timeout");
        return;
    }
    crate::mesa_println!("[xHCI] ✓ Fase 1: CBW enviado OK");

    // ── Fase 2: Recibir 8 bytes de datos por Bulk IN (DCI = in_dci) ──
    let data_trb: [u32; 4] = [
        (data_buf.as_u64() & 0xFFFF_FFF0) as u32,
        (data_buf.as_u64() >> 32) as u32,
        8,
        (1 << 10) | (1 << 5), // Type=1 (Normal) | IOC
    ];
    in_ring.enqueue_trb(data_trb);
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    ring_hc_doorbell_target(bar0, dboff, slot_id, in_dci);
    crate::mesa_println!("[xHCI] Fase 2: esperando datos (8 bytes)...");
    if wait_for_completion(evt_ring, regs, 5000).is_none() {
        crate::mesa_println!("[xHCI] ❌ Fase 2: timeout");
        return;
    }
    crate::mesa_println!("[xHCI] ✓ Fase 2: datos recibidos");

    // ── Fase 3: Recibir CSW (13 bytes) por Bulk IN ──
    crate::mesa_println!(
        "[DEBUG CSW ADDR] csw_buf.phys={:#x} csw_virt={:p}",
        csw_buf.as_u64(),
        csw_virt
    );
    crate::mesa_println!(
        "[DEBUG EP] CSW → in_ring (phys={:#x}) doorbell target={}",
        in_ring.base_phys,
        in_dci
    );
    // Buffer poisoning: fill with 0xAA to detect if DMA actually writes
    unsafe {
        core::ptr::write_bytes(csw_virt, 0xAA, 64);
    }
    let csw_phys = csw_buf.as_u64();
    let csw_dw0 = csw_phys as u32;
    let csw_dw1 = (csw_phys >> 32) as u32;
    let csw_trb: [u32; 4] = [
        csw_dw0,
        csw_dw1,
        13,
        (1 << 10) | (1 << 5), // Type=1 (Normal) | IOC
    ];
    crate::mesa_println!(
        "[DEBUG CSW TRB] DWORDs: {:08x} {:08x} {:08x} {:08x}",
        csw_trb[0],
        csw_trb[1],
        csw_trb[2],
        csw_trb[3]
    );
    in_ring.enqueue_trb(csw_trb);
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    ring_hc_doorbell_target(bar0, dboff, slot_id, in_dci);
    crate::mesa_println!("[xHCI] Fase 3: esperando CSW...");
    if wait_for_completion(evt_ring, regs, 5000).is_none() {
        crate::mesa_println!("[xHCI] ❌ Fase 3: timeout");
        return;
    }
    crate::mesa_println!("[xHCI] ✓ Fase 3: CSW recibido");

    // ── Interpretar resultados ──
    compiler_fence(Ordering::SeqCst);
    unsafe {
        core::arch::asm!("clflush [{}]", in(reg) csw_virt, options(nostack, preserves_flags));
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    compiler_fence(Ordering::SeqCst);
    // Raw byte dump after event + cache flush
    let b0 = unsafe { core::ptr::read_volatile(csw_virt.add(0)) };
    let b1 = unsafe { core::ptr::read_volatile(csw_virt.add(1)) };
    let b2 = unsafe { core::ptr::read_volatile(csw_virt.add(2)) };
    let b3 = unsafe { core::ptr::read_volatile(csw_virt.add(3)) };
    crate::mesa_println!(
        "[DEBUG CSW RAW] csw_buf bytes: {:02x} {:02x} {:02x} {:02x}",
        b0,
        b1,
        b2,
        b3
    );
    let csw = unsafe { &*(csw_virt as *const CommandStatusWrapper) };
    let csw_sig =
        u32::from_le(unsafe { core::ptr::read_volatile(core::ptr::addr_of!(csw.d_csw_signature)) });
    let csw_status = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(csw.b_csw_status)) };

    crate::mesa_println!(
        "[xHCI] CSW: sig={:#x} tag={} residue={} status={}",
        csw_sig,
        u32::from_le(unsafe { core::ptr::read_volatile(core::ptr::addr_of!(csw.d_csw_tag)) }),
        u32::from_le(unsafe {
            core::ptr::read_volatile(core::ptr::addr_of!(csw.d_csw_data_residue))
        }),
        csw_status
    );

    if csw_sig != CSW_SIGNATURE {
        crate::mesa_println!("[xHCI] ❌ CSW signature inválida");
        return;
    }
    if csw_status != 0 {
        crate::mesa_println!("[xHCI] ❌ CSW status = {} (fallo)", csw_status);
        return;
    }

    let raw_data = unsafe { core::slice::from_raw_parts(data_virt, 8) };
    let max_lba = u32::from_be_bytes([raw_data[0], raw_data[1], raw_data[2], raw_data[3]]);
    let block_size = u32::from_be_bytes([raw_data[4], raw_data[5], raw_data[6], raw_data[7]]);
    let total_sectors = max_lba + 1;
    let capacity_mb = (total_sectors as u64 * block_size as u64) / (1024 * 1024);

    crate::mesa_println!("══════════════════════════════════════════════");
    crate::mesa_println!("  💾 PENDRIVE DETECTADO");
    crate::mesa_println!(
        "  📐 Capacidad: {} MB ({} sectores × {} bytes)",
        capacity_mb,
        total_sectors,
        block_size
    );
    crate::mesa_println!("  🏁 Último LBA: {:#x}", max_lba);
    crate::mesa_println!("══════════════════════════════════════════════");

    unsafe {
        USB_DEVICES[idx].class = 0x08;
        USB_DEVICES[idx].transfers_ok += 1;
    }

    // ── Leer MBR (LBA 0) ──
    crate::mesa_println!("[xHCI] Leyendo MBR (LBA 0)...");
    let mut mbr_buf = [0u8; 512];
    if scsi_read_10(
        0,
        1,
        &mut mbr_buf,
        &mut out_ring,
        &mut in_ring,
        slot_id,
        out_dci,
        in_dci,
        adapter,
        evt_ring,
        regs,
        bar0,
        dboff,
    ) {
        let magic = u16::from_le_bytes([mbr_buf[510], mbr_buf[511]]);
        crate::mesa_println!("  📋 MBR: magic={:#06x}", magic);
        if magic == 0xAA55 {
            crate::mesa_println!("  ✅ Firma MBR válida (0xAA55)");
            let mut part_count = 0u8;
            let mut is_gpt = false;
            for i in 0..4 {
                let off = 446 + i * 16;
                let ptype = mbr_buf[off + 4];
                let lba_start = u32::from_le_bytes([
                    mbr_buf[off + 8],
                    mbr_buf[off + 9],
                    mbr_buf[off + 10],
                    mbr_buf[off + 11],
                ]);
                let sector_count = u32::from_le_bytes([
                    mbr_buf[off + 12],
                    mbr_buf[off + 13],
                    mbr_buf[off + 14],
                    mbr_buf[off + 15],
                ]);
                if ptype != 0 {
                    let type_str = match ptype {
                        0x0B | 0x0C => "FAT32",
                        0x83 => "Linux ext",
                        0x07 => "NTFS/exFAT",
                        _ => "desconocido",
                    };
                    crate::mesa_println!(
                        "    Partición {}: type={:#04x} ({}) LBA={} sectores={} ({:.1} MB)",
                        i + 1,
                        ptype,
                        type_str,
                        lba_start,
                        sector_count,
                        sector_count as f64 * 512.0 / (1024.0 * 1024.0)
                    );
                    if ptype == 0xEE {
                        crate::mesa_println!("  🏁 Protective MBR (GPT) detectado!");
                        is_gpt = true;
                    }
                    unsafe {
                        USB_DEVICES[idx].part_type[part_count as usize] = ptype;
                        USB_DEVICES[idx].part_lba_start[part_count as usize] = lba_start as u64;
                        USB_DEVICES[idx].part_lba_end[part_count as usize] =
                            (lba_start as u64 + sector_count as u64 - 1);
                        USB_DEVICES[idx].part_size_mb[part_count as usize] =
                            (sector_count as u64 * 512 / (1024 * 1024)) as u32;
                    }
                    part_count += 1;
                }
            }
            unsafe {
                USB_DEVICES[idx].num_partitions = part_count;
            }

            if is_gpt {
                parse_gpt(
                    slot_id,
                    &mut out_ring,
                    &mut in_ring,
                    out_dci,
                    in_dci,
                    adapter,
                    evt_ring,
                    regs,
                    bar0,
                    dboff,
                    total_sectors as u64,
                );
            }
        } else {
            crate::mesa_println!("  ⚠️  MBR sin firma 0xAA55 (GPT?)");
        }
    } else {
        crate::mesa_println!("  ❌ Error leyendo MBR");
    }

    // ── Guardar estado global para acceso post-init ──
    unsafe {
        USB_STORAGE_STATE.ready = true;
        USB_STORAGE_STATE.slot_id = slot_id;
        USB_STORAGE_STATE.out_dci = out_dci;
        USB_STORAGE_STATE.in_dci = in_dci;
        USB_STORAGE_STATE.out_ring_phys = USB_DEVICES[idx].bulk_out_ring_phys;
        USB_STORAGE_STATE.out_ring_virt = USB_DEVICES[idx].bulk_out_ring_virt;
        USB_STORAGE_STATE.out_ring_enqueue = out_ring.enqueue;
        USB_STORAGE_STATE.out_ring_cycle = out_ring.cycle;
        USB_STORAGE_STATE.in_ring_phys = USB_DEVICES[idx].bulk_in_ring_phys;
        USB_STORAGE_STATE.in_ring_virt = USB_DEVICES[idx].bulk_in_ring_virt;
        USB_STORAGE_STATE.in_ring_enqueue = in_ring.enqueue;
        USB_STORAGE_STATE.in_ring_cycle = in_ring.cycle;
        USB_STORAGE_STATE.free_start_lba = USB_DEVICES[idx].free_start_lba;
        USB_STORAGE_STATE.free_sectors = USB_DEVICES[idx].free_sectors;
        USB_STORAGE_STATE.capacity_sectors = total_sectors as u64;
        USB_STORAGE_STATE.block_size = block_size;
        USB_STORAGE_STATE.bar0 = bar0;
        USB_STORAGE_STATE.dboff = dboff;
        USB_STORAGE_STATE.evt_ring_phys = evt_ring.base_phys;
        USB_STORAGE_STATE.evt_ring_virt = evt_ring.base_virt as u64;
        USB_STORAGE_STATE.evt_ring_dequeue = evt_ring.dequeue;
        USB_STORAGE_STATE.evt_ring_cycle = evt_ring.cycle;
        USB_STORAGE_STATE.caplength = core::ptr::read_volatile(bar0 as *const u8);
    }

    crate::mesa_println!("[xHCI] ✅ USB Mass Storage registrado como block device");

    // Inicializar MesaFS (auto-descubre partición GPT con firma MESA_FS1)
    let partition_created = crate::drivers::usb::mesa_fs::mesa_fs_init();

    if partition_created {
        crate::mesa_println!("[MesaFS] ✅ Nueva partición MesaFS creada/formateada");
    }

    if crate::drivers::usb::mesa_fs::mesa_fs_is_initialized() {
        crate::fs::register_disk_fs();
        crate::mesa_println!("[VFS] Disco USB montado exitosamente en /disks/usb_disk_0/");
    } else {
        crate::mesa_println!("[MesaFS] ⚠️  No se pudo inicializar la particion de persistencia");
    }
}

fn scsi_read_10(
    lba: u32,
    sector_count: u16,
    buffer: &mut [u8],
    out_ring: &mut TrbRing,
    in_ring: &mut TrbRing,
    slot_id: u8,
    out_dci: u8,
    in_dci: u8,
    adapter: &mut PmmXhciAdapter,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
) -> bool {
    use crate::memory::xhci_access::XhciMemory;

    let transfer_len = (sector_count as u32) * 512;
    if buffer.len() < transfer_len as usize {
        crate::mesa_println!("[SCSI] buffer demasiado pequeño");
        return false;
    }

    crate::mesa_println!(
        "[DEBUG EP] scsi_read_10: out_ring_phys={:#x} in_ring_phys={:#x} out_dci={} in_dci={} slot_id={}",
        out_ring.base_phys, in_ring.base_phys, out_dci, in_dci, slot_id
    );

    let cbw_buf = match adapter.alloc_64byte(64) {
        Some(p) => p,
        None => {
            return false;
        }
    };
    let data_buf = match adapter.alloc_64byte(transfer_len as usize) {
        Some(p) => p,
        None => {
            return false;
        }
    };
    let csw_buf = match adapter.alloc_64byte(64) {
        Some(p) => p,
        None => {
            return false;
        }
    };

    let cbw_virt = adapter.virt_from_phys(cbw_buf) as *mut u8;
    let data_virt = adapter.virt_from_phys(data_buf) as *mut u8;
    let csw_virt = adapter.virt_from_phys(csw_buf) as *mut u8;

    unsafe {
        core::ptr::write_bytes(cbw_virt, 0, 64);
    }
    unsafe {
        core::ptr::write_bytes(data_virt, 0, transfer_len as usize);
    }
    unsafe {
        core::ptr::write_bytes(csw_virt, 0, 64);
    }

    // Construir CBW: READ (10)
    let cbw = cbw_virt as *mut CommandBlockWrapper;
    unsafe {
        static NEXT_TAG: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(1);
        let tag = NEXT_TAG.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        (*cbw).d_cbw_signature = CBW_SIGNATURE.to_le();
        (*cbw).d_cbw_tag = tag.to_le();
        (*cbw).d_cbw_data_transfer_length = transfer_len.to_le();
        (*cbw).bm_cbw_flags = 0x80;
        (*cbw).b_cbw_lun = 0;
        (*cbw).b_cbw_cb_length = 10;
        (*cbw).cbw_cb = [0u8; 16];
        (*cbw).cbw_cb[0] = 0x28;
        let lba_b = lba.to_be_bytes();
        (*cbw).cbw_cb[2] = lba_b[0];
        (*cbw).cbw_cb[3] = lba_b[1];
        (*cbw).cbw_cb[4] = lba_b[2];
        (*cbw).cbw_cb[5] = lba_b[3];
        let cnt_b = sector_count.to_be_bytes();
        (*cbw).cbw_cb[7] = cnt_b[0];
        (*cbw).cbw_cb[8] = cnt_b[1];
    }

    // Fase 1: Enviar CBW por Bulk OUT
    let out_trb: [u32; 4] = [
        (cbw_buf.as_u64() & 0xFFFF_FFF0) as u32,
        (cbw_buf.as_u64() >> 32) as u32,
        31,
        (1 << 10) | (1 << 5),
    ];
    out_ring.enqueue_trb(out_trb);
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    ring_hc_doorbell_target(bar0, dboff, slot_id, out_dci);
    if wait_for_completion(evt_ring, regs, 5000).is_none() {
        return false;
    }

    // Fase 2: Recibir datos por Bulk IN
    let data_phys = data_buf.as_u64();
    let data_virt_ptr = data_virt as *const u8;
    crate::mesa_println!(
        "[DEBUG READ DMA] Buffer Virt: {:p} | Phys: {:#x} | LBA: {} | Count: {}",
        data_virt_ptr,
        data_phys,
        lba,
        sector_count
    );

    let data_trb: [u32; 4] = [
        (data_buf.as_u64() & 0xFFFF_FFF0) as u32,
        (data_buf.as_u64() >> 32) as u32,
        transfer_len,
        (1 << 10) | (1 << 5),
    ];
    in_ring.enqueue_trb(data_trb);
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    ring_hc_doorbell_target(bar0, dboff, slot_id, in_dci);
    if wait_for_completion(evt_ring, regs, 5000).is_none() {
        return false;
    }

    // Fase 3: Recibir CSW por Bulk IN
    crate::mesa_println!(
        "[DEBUG CSW ADDR] scsi_read_10 csw_buf.phys={:#x} csw_virt={:p}",
        csw_buf.as_u64(),
        csw_virt
    );
    crate::mesa_println!(
        "[DEBUG EP] CSW → in_ring (phys={:#x}) doorbell target={}",
        in_ring.base_phys,
        in_dci
    );
    // Buffer poisoning: fill with 0xAA to detect if DMA actually writes
    unsafe {
        core::ptr::write_bytes(csw_virt, 0xAA, 64);
    }
    let csw_phys = csw_buf.as_u64();
    let csw_dw0 = csw_phys as u32;
    let csw_dw1 = (csw_phys >> 32) as u32;
    let csw_trb: [u32; 4] = [csw_dw0, csw_dw1, 13, (1 << 10) | (1 << 5)];
    crate::mesa_println!(
        "[DEBUG CSW TRB] scsi_read_10 DWORDs: {:08x} {:08x} {:08x} {:08x}",
        csw_trb[0],
        csw_trb[1],
        csw_trb[2],
        csw_trb[3]
    );
    in_ring.enqueue_trb(csw_trb);
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    ring_hc_doorbell_target(bar0, dboff, slot_id, in_dci);
    if wait_for_completion(evt_ring, regs, 5000).is_none() {
        return false;
    }

    // Interpretar resultados: flush cache + volatile reads for CSW
    compiler_fence(Ordering::SeqCst);
    unsafe {
        core::arch::asm!("clflush [{}]", in(reg) csw_virt, options(nostack, preserves_flags));
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    compiler_fence(Ordering::SeqCst);
    // Raw byte dump after event + cache flush
    let b0 = unsafe { core::ptr::read_volatile(csw_virt.add(0)) };
    let b1 = unsafe { core::ptr::read_volatile(csw_virt.add(1)) };
    let b2 = unsafe { core::ptr::read_volatile(csw_virt.add(2)) };
    let b3 = unsafe { core::ptr::read_volatile(csw_virt.add(3)) };
    crate::mesa_println!(
        "[DEBUG CSW RAW] scsi_read_10 csw_buf bytes: {:02x} {:02x} {:02x} {:02x}",
        b0,
        b1,
        b2,
        b3
    );
    let csw = unsafe { &*(csw_virt as *const CommandStatusWrapper) };
    let sig =
        u32::from_le(unsafe { core::ptr::read_volatile(core::ptr::addr_of!(csw.d_csw_signature)) });
    let csw_status = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(csw.b_csw_status)) };
    let csw_residue = u32::from_le(unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(csw.d_csw_data_residue))
    });
    crate::mesa_println!(
        "[DEBUG READ] CSW Sig: {:#x} | Status: {} | Residue: {} | LBA: {}",
        sig,
        csw_status,
        csw_residue,
        lba
    );
    if sig != CSW_SIGNATURE || csw_status != 0 {
        crate::mesa_println!(
            "[DEBUG READ] ❌ CSW error: sig={:#x} (expected {:#x}), status={}",
            sig,
            CSW_SIGNATURE,
            csw_status
        );
        return false;
    }

    unsafe {
        core::ptr::copy_nonoverlapping(data_virt, buffer.as_mut_ptr(), transfer_len as usize);
    }
    true
}

// ---------------------------------------------------------------------------
// SCSI WRITE (10) — opcode 0x2A
// Envía datos al dispositivo por Bulk OUT: CBW → DATA → CSW
// ---------------------------------------------------------------------------
fn scsi_write_10(
    lba: u32,
    sector_count: u16,
    buffer: &[u8],
    out_ring: &mut TrbRing,
    in_ring: &mut TrbRing,
    slot_id: u8,
    out_dci: u8,
    in_dci: u8,
    adapter: &mut PmmXhciAdapter,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
) -> bool {
    use crate::memory::xhci_access::XhciMemory;

    let transfer_len = (sector_count as u32) * 512;
    if buffer.len() < transfer_len as usize {
        crate::mesa_println!("[SCSI] buffer demasiado pequeño para write");
        return false;
    }

    crate::mesa_println!(
        "[DEBUG EP] scsi_write_10: out_ring_phys={:#x} in_ring_phys={:#x} out_dci={} in_dci={} slot_id={}",
        out_ring.base_phys, in_ring.base_phys, out_dci, in_dci, slot_id
    );

    // Reintentar hasta 2 veces (original + 1 retry tras REQUEST SENSE)
    for attempt in 0..2 {
        let cbw_buf = match adapter.alloc_64byte(64) {
            Some(p) => p,
            None => return false,
        };
        let data_buf = match adapter.alloc_64byte(transfer_len as usize) {
            Some(p) => p,
            None => return false,
        };
        let csw_buf = match adapter.alloc_64byte(64) {
            Some(p) => p,
            None => return false,
        };

        let cbw_virt = adapter.virt_from_phys(cbw_buf) as *mut u8;
        let data_virt = adapter.virt_from_phys(data_buf) as *mut u8;
        let csw_virt = adapter.virt_from_phys(csw_buf) as *mut u8;

        unsafe {
            core::ptr::write_bytes(cbw_virt, 0, 64);
        }
        unsafe {
            core::ptr::copy_nonoverlapping(buffer.as_ptr(), data_virt, transfer_len as usize);
        }
        unsafe {
            core::ptr::write_bytes(csw_virt, 0, 64);
        }

        // Construir CBW: WRITE (10)
        let cbw = cbw_virt as *mut CommandBlockWrapper;
        let cbw_tag: u32;
        unsafe {
            static NEXT_TAG: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(1);
            let tag = NEXT_TAG.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            cbw_tag = tag;
            (*cbw).d_cbw_signature = CBW_SIGNATURE.to_le();
            (*cbw).d_cbw_tag = tag.to_le();
            (*cbw).d_cbw_data_transfer_length = transfer_len.to_le();
            (*cbw).bm_cbw_flags = 0x00; // Data-Out (Host→Device)
            (*cbw).b_cbw_lun = 0;
            (*cbw).b_cbw_cb_length = 10;
            let mut cdb = [0u8; 16];
            cdb[0] = 0x2A; // SCSI WRITE (10)
            cdb[1] = 0x00;
            cdb[2..6].copy_from_slice(&(lba as u32).to_be_bytes());
            cdb[6] = 0x00;
            cdb[7..9].copy_from_slice(&(sector_count as u16).to_be_bytes());
            cdb[9] = 0x00;
            (*cbw).cbw_cb = cdb;
        }

        // Fase 1: CBW por Bulk OUT
        let out_trb: [u32; 4] = [
            (cbw_buf.as_u64() & 0xFFFF_FFF0) as u32,
            (cbw_buf.as_u64() >> 32) as u32,
            31,
            (1 << 10) | (1 << 5),
        ];
        out_ring.enqueue_trb(out_trb);
        unsafe {
            core::arch::asm!("mfence", options(nostack, preserves_flags));
        }
        ring_hc_doorbell_target(bar0, dboff, slot_id, out_dci);
        if wait_for_completion(evt_ring, regs, 5000).is_none() {
            return false;
        }

        // Fase 2: Datos por Bulk OUT
        let data_phys = data_buf.as_u64();
        let data_virt_ptr = data_virt as *const u8;

        // IMPRESIÓN OBLIGATORIA DE ALINEAMIENTO DEL BUFFER
        crate::mesa_println!(
            "[DEBUG DMA] Buffer Virt: {:p} | Phys: {:#x}",
            data_virt_ptr,
            data_phys
        );

        let data_trb: [u32; 4] = [
            (data_buf.as_u64() & 0xFFFF_FFF0) as u32,
            (data_buf.as_u64() >> 32) as u32,
            transfer_len,
            (1 << 10) | (1 << 5),
        ];
        out_ring.enqueue_trb(data_trb);
        unsafe {
            core::arch::asm!("mfence", options(nostack, preserves_flags));
        }
        ring_hc_doorbell_target(bar0, dboff, slot_id, out_dci);
        if wait_for_completion(evt_ring, regs, 5000).is_none() {
            return false;
        }

        // Fase 3: CSW por Bulk IN
        crate::mesa_println!(
            "[DEBUG CSW ADDR] scsi_write_10 csw_buf.phys={:#x} csw_virt={:p}",
            csw_buf.as_u64(),
            csw_virt
        );
        crate::mesa_println!(
            "[DEBUG EP] CSW → in_ring (phys={:#x}) doorbell target={}",
            in_ring.base_phys,
            in_dci
        );
        // Buffer poisoning: fill with 0xAA to detect if DMA actually writes
        unsafe {
            core::ptr::write_bytes(csw_virt, 0xAA, 64);
        }
        let csw_phys = csw_buf.as_u64();
        let csw_dw0 = csw_phys as u32;
        let csw_dw1 = (csw_phys >> 32) as u32;
        let csw_trb: [u32; 4] = [csw_dw0, csw_dw1, 13, (1 << 10) | (1 << 5)];
        crate::mesa_println!(
            "[DEBUG CSW TRB] scsi_write_10 DWORDs: {:08x} {:08x} {:08x} {:08x}",
            csw_trb[0],
            csw_trb[1],
            csw_trb[2],
            csw_trb[3]
        );
        in_ring.enqueue_trb(csw_trb);
        unsafe {
            core::arch::asm!("mfence", options(nostack, preserves_flags));
        }
        ring_hc_doorbell_target(bar0, dboff, slot_id, in_dci);
        if wait_for_completion(evt_ring, regs, 5000).is_none() {
            return false;
        }

        // Interpretar resultados: flush cache + volatile reads for CSW
        compiler_fence(Ordering::SeqCst);
        unsafe {
            core::arch::asm!("clflush [{}]", in(reg) csw_virt, options(nostack, preserves_flags));
            core::arch::asm!("mfence", options(nostack, preserves_flags));
        }
        compiler_fence(Ordering::SeqCst);
        // Raw byte dump after event + cache flush
        let b0 = unsafe { core::ptr::read_volatile(csw_virt.add(0)) };
        let b1 = unsafe { core::ptr::read_volatile(csw_virt.add(1)) };
        let b2 = unsafe { core::ptr::read_volatile(csw_virt.add(2)) };
        let b3 = unsafe { core::ptr::read_volatile(csw_virt.add(3)) };
        crate::mesa_println!(
            "[DEBUG CSW RAW] scsi_write_10 csw_buf bytes: {:02x} {:02x} {:02x} {:02x}",
            b0,
            b1,
            b2,
            b3
        );
        let csw = unsafe { &*(csw_virt as *const CommandStatusWrapper) };
        let sig = u32::from_le(unsafe {
            core::ptr::read_volatile(core::ptr::addr_of!(csw.d_csw_signature))
        });
        let csw_tag =
            u32::from_le(unsafe { core::ptr::read_volatile(core::ptr::addr_of!(csw.d_csw_tag)) });
        let csw_status = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(csw.b_csw_status)) };
        let csw_residue = u32::from_le(unsafe {
            core::ptr::read_volatile(core::ptr::addr_of!(csw.d_csw_data_residue))
        });

        // IMPRESIÓN OBLIGATORIA DEL CSW ANTES DE CUALQUIER IF O RETURN
        crate::mesa_println!(
            "[DEBUG WRITE] CSW Sig: {:#x} | Status: {} | Residue: {}",
            sig,
            csw_status,
            csw_residue
        );

        if sig != CSW_SIGNATURE || csw_status != 0 || csw_tag != cbw_tag {
            crate::mesa_println!(
                "[SCSI WRITE DEBUG] Tag CBW: {:#x} | Tag CSW: {:#x} | Status: {} | Sign: {:#x}",
                cbw_tag,
                csw_tag,
                csw_status,
                sig
            );
        }
        if sig != CSW_SIGNATURE {
            return false;
        }
        if csw_status == 0 {
            return true;
        }

        crate::mesa_println!(
            "[SCSI WRITE] CSW status error: {} (1=comando fallido, 2=error de fase)",
            csw_status
        );
        if attempt == 0 {
            scsi_request_sense(
                adapter, out_ring, in_ring, slot_id, out_dci, in_dci, evt_ring, regs, bar0, dboff,
            );
            crate::mesa_println!("[SCSI WRITE] Reintentando escritura LBA {}...", lba);
        }
    }
    false
}
fn scsi_request_sense(
    adapter: &mut PmmXhciAdapter,
    out_ring: &mut TrbRing,
    in_ring: &mut TrbRing,
    slot_id: u8,
    out_dci: u8,
    in_dci: u8,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
) {
    use crate::memory::xhci_access::XhciMemory;
    let sense_buf = match adapter.alloc_64byte(18) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[SCSI] no mem REQUEST SENSE");
            return;
        }
    };
    let cbw_buf = match adapter.alloc_64byte(64) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[SCSI] no mem RS CBW");
            return;
        }
    };
    let csw_buf = match adapter.alloc_64byte(64) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[SCSI] no mem RS CSW");
            return;
        }
    };
    let cbw_virt = adapter.virt_from_phys(cbw_buf) as *mut u8;
    let sense_virt = adapter.virt_from_phys(sense_buf) as *mut u8;
    let csw_virt = adapter.virt_from_phys(csw_buf) as *mut u8;
    unsafe {
        core::ptr::write_bytes(cbw_virt, 0, 64);
    }
    unsafe {
        core::ptr::write_bytes(sense_virt, 0, 18);
    }
    unsafe {
        core::ptr::write_bytes(csw_virt, 0, 64);
    }

    let cbw = cbw_virt as *mut CommandBlockWrapper;
    unsafe {
        static NEXT_TAG: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(1);
        let tag = NEXT_TAG.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        (*cbw).d_cbw_signature = CBW_SIGNATURE.to_le();
        (*cbw).d_cbw_tag = tag.to_le();
        (*cbw).d_cbw_data_transfer_length = 18u32.to_le();
        (*cbw).bm_cbw_flags = 0x80; // Data-In (Device→Host)
        (*cbw).b_cbw_lun = 0;
        (*cbw).b_cbw_cb_length = 6;
        let mut cdb = [0u8; 16];
        cdb[0] = 0x03; // REQUEST SENSE
        cdb[1] = 0x00;
        cdb[2] = 0x00;
        cdb[3] = 0x00;
        cdb[4] = 18; // allocation length
        cdb[5] = 0x00;
        (*cbw).cbw_cb = cdb;
    }

    // Fase 1: CBW
    let trb: [u32; 4] = [
        (cbw_buf.as_u64() & 0xFFFF_FFF0) as u32,
        (cbw_buf.as_u64() >> 32) as u32,
        31,
        (1 << 10) | (1 << 5),
    ];
    out_ring.enqueue_trb(trb);
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    ring_hc_doorbell_target(bar0, dboff, slot_id, out_dci);
    if wait_for_completion(evt_ring, regs, 5000).is_none() {
        return;
    }

    // Fase 2: recibir sense data
    let data_trb: [u32; 4] = [
        (sense_buf.as_u64() & 0xFFFF_FFF0) as u32,
        (sense_buf.as_u64() >> 32) as u32,
        18,
        (1 << 10) | (1 << 5),
    ];
    in_ring.enqueue_trb(data_trb);
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    ring_hc_doorbell_target(bar0, dboff, slot_id, in_dci);
    if wait_for_completion(evt_ring, regs, 5000).is_none() {
        return;
    }

    // Fase 3: CSW por Bulk IN
    crate::mesa_println!(
        "[DEBUG CSW ADDR] scsi_request_sense csw_buf.phys={:#x} csw_virt={:p}",
        csw_buf.as_u64(),
        csw_virt
    );
    crate::mesa_println!(
        "[DEBUG EP] CSW → in_ring (phys={:#x}) doorbell target={}",
        in_ring.base_phys,
        in_dci
    );
    // Buffer poisoning: fill with 0xAA to detect if DMA actually writes
    unsafe {
        core::ptr::write_bytes(csw_virt, 0xAA, 64);
    }
    let csw_phys = csw_buf.as_u64();
    let csw_dw0 = csw_phys as u32;
    let csw_dw1 = (csw_phys >> 32) as u32;
    let csw_trb: [u32; 4] = [csw_dw0, csw_dw1, 13, (1 << 10) | (1 << 5)];
    crate::mesa_println!(
        "[DEBUG CSW TRB] scsi_request_sense DWORDs: {:08x} {:08x} {:08x} {:08x}",
        csw_trb[0],
        csw_trb[1],
        csw_trb[2],
        csw_trb[3]
    );
    in_ring.enqueue_trb(csw_trb);
    unsafe {
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    ring_hc_doorbell_target(bar0, dboff, slot_id, in_dci);
    if wait_for_completion(evt_ring, regs, 5000).is_none() {
        return;
    }

    compiler_fence(Ordering::SeqCst);
    unsafe {
        core::arch::asm!("clflush [{}]", in(reg) csw_virt, options(nostack, preserves_flags));
        core::arch::asm!("mfence", options(nostack, preserves_flags));
    }
    compiler_fence(Ordering::SeqCst);
    // Raw byte dump after event + cache flush
    let b0 = unsafe { core::ptr::read_volatile(csw_virt.add(0)) };
    let b1 = unsafe { core::ptr::read_volatile(csw_virt.add(1)) };
    let b2 = unsafe { core::ptr::read_volatile(csw_virt.add(2)) };
    let b3 = unsafe { core::ptr::read_volatile(csw_virt.add(3)) };
    crate::mesa_println!(
        "[DEBUG CSW RAW] scsi_request_sense csw_buf bytes: {:02x} {:02x} {:02x} {:02x}",
        b0,
        b1,
        b2,
        b3
    );
    let csw = unsafe { &*(csw_virt as *const CommandStatusWrapper) };
    let csw_sig =
        u32::from_le(unsafe { core::ptr::read_volatile(core::ptr::addr_of!(csw.d_csw_signature)) });
    let csw_stat = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(csw.b_csw_status)) };
    if csw_sig != CSW_SIGNATURE || csw_stat != 0 {
        crate::mesa_println!(
            "[SCSI] REQUEST SENSE CSW fallo: sig={:#x} status={}",
            csw_sig,
            csw_stat
        );
        return;
    }
    let sense = unsafe { core::slice::from_raw_parts(sense_virt, 18) };
    let key = sense[2] & 0x0F;
    let asc = sense[12];
    let ascq = sense[13];
    // IMPRESIÓN OBLIGATORIA DEL REQUEST SENSE DIRECTAMENTE EN PANTALLA
    crate::mesa_println!(
        "[DEBUG SENSE] Key: {:#x} | ASC: {:#x} | ASCQ: {:#x}",
        key,
        asc,
        ascq
    );
}

// ---------------------------------------------------------------------------
// CRC32 para GPT
// ---------------------------------------------------------------------------
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// GPT Parser: Lee GPT Header (LBA 1) y entradas de partición
//
// Layout GPT Header (UEFI Spec):
//   Offset  Size  Field
//   0       8     signature "EFI PART"
//   8       4     revision
//   12      4     header_size
//   16      4     header_crc32
//   20      4     reserved
//   24      8     my_lba
//   32      8     alternate_lba
//   40      8     first_usable_lba
//   48      8     last_usable_lba
//   56      16    disk_guid
//   72      8     partition_entry_lba
//   80      4     num_partition_entries
//   84      4     size_partition_entry
//   88      4     partition_array_crc32
// ---------------------------------------------------------------------------
fn parse_gpt(
    slot_id: u8,
    out_ring: &mut TrbRing,
    in_ring: &mut TrbRing,
    out_dci: u8,
    in_dci: u8,
    adapter: &mut PmmXhciAdapter,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
    disk_total_sectors: u64,
) {
    use crate::memory::xhci_access::XhciMemory;

    let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
    let mut buf = [0u8; 512];

    // Leer GPT Header (LBA 1)
    if !scsi_read_10(
        1, 1, &mut buf, out_ring, in_ring, slot_id, out_dci, in_dci, adapter, evt_ring, regs, bar0,
        dboff,
    ) {
        crate::mesa_println!("[GPT] Error leyendo LBA 1 (GPT Header)");
        return;
    }

    // Validar firma
    if &buf[0..8] != b"EFI PART" {
        crate::mesa_println!("[GPT] Firma inválida (no EFI PART)");
        return;
    }

    // Extraer campos con from_le_bytes (Little-Endian nativo)
    let first_usable = u64::from_le_bytes(buf[40..48].try_into().unwrap());
    let last_usable = u64::from_le_bytes(buf[48..56].try_into().unwrap());
    let part_entry_lba = u64::from_le_bytes(buf[72..80].try_into().unwrap());
    let num_entries = u32::from_le_bytes(buf[80..84].try_into().unwrap());
    let entry_size = u32::from_le_bytes(buf[84..88].try_into().unwrap());

    // Mostrar valores para depuración
    crate::mesa_println!("  📋 GPT Header crudo: first_usable={}, last_usable={}, entries_lba={}, num={}, entry_size={}",
        first_usable, last_usable, part_entry_lba, num_entries, entry_size);

    // ── SANITY CHECKS: evitar división por cero y bucles infinitos ──
    let std_entry_size = entry_size == 128;
    let sane_num = num_entries > 0 && num_entries <= 128;
    let sane_entry_size = entry_size >= 128 && entry_size <= 512;
    let sane_lba_range = last_usable > first_usable && first_usable > 0;

    if !std_entry_size {
        crate::mesa_println!("  ⚠️  entry_size={} no es 128 (no estándar)", entry_size);
    }

    if !sane_num || !sane_entry_size || !sane_lba_range {
        crate::mesa_println!("[GPT] ⚠️  Cabecera GPT no estándar/híbrida — aplicando fallback");

        // Fallback: usar fin de partición MBR + READ CAPACITY 10
        // La partición MBR reportó LBA_start y sector_count.
        // MBR partition end = mbr_lba_start + mbr_sector_count - 1
        let dev_info = unsafe { &USB_DEVICES[idx] };
        let mut mbr_end = 0u64;
        for p in 0..dev_info.num_partitions as usize {
            if dev_info.part_type[p] != 0xEE {
                let end = dev_info.part_lba_end[p];
                if end > mbr_end {
                    mbr_end = end;
                }
            }
        }

        // Si no hay particiones MBR no-EE, usar valor fijo
        if mbr_end == 0 {
            mbr_end = 37047;
        }

        let free_start = mbr_end + 1;
        let free_sectors = disk_total_sectors.saturating_sub(free_start);
        let free_gb = (free_sectors as f64 * 512.0) / (1024.0 * 1024.0 * 1024.0);

        crate::mesa_println!("══════════════════════════════════════════════");
        crate::mesa_println!("  🆓 PERSISTENCIA MESAOS: ESPACIO LIBRE (FALLBACK)");
        crate::mesa_println!("  LBA Inicio Libre: {}", free_start);
        crate::mesa_println!("  LBA Fin Libre:    {}", disk_total_sectors - 1);
        crate::mesa_println!("  Sectores Libres:  {}", free_sectors);
        crate::mesa_println!("  Espacio Libre:    {:.2} GB disponibles", free_gb);
        crate::mesa_println!("══════════════════════════════════════════════");

        unsafe {
            USB_DEVICES[idx].is_gpt = true;
            USB_DEVICES[idx].free_start_lba = free_start;
            USB_DEVICES[idx].free_sectors = free_sectors;
        }
        return;
    }

    // ── Sanity check pasado: parsear entradas GPT ──
    crate::mesa_println!("  ✅ GPT Header válido (EFI PART)");
    unsafe {
        USB_DEVICES[idx].is_gpt = true;
    }

    let mut max_used = first_usable;
    let entries_per_sector = 512 / entry_size as usize;
    let total_secs = (num_entries as usize + entries_per_sector - 1) / entries_per_sector;

    let mut part_count = 0u8;
    let mut entry_buf = [0u8; 512];

    for sec in 0..total_secs.min(32) {
        let lba = part_entry_lba + sec as u64;
        if !scsi_read_10(
            lba as u32,
            1,
            &mut entry_buf,
            out_ring,
            in_ring,
            slot_id,
            out_dci,
            in_dci,
            adapter,
            evt_ring,
            regs,
            bar0,
            dboff,
        ) {
            crate::mesa_println!("[GPT] Error leyendo LBA {} (entradas)", lba);
            break;
        }

        for ei in 0..entries_per_sector {
            if part_count >= 8 {
                break;
            }
            let off = ei * entry_size as usize;
            if off + 16 > 512 {
                break;
            }

            let guid: [u8; 16] = entry_buf[off..off + 16].try_into().unwrap();
            if guid == [0u8; 16] {
                continue;
            }

            let start = u64::from_le_bytes(entry_buf[off + 32..off + 40].try_into().unwrap());
            let end = u64::from_le_bytes(entry_buf[off + 40..off + 48].try_into().unwrap());
            let sectors = end - start + 1;
            let mb = (sectors * 512) / (1024 * 1024);

            // Convertir UTF-16LE name (offset 56, 36 × u16) a ASCII
            let mut name_bytes = [0u8; 36];
            for j in 0..18 {
                let idx = off + 56 + j * 2;
                if idx + 2 > 512 {
                    break;
                }
                let c = u16::from_le_bytes([entry_buf[idx], entry_buf[idx + 1]]);
                if c == 0 {
                    break;
                }
                name_bytes[j] = (c & 0xFF) as u8;
            }

            crate::mesa_println!(
                "    🗂️  #{} LBA {:<12}→{:<12}  {} MB  {}",
                part_count + 1,
                start,
                end,
                mb,
                core::str::from_utf8(&name_bytes).unwrap_or("?")
            );

            unsafe {
                USB_DEVICES[idx].part_type[part_count as usize] = 0xEE;
                USB_DEVICES[idx].part_lba_start[part_count as usize] = start;
                USB_DEVICES[idx].part_lba_end[part_count as usize] = end;
                USB_DEVICES[idx].part_size_mb[part_count as usize] = mb as u32;
            }
            part_count += 1;

            if end > max_used {
                max_used = end;
            }
        }
    }

    unsafe {
        USB_DEVICES[idx].num_partitions = part_count;
    }

    // Calcular espacio libre: usar límite físico real si GPT está truncado por ISO
    let free_start = max_used + 1;
    let gpt_end = last_usable;

    let real_end = if free_start >= gpt_end && disk_total_sectors > free_start {
        crate::mesa_println!("[GPT] ⚠️  Espacio libre según GPT truncado por ISO híbrida");
        crate::mesa_println!("[GPT] Usando capacidad física real (READ CAPACITY 10)...");
        disk_total_sectors - 1
    } else {
        gpt_end
    };

    if real_end >= free_start {
        let free_sectors = real_end - free_start + 1;
        let free_gb = (free_sectors as f64 * 512.0) / (1024.0 * 1024.0 * 1024.0);

        crate::mesa_println!("══════════════════════════════════════════════");
        crate::mesa_println!("  🆓 ESPACIO LIBRE PARA MESAOS");
        crate::mesa_println!("  LBA inicio libre: {}", free_start);
        crate::mesa_println!("  LBA fin libre:    {}", real_end);
        crate::mesa_println!("  Sectores libres:  {}", free_sectors);
        crate::mesa_println!("  Espacio:          {:.2} GB", free_gb);
        crate::mesa_println!("══════════════════════════════════════════════");

        unsafe {
            USB_DEVICES[idx].free_start_lba = free_start;
            USB_DEVICES[idx].free_sectors = free_sectors;
        }
    } else {
        crate::mesa_println!("  ⚠️  No hay espacio libre disponible");
    }
}

// ---------------------------------------------------------------------------
// Agregar entrada de partición en tabla GPT
// ---------------------------------------------------------------------------
pub fn add_gpt_partition_entry(
    out_ring: &mut TrbRing,
    in_ring: &mut TrbRing,
    out_dci: u8,
    in_dci: u8,
    adapter: &mut PmmXhciAdapter,
    evt_ring: &mut EventRing,
    regs: &mut Registers<MemoryMapper>,
    bar0: u64,
    dboff: u32,
    slot_id: u8,
    start_lba: u64,
    end_lba: u64,
    partition_name: &str,
) -> bool {
    use crate::memory::xhci_access::XhciMemory;

    crate::mesa_println!(
        "[GPT] Agregando partición '{}' en LBA {}-{}...",
        partition_name,
        start_lba,
        end_lba
    );

    let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);

    // Leer GPT Header (LBA 1)
    let mut header_buf = [0u8; 512];
    if !scsi_read_10(
        1,
        1,
        &mut header_buf,
        out_ring,
        in_ring,
        slot_id,
        out_dci,
        in_dci,
        adapter,
        evt_ring,
        regs,
        bar0,
        dboff,
    ) {
        crate::mesa_println!("[GPT] Error leyendo GPT Header");
        return false;
    }

    // Validar firma
    if &header_buf[0..8] != b"EFI PART" {
        crate::mesa_println!("[GPT] Firma GPT inválida");
        return false;
    }

    // Extraer campos del header
    let part_entry_lba = u64::from_le_bytes(header_buf[72..80].try_into().unwrap());
    let num_entries = u32::from_le_bytes(header_buf[80..84].try_into().unwrap());
    let entry_size = u32::from_le_bytes(header_buf[84..88].try_into().unwrap());

    crate::mesa_println!(
        "[GPT] Partición entries en LBA={}, num={}, size={}",
        part_entry_lba,
        num_entries,
        entry_size
    );

    // Buscar un slot libre en la tabla de particiones
    let entries_per_sector = 512 / entry_size as usize;
    let mut found_slot = false;
    let mut target_lba = 0u64;
    let mut target_offset = 0usize;

    for sec in 0..(num_entries as usize + entries_per_sector - 1) / entries_per_sector {
        let lba = part_entry_lba + sec as u64;
        let mut entry_buf = [0u8; 512];

        if !scsi_read_10(
            lba as u32,
            1,
            &mut entry_buf,
            out_ring,
            in_ring,
            slot_id,
            out_dci,
            in_dci,
            adapter,
            evt_ring,
            regs,
            bar0,
            dboff,
        ) {
            crate::mesa_println!("[GPT] Error leyendo sector de entradas LBA {}", lba);
            continue;
        }

        for ei in 0..entries_per_sector {
            let off = ei * entry_size as usize;
            if off + 16 > 512 {
                break;
            }

            let guid: [u8; 16] = entry_buf[off..off + 16].try_into().unwrap();
            if guid == [0u8; 16] {
                // Slot libre encontrado
                found_slot = true;
                target_lba = lba;
                target_offset = off;
                break;
            }
        }

        if found_slot {
            break;
        }
    }

    if !found_slot {
        crate::mesa_println!("[GPT] No hay slots libres en la tabla de particiones");
        return false;
    }

    crate::mesa_println!(
        "[GPT] Slot libre encontrado en LBA={}, offset={}",
        target_lba,
        target_offset
    );

    // Leer el sector nuevamente para modificarlo
    let mut entry_buf = [0u8; 512];
    if !scsi_read_10(
        target_lba as u32,
        1,
        &mut entry_buf,
        out_ring,
        in_ring,
        slot_id,
        out_dci,
        in_dci,
        adapter,
        evt_ring,
        regs,
        bar0,
        dboff,
    ) {
        crate::mesa_println!("[GPT] Error releyendo sector de entradas");
        return false;
    }

    // Crear entrada de partición GPT
    // Partition Type GUID: Basic Data Partition (EBD0A0A2-B9E5-4433-87C0-68B6B72699C7)
    let partition_type_guid: [u8; 16] = [
        0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44, 0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99,
        0xC7,
    ];

    // Unique Partition GUID (generar uno simple basado en LBA)
    let unique_guid: [u8; 16] = {
        let mut guid = [0u8; 16];
        let lba_bytes = start_lba.to_le_bytes();
        guid[0..8].copy_from_slice(&lba_bytes);
        guid[8..12].copy_from_slice(&(slot_id as u32).to_le_bytes());
        guid
    };

    // Escribir la entrada
    entry_buf[target_offset..target_offset + 16].copy_from_slice(&partition_type_guid);
    entry_buf[target_offset + 16..target_offset + 32].copy_from_slice(&unique_guid);
    entry_buf[target_offset + 32..target_offset + 40].copy_from_slice(&start_lba.to_le_bytes());
    entry_buf[target_offset + 40..target_offset + 48].copy_from_slice(&end_lba.to_le_bytes());
    entry_buf[target_offset + 48..target_offset + 56].copy_from_slice(&[0u8; 8]); // Attributes
    entry_buf[target_offset + 56..target_offset + 128].copy_from_slice(&[0u8; 72]); // Partition name (UTF-16LE)

    // Escribir nombre de partición en UTF-16LE
    let name_utf16: Vec<u16> = partition_name.encode_utf16().collect();
    for (i, &c) in name_utf16.iter().enumerate().take(36) {
        if target_offset + 56 + i * 2 + 2 <= 512 {
            entry_buf[target_offset + 56 + i * 2] = (c & 0xFF) as u8;
            entry_buf[target_offset + 56 + i * 2 + 1] = ((c >> 8) & 0xFF) as u8;
        }
    }

    // Escribir el sector modificado
    if !scsi_write_10(
        target_lba as u32,
        1,
        &entry_buf,
        out_ring,
        in_ring,
        slot_id,
        out_dci,
        in_dci,
        adapter,
        evt_ring,
        regs,
        bar0,
        dboff,
    ) {
        crate::mesa_println!("[GPT] Error escribiendo entrada de partición");
        return false;
    }

    crate::mesa_println!("[GPT] ✅ Entrada de partición escrita");

    // Recalcular CRC de la tabla de particiones
    let mut all_entries = alloc::vec::Vec::new();
    for sec in 0..(num_entries as usize + entries_per_sector - 1) / entries_per_sector {
        let lba = part_entry_lba + sec as u64;
        let mut buf = [0u8; 512];
        if scsi_read_10(
            lba as u32, 1, &mut buf, out_ring, in_ring, slot_id, out_dci, in_dci, adapter,
            evt_ring, regs, bar0, dboff,
        ) {
            all_entries.extend_from_slice(&buf);
        }
    }

    let entries_crc = crc32(&all_entries);
    header_buf[88..92].copy_from_slice(&entries_crc.to_le_bytes());

    // Recalcular CRC del header (con CRC de particiones = 0)
    header_buf[16..20].copy_from_slice(&[0u8; 4]);
    let header_crc = crc32(&header_buf[..92]);
    header_buf[16..20].copy_from_slice(&header_crc.to_le_bytes());

    // Escribir el header actualizado
    if !scsi_write_10(
        1,
        1,
        &header_buf,
        out_ring,
        in_ring,
        slot_id,
        out_dci,
        in_dci,
        adapter,
        evt_ring,
        regs,
        bar0,
        dboff,
    ) {
        crate::mesa_println!("[GPT] Error escribiendo GPT Header actualizado");
        return false;
    }

    crate::mesa_println!("[GPT] ✅ GPT Header actualizado con nuevos CRCs");
    true
}

// ---------------------------------------------------------------------------
// Estado global del subsistema xHCI (singleton) para operaciones post-init
// ---------------------------------------------------------------------------
pub static mut USB_STORAGE_STATE: UsbStorageState = UsbStorageState::empty();

#[derive(Copy, Clone)]
pub struct UsbStorageState {
    pub ready: bool,
    pub slot_id: u8,
    pub out_dci: u8,
    pub in_dci: u8,
    pub out_ring_phys: u64,
    pub out_ring_virt: u64,
    pub out_ring_enqueue: usize,
    pub out_ring_cycle: bool,
    pub in_ring_phys: u64,
    pub in_ring_virt: u64,
    pub in_ring_enqueue: usize,
    pub in_ring_cycle: bool,
    pub free_start_lba: u64,
    pub free_sectors: u64,
    pub capacity_sectors: u64,
    pub block_size: u32,
    pub bar0: u64,
    pub dboff: u32,
    pub evt_ring_phys: u64,
    pub evt_ring_virt: u64,
    pub evt_ring_dequeue: usize,
    pub evt_ring_cycle: bool,
    pub caplength: u8,
}

impl UsbStorageState {
    const fn empty() -> Self {
        Self {
            ready: false,
            slot_id: 0,
            out_dci: 0,
            in_dci: 0,
            out_ring_phys: 0,
            out_ring_virt: 0,
            out_ring_enqueue: 0,
            out_ring_cycle: true,
            in_ring_phys: 0,
            in_ring_virt: 0,
            in_ring_enqueue: 0,
            in_ring_cycle: true,
            free_start_lba: 0,
            free_sectors: 0,
            capacity_sectors: 0,
            block_size: 512,
            bar0: 0,
            dboff: 0,
            evt_ring_phys: 0,
            evt_ring_virt: 0,
            evt_ring_dequeue: 0,
            evt_ring_cycle: true,
            caplength: 0,
        }
    }
}

// Reconstruye EventRing desde el estado guardado
unsafe fn rebuild_evt_ring() -> EventRing {
    let s = USB_STORAGE_STATE;
    EventRing {
        base_virt: s.evt_ring_virt as *const u32,
        base_phys: s.evt_ring_phys,
        dequeue: s.evt_ring_dequeue,
        cycle: s.evt_ring_cycle,
    }
}

// Reconstruye Registers desde bar0 guardado
unsafe fn rebuild_regs() -> Registers<MemoryMapper> {
    Registers::new(USB_STORAGE_STATE.bar0 as usize, MemoryMapper)
}

// ---------------------------------------------------------------------------
// UsbBlockDevice — implementa BlockDevice para integrarse con el VFS
// ---------------------------------------------------------------------------
pub struct UsbBlockDevice;

impl crate::drivers::block::BlockDevice for UsbBlockDevice {
    fn read(&self, lba: u64, count: u16, buffer: &mut [u8]) -> Result<(), &'static str> {
        usb_storage_read(lba, count, buffer)
    }

    fn write(&self, lba: u64, count: u16, buffer: &[u8]) -> Result<(), &'static str> {
        usb_storage_write(lba, count, buffer)
    }

    fn capacity(&self) -> u64 {
        unsafe { USB_STORAGE_STATE.capacity_sectors }
    }
}

// ---------------------------------------------------------------------------
// API pública para leer/escribir sectores en el dispositivo USB Mass Storage
// ---------------------------------------------------------------------------
pub fn usb_storage_read(lba: u64, count: u16, buffer: &mut [u8]) -> Result<(), &'static str> {
    let state = unsafe { USB_STORAGE_STATE };
    if !state.ready {
        return Err("USB storage not ready");
    }
    let total_bytes = (count as u64) * 512;
    if (buffer.len() as u64) < total_bytes {
        return Err("buffer too small");
    }

    let max_per_call = 256u16;
    let mut remaining = count;
    let mut current_lba = lba;
    let mut offset = 0usize;

    unsafe {
        while remaining > 0 {
            let batch = remaining.min(max_per_call);
            let chunk = &mut buffer[offset..offset + (batch as usize) * 512];
            let idx = (state.slot_id as usize).min(MAX_USB_DEVICES - 1);

            let mut out_ring = TrbRing {
                base_virt: USB_DEVICES[idx].bulk_out_ring_virt as *mut u32,
                base_phys: USB_DEVICES[idx].bulk_out_ring_phys,
                enqueue: USB_STORAGE_STATE.out_ring_enqueue,
                cycle: USB_STORAGE_STATE.out_ring_cycle,
            };
            let mut in_ring = TrbRing {
                base_virt: USB_DEVICES[idx].bulk_in_ring_virt as *mut u32,
                base_phys: USB_DEVICES[idx].bulk_in_ring_phys,
                enqueue: USB_STORAGE_STATE.in_ring_enqueue,
                cycle: USB_STORAGE_STATE.in_ring_cycle,
            };
            crate::mesa_println!(
                "[DEBUG EP RINGS] usb_storage_read: out_phys={:#x} in_phys={:#x} out_virt={:p} in_virt={:p}",
                out_ring.base_phys,
                in_ring.base_phys,
                out_ring.base_virt,
                in_ring.base_virt
            );
            let mut evt = rebuild_evt_ring();
            let mut regs = rebuild_regs();

            if !scsi_read_10(
                current_lba as u32,
                batch,
                chunk,
                &mut out_ring,
                &mut in_ring,
                state.slot_id,
                state.out_dci,
                state.in_dci,
                &mut PmmXhciAdapter,
                &mut evt,
                &mut regs,
                state.bar0,
                state.dboff,
            ) {
                return Err("USB storage read failed");
            }

            // Persistir ring state para la próxima llamada
            USB_STORAGE_STATE.out_ring_enqueue = out_ring.enqueue;
            USB_STORAGE_STATE.out_ring_cycle = out_ring.cycle;
            USB_STORAGE_STATE.in_ring_enqueue = in_ring.enqueue;
            USB_STORAGE_STATE.in_ring_cycle = in_ring.cycle;
            USB_STORAGE_STATE.evt_ring_dequeue = evt.dequeue;
            USB_STORAGE_STATE.evt_ring_cycle = evt.cycle;

            remaining -= batch;
            current_lba += batch as u64;
            offset += (batch as usize) * 512;
        }
    }

    Ok(())
}

pub fn usb_storage_write(lba: u64, count: u16, buffer: &[u8]) -> Result<(), &'static str> {
    let state = unsafe { USB_STORAGE_STATE };
    if !state.ready {
        return Err("USB storage not ready");
    }
    let total_bytes = (count as u64) * 512;
    if (buffer.len() as u64) < total_bytes {
        return Err("buffer too small");
    }

    let max_per_call = 256u16;
    let mut remaining = count;
    let mut current_lba = lba;
    let mut offset = 0usize;

    unsafe {
        while remaining > 0 {
            let batch = remaining.min(max_per_call);
            let chunk = &buffer[offset..offset + (batch as usize) * 512];
            let idx = (state.slot_id as usize).min(MAX_USB_DEVICES - 1);

            let mut out_ring = TrbRing {
                base_virt: USB_DEVICES[idx].bulk_out_ring_virt as *mut u32,
                base_phys: USB_DEVICES[idx].bulk_out_ring_phys,
                enqueue: USB_STORAGE_STATE.out_ring_enqueue,
                cycle: USB_STORAGE_STATE.out_ring_cycle,
            };
            let mut in_ring = TrbRing {
                base_virt: USB_DEVICES[idx].bulk_in_ring_virt as *mut u32,
                base_phys: USB_DEVICES[idx].bulk_in_ring_phys,
                enqueue: USB_STORAGE_STATE.in_ring_enqueue,
                cycle: USB_STORAGE_STATE.in_ring_cycle,
            };
            crate::mesa_println!(
                "[DEBUG EP RINGS] usb_storage_write: out_phys={:#x} in_phys={:#x}",
                out_ring.base_phys,
                in_ring.base_phys
            );
            let mut evt = rebuild_evt_ring();
            let mut regs = rebuild_regs();

            if !scsi_write_10(
                current_lba as u32,
                batch,
                chunk,
                &mut out_ring,
                &mut in_ring,
                state.slot_id,
                state.out_dci,
                state.in_dci,
                &mut PmmXhciAdapter,
                &mut evt,
                &mut regs,
                state.bar0,
                state.dboff,
            ) {
                return Err("USB storage write failed");
            }

            USB_STORAGE_STATE.out_ring_enqueue = out_ring.enqueue;
            USB_STORAGE_STATE.out_ring_cycle = out_ring.cycle;
            USB_STORAGE_STATE.in_ring_enqueue = in_ring.enqueue;
            USB_STORAGE_STATE.in_ring_cycle = in_ring.cycle;
            USB_STORAGE_STATE.evt_ring_dequeue = evt.dequeue;
            USB_STORAGE_STATE.evt_ring_cycle = evt.cycle;

            remaining -= batch;
            current_lba += batch as u64;
            offset += (batch as usize) * 512;
        }
    }

    Ok(())
}

pub fn usb_storage_free_space_info() -> Option<(u64, u64)> {
    let state = unsafe { USB_STORAGE_STATE };
    if !state.ready || state.free_sectors == 0 {
        return None;
    }
    Some((state.free_start_lba, state.free_sectors))
}

// ---------------------------------------------------------------------------
// Punto de entrada principal
// ---------------------------------------------------------------------------
pub fn init(dev: &PciDevice) {
    xhci_log_init();
    xhci_log(format_args!("[xHCI] Inicializando driver Nativo Rust..."));

    crate::pci::pci_enable_memory_space(dev.bus, dev.device, dev.function);
    crate::pci::pci_enable_bus_mastering(dev.bus, dev.device, dev.function);

    xhci_log(format_args!(
        "[xHCI] Leyendo BAR0 y BAR1 para dirección de 64 bits..."
    ));

    // Leer BAR0 y BAR1 explícitamente para formar dirección de 64 bits
    let bar0_low = crate::pci::pci_config_read(dev.bus, dev.device, dev.function, 0x10);
    let bar1_high = crate::pci::pci_config_read(dev.bus, dev.device, dev.function, 0x14);

    xhci_log(format_args!(
        "[xHCI] BAR0 (low): {:#x}, BAR1 (high): {:#x}",
        bar0_low, bar1_high
    ));

    // Verificar si BAR0 indica que es de 64 bits (bit 2-3 = 0b10)
    let is_64bit = ((bar0_low >> 1) & 0x03) == 0x02;
    xhci_log(format_args!("[xHCI] BAR0 es de 64 bits: {}", is_64bit));

    // Combinar BAR0 y BAR1 para formar dirección de 64 bits
    let bar0_address = if is_64bit {
        let address = ((bar1_high as u64) << 32) | ((bar0_low & 0xFFFFFFF0) as u64);
        xhci_log(format_args!(
            "[xHCI] Dirección combinada BAR0+BAR1: {:#x}",
            address
        ));
        address
    } else {
        let address = (bar0_low & 0xFFFFFFF0) as u64;
        xhci_log(format_args!(
            "[xHCI] Dirección BAR0 (32 bits): {:#x}",
            address
        ));
        address
    };

    xhci_log(format_args!(
        "[xHCI] Usando dirección BAR: {:#x}",
        bar0_address
    ));
    unsafe {
        XHCI_BAR0 = Some(bar0_address);
    }

    let mapper = MemoryMapper;
    let mut regs = unsafe { Registers::new(bar0_address as usize, mapper) };

    // Guardar bar0_address como bar0 para uso posterior en el código
    let bar0 = bar0_address;

    // 1. Reset
    crate::mesa_println!("[xHCI] Reseteando HC...");
    unsafe {
        let mut cmd = regs.operational.usbcmd.read_volatile();
        cmd.clear_run_stop();
        regs.operational.usbcmd.write_volatile(cmd);
        while !regs.operational.usbsts.read_volatile().hc_halted() {
            core::hint::spin_loop();
        }
        let mut cmd = regs.operational.usbcmd.read_volatile();
        cmd.set_host_controller_reset();
        regs.operational.usbcmd.write_volatile(cmd);
        while regs
            .operational
            .usbcmd
            .read_volatile()
            .host_controller_reset()
        {
            core::hint::spin_loop();
        }
        while regs
            .operational
            .usbsts
            .read_volatile()
            .controller_not_ready()
        {
            core::hint::spin_loop();
        }
    }
    crate::mesa_println!("[xHCI] HC Reseteado.");

    // 2. Capacidades y Slots
    let hcs1 = regs.capability.hcsparams1.read_volatile();
    let max_slots = hcs1.number_of_device_slots();
    let num_ports = hcs1.number_of_ports();
    let dboff = regs.capability.dboff.read_volatile().get();

    // Leer CSZ (Context Size) de HCCPARAMS1 antes de cualquier operación de contextos
    let hcc = regs.capability.hccparams1.read_volatile();
    let csz = hcc.context_size();
    unsafe {
        XHCI_CONTEXT_SIZE = if csz { 64 } else { 32 };
    }
    crate::mesa_println!(
        "[xHCI] Slots={}, Puertos={}, DBOFF={:#x}, CSZ={} (ctx={} bytes)",
        max_slots,
        num_ports,
        dboff,
        csz,
        unsafe { XHCI_CONTEXT_SIZE }
    );

    let mut config = regs.operational.config.read_volatile();
    config.set_max_device_slots_enabled(max_slots);
    regs.operational.config.write_volatile(config);

    let mut adapter = PmmXhciAdapter;
    use crate::memory::xhci_access::XhciMemory;

    // 3. DCBAA
    let dcbaa_phys = match adapter.alloc_64byte((max_slots as usize + 1) * 8) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ERROR: DCBAA");
            return;
        }
    };
    let dcbaa_virt = adapter.virt_from_phys(dcbaa_phys) as *mut u64;
    unsafe {
        core::ptr::write_bytes(dcbaa_virt as *mut u8, 0, (max_slots as usize + 1) * 8);
    }
    let mut dcbaap = regs.operational.dcbaap.read_volatile();
    dcbaap.set(dcbaa_phys.as_u64());
    regs.operational.dcbaap.write_volatile(dcbaap);
    crate::mesa_println!("[xHCI] DCBAA @ phys={:#x}", dcbaa_phys.as_u64());

    // 4. Command Ring
    let cr_phys = match adapter.alloc_64byte(RING_SIZE * 16) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ERROR: Command Ring");
            return;
        }
    };
    let cr_virt = adapter.virt_from_phys(cr_phys) as *mut u32;
    let mut cmd_ring = TrbRing::new(cr_phys.as_u64(), cr_virt);
    let mut crcr = regs.operational.crcr.read_volatile();
    crcr.set_ring_cycle_state();
    crcr.set_command_ring_pointer(cr_phys.as_u64());
    regs.operational.crcr.write_volatile(crcr);
    crate::mesa_println!("[xHCI] Command Ring @ phys={:#x}", cr_phys.as_u64());

    // 5. Event Ring + ERST
    let er_phys = match adapter.alloc_64byte(RING_SIZE * 16) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ERROR: Event Ring");
            return;
        }
    };
    let erst_phys = match adapter.alloc_64byte(64) {
        Some(p) => p,
        None => {
            crate::mesa_println!("[xHCI] ERROR: ERST");
            return;
        }
    };
    let er_virt = adapter.virt_from_phys(er_phys) as *const u32;
    let mut evt_ring = EventRing::new(er_phys.as_u64(), er_virt);

    let erst_virt = adapter.virt_from_phys(erst_phys) as *mut u64;
    unsafe {
        core::ptr::write_volatile(erst_virt, er_phys.as_u64());
        core::ptr::write_volatile(erst_virt.add(1), RING_SIZE as u64);
    }
    let mut ir = regs.interrupter_register_set.interrupter_mut(0);
    let mut sz = ir.erstsz.read_volatile();
    sz.set(1);
    ir.erstsz.write_volatile(sz);
    let mut ba = ir.erstba.read_volatile();
    ba.set(erst_phys.as_u64());
    ir.erstba.write_volatile(ba);
    let mut dp = ir.erdp.read_volatile();
    dp.set_event_ring_dequeue_pointer(er_phys.as_u64());
    ir.erdp.write_volatile(dp);
    crate::mesa_println!("[xHCI] Event Ring @ phys={:#x}", er_phys.as_u64());

    // 6. Arrancar HC
    let mut cmd = regs.operational.usbcmd.read_volatile();
    cmd.set_run_stop();
    regs.operational.usbcmd.write_volatile(cmd);
    unsafe {
        while regs.operational.usbsts.read_volatile().hc_halted() {
            core::hint::spin_loop();
        }
    }
    crate::mesa_println!("[xHCI] HC corriendo.");
    unsafe {
        USB_HC_RUNNING = true;
    }

    let caplength = unsafe { core::ptr::read_volatile(bar0 as *const u8) };
    let op_base = bar0 + caplength as u64;
    let port_base = op_base + 0x400;

    // 7. Escaneo de puertos + Enable Slot + Address Device
    xhci_log(format_args!(
        "[xHCI] Iniciando escaneo de {} puertos...",
        num_ports
    ));
    xhci_log(format_args!("[xHCI] Port base address: {:#x}", port_base));

    for i in 0..num_ports {
        xhci_log(format_args!("[xHCI] Escaneando puerto {}...", i + 1));

        let portsc_ptr = (port_base + (i as u64 * 0x10)) as *mut u32;
        let mut portsc_val = unsafe { core::ptr::read_volatile(portsc_ptr) };

        xhci_log(format_args!(
            "[xHCI] Puerto {}: PORTSC raw value = {:#x}",
            i + 1,
            portsc_val
        ));
        xhci_log(format_args!(
            "[xHCI] Puerto {}: CCS (bit 0) = {}, PED (bit 1) = {}, PR (bit 4) = {}",
            i + 1,
            (portsc_val & 1) != 0,
            (portsc_val & (1 << 1)) != 0,
            (portsc_val & (1 << 4)) != 0
        ));

        // Bit 0: Current Connect Status (CCS)
        if (portsc_val & 1) == 0 {
            xhci_log(format_args!(
                "[xHCI] Puerto {}: Sin dispositivo conectado (CCS=0)",
                i + 1
            ));
            continue;
        }

        xhci_log(format_args!(
            "[xHCI] Puerto {}: Dispositivo conectado detectado (CCS=1)",
            i + 1
        ));

        // PLS=4 = Disabled, PLS=8 = Compliance Mode
        // Si está en uno de estos estados, no hacemos Warm Reset preventivo
        // porque aún no conocemos la velocidad (podría ser USB 2.0).
        // El standard Port Reset + recovery post-reset lo manejarán.
        let _pls_init = ((portsc_val >> 5) & 0xF) as u8;

        // Clear W1C bits correctamente (PED bit 1 + change bits 17-23 son RW1C)
        // NO reescribir el valor leído directamente, limpiar RW1C con máscara
        let w1c_mask = (1 << 1) | 0xFE0000; // PED (bit 1) + Change bits 17-23
        let portsc_write = (portsc_val & !w1c_mask) | (1 << 4); // Clear RW1C bits, set PR
        xhci_log(format_args!(
            "[xHCI] Puerto {}: Escribiendo PORTSC para iniciar reset: {:#x}",
            i + 1,
            portsc_write
        ));
        xhci_log(format_args!(
            "[xHCI] Puerto {}: W1C mask = {:#x}, PR bit set",
            i + 1,
            w1c_mask
        ));
        unsafe {
            core::ptr::write_volatile(portsc_ptr, portsc_write);
        }

        xhci_log(format_args!(
            "[xHCI] Puerto {}: Iniciando Port Reset...",
            i + 1
        ));

        // Esperar a que termine el Reset (PRC bit 21 o PED bit 1)
        // Timeout: 100 ms según especificación xHCI
        let mut speed = 0;
        let start = read_tsc();
        let target = ms_to_spins(100);

        loop {
            let val = unsafe { core::ptr::read_volatile(portsc_ptr) };
            xhci_log(format_args!(
                "[xHCI] Puerto {}: PORTSC durante reset = {:#x}",
                i + 1,
                val
            ));
            xhci_log(format_args!(
                "[xHCI] Puerto {}: PRC (bit 21) = {}, PED (bit 1) = {}",
                i + 1,
                (val >> 21) & 1,
                (val >> 1) & 1
            ));

            if (val & (1 << 21)) != 0 || (val & (1 << 1)) != 0 {
                speed = ((val >> 10) & 0xF) as u8;
                xhci_log(format_args!(
                    "[xHCI] Puerto {}: Reset completado, speed = {}",
                    i + 1,
                    speed
                ));
                // Clear PRC (RW1C) preservando PP: limpiar RW1C y setear solo PRC
                let mut prc_clear = unsafe { core::ptr::read_volatile(portsc_ptr) };
                prc_clear &= !((1 << 1) | 0xFE0000); // PED + change bits a 0
                prc_clear |= 1 << 21; // Solo PRC
                xhci_log(format_args!(
                    "[xHCI] Puerto {}: Limpiando PRC bit (read-modify-write: {:#x})",
                    i + 1,
                    prc_clear
                ));
                unsafe {
                    core::ptr::write_volatile(portsc_ptr, prc_clear);
                }
                break;
            }
            if read_tsc() - start >= target {
                xhci_log(format_args!(
                    "[xHCI] ❌ ERROR: Puerto {} timeout esperando Port Reset (100 ms)",
                    i + 1
                ));
                xhci_log(format_args!(
                    "[xHCI] ❌ Puerto {}: Último PORTSC = {:#x}",
                    i + 1,
                    unsafe { core::ptr::read_volatile(portsc_ptr) }
                ));
                break;
            }
            core::hint::spin_loop();
        }

        if speed == 0 {
            xhci_log(format_args!(
                "[xHCI] ❌ ERROR: Puerto {} falló el reset (speed=0), saltando al siguiente puerto",
                i + 1
            ));
            continue; // Falló el reset
        }

        xhci_log(format_args!(
            "[xHCI] ✅ Puerto {} reseteado exitosamente (speed={})",
            i + 1,
            speed
        ));

        // Verificar estado del puerto después del reset
        let portsc_after = unsafe { core::ptr::read_volatile(portsc_ptr) };
        let ped = (portsc_after & (1 << 1)) != 0; // Port Enabled/Disabled
        let ccs = (portsc_after & 1) != 0; // Current Connect Status
        let pls_after = ((portsc_after >> 5) & 0xF) as u8; // Port Link State (4 bits)

        xhci_log(format_args!(
            "[xHCI] Puerto {} estado después de reset: CCS={}, PED={}, PLS={}",
            i + 1,
            ccs,
            ped,
            pls_after
        ));
        xhci_log(format_args!(
            "[xHCI] Puerto {}: PORTSC después de reset = {:#x}",
            i + 1,
            portsc_after
        ));

        // Verificar si está en estado inválido después del reset
        // PLS=4 = Disabled, PLS=8 = Compliance Mode
        if pls_after == 8 {
            xhci_log(format_args!(
                "[xHCI] ❌ ERROR: Puerto {} entró en Compliance Mode (PLS=8) después del reset",
                i + 1
            ));
        }

        if !ped || pls_after == 4 || pls_after == 8 {
            if speed >= 4 {
                // ── SuperSpeed (USB 3.x) → Warm Reset (WPR bit 28) ──
                xhci_log(format_args!(
                    "[xHCI] ⚠️  Puerto {} (SS): no habilitado (PED={}, PLS={}). Warm Reset...",
                    i + 1,
                    ped,
                    pls_after
                ));
                let mut recovered = false;
                for wr_attempt in 0..3 {
                    // WPR set: limpiar RW1C y setear solo WPR
                    let mut wr_val = unsafe { core::ptr::read_volatile(portsc_ptr) };
                    wr_val &= !((1 << 1) | 0xFE0000);
                    wr_val |= 1 << 28;
                    unsafe {
                        core::ptr::write_volatile(portsc_ptr, wr_val);
                    }
                    delay_ms(10);
                    let wr_start = read_tsc();
                    let wr_target = ms_to_spins(200);
                    loop {
                        let val = unsafe { core::ptr::read_volatile(portsc_ptr) };
                        if (val & ((1 << 19) | (1 << 21))) != 0 {
                            // WRC+PRC clear: limpiar RW1C y setear solo WRC+PRC
                            let mut wrc_val = unsafe { core::ptr::read_volatile(portsc_ptr) };
                            wrc_val &= !((1 << 1) | 0xFE0000);
                            wrc_val |= (1 << 19) | (1 << 21);
                            unsafe {
                                core::ptr::write_volatile(portsc_ptr, wrc_val);
                            }
                            break;
                        }
                        if read_tsc() - wr_start >= wr_target {
                            break;
                        }
                        core::hint::spin_loop();
                    }
                    delay_ms(100);
                    let p = unsafe { core::ptr::read_volatile(portsc_ptr) };
                    if (p & (1 << 1)) != 0 && (p & 1) != 0 {
                        recovered = true;
                        speed = ((p >> 10) & 0xF) as u8;
                        xhci_log(format_args!(
                            "[xHCI] ✅ Puerto {} SS recuperado por Warm Reset",
                            i + 1
                        ));
                        break;
                    }
                }
                if !recovered {
                    xhci_log(format_args!(
                        "[xHCI] ❌ Puerto {} SS: Warm Reset falló",
                        i + 1
                    ));
                    continue;
                }
            } else {
                // ── USB 2.0 (FS/LS/HS) → Port Power Cycle (PP bit 9) ──
                // En AMD Renoir, Warm Reset en USB 2.0 empeora el estado.
                // Power cycling forza al PHY a reiniciar desde Rx.Detect.
                xhci_log(format_args!(
                    "[xHCI] ⚠️  Puerto {} (USB 2.0, speed={}): Power Cycle...",
                    i + 1,
                    speed
                ));

                // 1. Power OFF: clear PP (bit 9)
                let mut ps = unsafe { core::ptr::read_volatile(portsc_ptr) };
                ps &= !(1 << 9);
                unsafe {
                    core::ptr::write_volatile(portsc_ptr, ps);
                }
                delay_ms(30);

                // 2. Power ON: set PP (bit 9)
                ps = unsafe { core::ptr::read_volatile(portsc_ptr) };
                ps |= (1 << 9);
                unsafe {
                    core::ptr::write_volatile(portsc_ptr, ps);
                }
                delay_ms(150);

                // 3. Verificar si el dispositivo fue re-detectado
                ps = unsafe { core::ptr::read_volatile(portsc_ptr) };
                if (ps & 1) == 0 {
                    xhci_log(format_args!(
                        "[xHCI] ❌ Puerto {}: dispositivo no reapareció tras Power Cycle",
                        i + 1
                    ));
                    continue;
                }
                xhci_log(format_args!(
                    "[xHCI] ✅ Puerto {}: dispositivo re-detectado tras Power Cycle",
                    i + 1
                ));

                // 4. Re-intentar reset estándar (PR bit 4)
                let w1c_mask = (1 << 1) | 0xFE0000;
                let reset_val = (ps & !w1c_mask) | (1 << 4);
                unsafe {
                    core::ptr::write_volatile(portsc_ptr, reset_val);
                }

                let start = read_tsc();
                let target = ms_to_spins(100);
                speed = 0;
                loop {
                    let val = unsafe { core::ptr::read_volatile(portsc_ptr) };
                    if (val & ((1 << 21) | (1 << 1))) != 0 {
                        speed = ((val >> 10) & 0xF) as u8;
                        // PRC clear: limpiar RW1C y setear solo PRC
                        let mut prc_clear = unsafe { core::ptr::read_volatile(portsc_ptr) };
                        prc_clear &= !((1 << 1) | 0xFE0000);
                        prc_clear |= 1 << 21;
                        unsafe {
                            core::ptr::write_volatile(portsc_ptr, prc_clear);
                        }
                        break;
                    }
                    if read_tsc() - start >= target {
                        break;
                    }
                    core::hint::spin_loop();
                }
                if speed == 0 {
                    xhci_log(format_args!(
                        "[xHCI] ❌ Puerto {}: reset tras Power Cycle falló",
                        i + 1
                    ));
                    continue;
                }
                xhci_log(format_args!(
                    "[xHCI] ✅ Puerto {}: reset tras Power Cycle OK (speed={})",
                    i + 1,
                    speed
                ));

                // 5. Verificar PED después del segundo reset
                ps = unsafe { core::ptr::read_volatile(portsc_ptr) };
                if (ps & (1 << 1)) == 0 {
                    xhci_log(format_args!(
                        "[xHCI] ❌ Puerto {}: PED=0 incluso tras Power Cycle + reset",
                        i + 1
                    ));
                    continue;
                }
            }
        } else {
            xhci_log(format_args!(
                "[xHCI] ✅ Puerto {} habilitado correctamente después del reset",
                i + 1
            ));
        }

        // Retraso para que el dispositivo se estabilice tras el reset
        // Según spec: mínimo 50 ms para pulso de reset, hasta 100 ms para confirmación
        crate::mesa_println!(
            "[xHCI] Puerto {} esperando estabilización del dispositivo (100 ms)...",
            i + 1
        );
        delay_ms(100);

        // Enable Slot (Type=9) con reintentos
        crate::mesa_println!("[xHCI] Puerto {}: Iniciando Enable Slot...", i + 1);

        let mut slot_id = None;
        for attempt in 0..3 {
            cmd_ring.enqueue_trb([0, 0, 0, 9u32 << 10]);
            ring_hc_doorbell_target(bar0 as u64, dboff, 0, 0); // Slot=0 = HC Command

            crate::mesa_println!(
                "[xHCI] Puerto {}: Esperando Command Completion (Enable Slot) intento {}...",
                i + 1,
                attempt + 1
            );
            match wait_for_completion(&mut evt_ring, &mut regs, 5000) {
                Some((sid, 1, _)) => {
                    crate::mesa_println!(
                        "[xHCI] Puerto {}: Slot habilitado exitosamente: Slot ID = {}",
                        i + 1,
                        sid
                    );
                    slot_id = Some(sid);
                    break;
                }
                Some((_, cc, _)) => {
                    crate::mesa_println!(
                        "[xHCI] Puerto {}: Enable Slot falló en intento {}. CC = {} ({})",
                        i + 1,
                        attempt + 1,
                        cc,
                        completion_code_to_string(cc)
                    );
                    if attempt < 2 {
                        crate::mesa_println!(
                            "[xHCI] Puerto {}: Reintentando Enable Slot...",
                            i + 1
                        );
                        for _ in 0..50_000_000 {
                            core::hint::spin_loop();
                        }
                    }
                }
                None => {
                    crate::mesa_println!(
                        "[xHCI] Puerto {}: Timeout esperando Enable Slot (200M spins) intento {}",
                        i + 1,
                        attempt + 1
                    );
                    if attempt < 2 {
                        crate::mesa_println!(
                            "[xHCI] Puerto {}: Reintentando Enable Slot...",
                            i + 1
                        );
                        for _ in 0..50_000_000 {
                            core::hint::spin_loop();
                        }
                    }
                }
            }
        }

        let slot_id = match slot_id {
            Some(sid) => sid,
            None => {
                crate::mesa_println!("[xHCI] ERROR: Puerto {} Enable Slot falló después de 3 intentos, saltando al siguiente puerto", i + 1);
                continue;
            }
        };

        // Registrar dispositivo en USB_DEVICES
        unsafe {
            let idx = (slot_id as usize).min(MAX_USB_DEVICES - 1);
            USB_DEVICES[idx].active = true;
            USB_DEVICES[idx].slot_id = slot_id;
            USB_DEVICES[idx].port = i as u8;
            USB_DEVICES[idx].speed = speed;
            USB_DEVICES[idx].phase = UsbPhase::SlotEnabled;
        }

        crate::mesa_println!("[xHCI] Puerto {}: Iniciando Address Device...", i + 1);

        // Address Device
        address_device(
            slot_id,
            i as u8,
            speed,
            dcbaa_virt,
            &mut adapter,
            &mut cmd_ring,
            &mut evt_ring,
            &mut regs,
            bar0 as u64,
            dboff,
        );
    }

    // Flush logs a archivo al finalizar la inicialización
    xhci_log_flush_to_file();

    crate::mesa_println!("[xHCI] Inicialización finalizada.");
}

// ---------------------------------------------------------------------------
// Imprime el estado actual del controlador xHCI y los dispositivos conectados
// ---------------------------------------------------------------------------
pub fn print_usb_status() {
    use crate::drivers::framebuffer::set_color;
    use crate::palette;

    let bar0 = match unsafe { XHCI_BAR0 } {
        Some(addr) => addr,
        None => {
            set_color(palette::ERROR);
            crate::mesa_println!("  [✗] Controlador xHCI no inicializado.");
            set_color(palette::TEXT);
            return;
        }
    };

    let mapper = MemoryMapper;
    let mut regs = unsafe { Registers::new(bar0 as usize, mapper) };

    let hcs1 = regs.capability.hcsparams1.read_volatile();
    let _hcs2 = regs.capability.hcsparams2.read_volatile();
    let hcc = regs.capability.hccparams1.read_volatile();
    let csz = hcc.context_size();
    unsafe {
        XHCI_CONTEXT_SIZE = if csz { 64 } else { 32 };
    }
    crate::mesa_println!(
        "[xHCI] HCCPARAMS1 CSZ: {} (context size = {} bytes)",
        csz,
        unsafe { XHCI_CONTEXT_SIZE }
    );
    let num_ports = hcs1.number_of_ports();
    let max_slots = hcs1.number_of_device_slots();
    let dboff = regs.capability.dboff.read_volatile().get();

    let usbsts = regs.operational.usbsts.read_volatile();
    let hc_halted = usbsts.hc_halted();
    let hc_error = usbsts.host_system_error();
    let hc_running = !hc_halted;

    // ── Sección cabecera ─────────────────────────────────────────────────────
    set_color(palette::FOAM);
    crate::mesa_println!("╔══════════════════════════════════════════════╗");
    crate::mesa_println!("║         ESTADO USB xHCI — MesaOS             ║");
    crate::mesa_println!("╚══════════════════════════════════════════════╝");
    set_color(palette::TEXT);
    crate::mesa_println!();

    // ── Controlador ──────────────────────────────────────────────────────────
    set_color(palette::FOAM);
    crate::mesa_println!("  ┌─ Controlador xHCI ─────────────────────────");
    set_color(palette::TEXT);

    crate::mesa_print!("  │  BAR0:         ");
    set_color(palette::GOLD);
    crate::mesa_println!("{:#x}", bar0);
    set_color(palette::TEXT);

    crate::mesa_print!("  │  Estado HC:    ");
    if hc_running && !hc_error {
        set_color(palette::SUCCESS);
        crate::mesa_println!("✓ RUNNING");
    } else if hc_error {
        set_color(palette::ERROR);
        crate::mesa_println!("✗ HOST SYSTEM ERROR");
    } else {
        set_color(palette::GOLD);
        crate::mesa_println!("⚠ HALTED");
    }
    set_color(palette::TEXT);

    crate::mesa_print!("  │  Max Slots:    ");
    set_color(palette::SUBTLE);
    crate::mesa_println!("{}", max_slots);
    set_color(palette::TEXT);

    crate::mesa_print!("  │  Puertos:      ");
    set_color(palette::SUBTLE);
    crate::mesa_println!("{}", num_ports);
    set_color(palette::TEXT);

    crate::mesa_print!("  │  DBOFF:        ");
    set_color(palette::SUBTLE);
    crate::mesa_println!("{:#x}", dboff);
    set_color(palette::TEXT);

    crate::mesa_print!("  │  64-bit:       ");
    set_color(palette::SUBTLE);
    crate::mesa_println!(
        "{}",
        if hcc.addressing_capability() {
            "Sí"
        } else {
            "No"
        }
    );
    set_color(palette::TEXT);

    crate::mesa_print!("  │  Scratchpad:   ");
    set_color(palette::SUBTLE);
    crate::mesa_println!("(ver HCS2 raw)");
    set_color(palette::TEXT);

    crate::mesa_println!("  └────────────────────────────────────────────");
    crate::mesa_println!();

    // ── Puertos físicos ───────────────────────────────────────────────────────
    set_color(palette::FOAM);
    crate::mesa_println!("  ┌─ Puertos físicos ──────────────────────────");
    set_color(palette::TEXT);

    let mut connected = 0u8;
    for i in 0..num_ports {
        let port = regs.port_register_set.read_volatile_at(i as usize);
        let portsc = port.portsc;
        let ccs = portsc.current_connect_status();
        let ped = portsc.port_enabled_disabled();
        let spd = portsc.port_speed();

        let speed_str = match spd {
            1 => "Full Speed (12 Mb/s)",
            2 => "Low Speed  (1.5 Mb/s)",
            3 => "High Speed (480 Mb/s)",
            4 => "SuperSpeed (5 Gb/s)",
            5 => "SuperSpeed+ (10 Gb/s)",
            _ => "Desconocida",
        };

        crate::mesa_print!("  │  Puerto {:2}:  ", i + 1);
        if ccs {
            connected += 1;
            set_color(palette::SUCCESS);
            crate::mesa_print!("● CONECTADO ");
            set_color(palette::GOLD);
            crate::mesa_print!("[{}]", speed_str);
            if ped {
                set_color(palette::SUCCESS);
                crate::mesa_print!(" HABILITADO");
            } else {
                set_color(palette::GOLD);
                crate::mesa_print!(" DESHABILITADO");
            }
        } else {
            set_color(palette::MUTED);
            crate::mesa_print!("○ vacío");
        }
        set_color(palette::TEXT);
        crate::mesa_println!();
    }

    crate::mesa_println!("  └────────────────────────────────────────────");
    crate::mesa_println!();

    // ── Dispositivos enumerados ───────────────────────────────────────────────
    set_color(palette::FOAM);
    crate::mesa_println!("  ┌─ Dispositivos enumerados ───────────────────");
    set_color(palette::TEXT);

    let devices = unsafe { &USB_DEVICES };
    let mut found_any = false;

    for dev in devices.iter() {
        if !dev.active {
            continue;
        }
        found_any = true;

        let speed_str = match dev.speed {
            1 => "FS",
            2 => "LS",
            3 => "HS",
            4 => "SS",
            5 => "SS+",
            _ => "?",
        };

        let phase_str = match dev.phase {
            UsbPhase::Detected => "Detectado",
            UsbPhase::SlotEnabled => "Slot OK",
            UsbPhase::AddressAssigned => "Dirección asignada",
            UsbPhase::DeviceDescriptorRead => "DevDesc leído",
            UsbPhase::ConfigDescriptorRead => "CfgDesc leído",
            UsbPhase::EndpointsConfigured => "Endpoints config.",
            UsbPhase::SetConfigurationDone => "✓ COMPLETAMENTE OPERATIVO",
            UsbPhase::Failed => "✗ FALLO",
        };

        let phase_color = match dev.phase {
            UsbPhase::SetConfigurationDone => palette::SUCCESS,
            UsbPhase::Failed => palette::ERROR,
            _ => palette::GOLD,
        };

        set_color(palette::FOAM);
        crate::mesa_print!(
            "  │  Slot {:2} │ Puerto {:2} │ {} │ ",
            dev.slot_id,
            dev.port + 1,
            speed_str
        );
        set_color(phase_color);
        crate::mesa_println!("{}", phase_str);
        set_color(palette::TEXT);

        if dev.vendor_id != 0 || dev.product_id != 0 {
            crate::mesa_print!("  │     VID:PID = ");
            set_color(palette::GOLD);
            crate::mesa_println!("{:#06X}:{:#06X}", dev.vendor_id, dev.product_id);
            set_color(palette::TEXT);
        }

        if dev.class != 0 {
            let class_str = match dev.class {
                0x00 => "(clase definida por interfaz)",
                0x01 => "Audio",
                0x02 => "CDC/Comunicaciones",
                0x03 => "HID (Teclado/Ratón)",
                0x05 => "Physical",
                0x06 => "Imagen",
                0x07 => "Impresora",
                0x08 => "Mass Storage (Disco)",
                0x09 => "Hub USB",
                0x0A => "CDC-Data",
                0x0B => "Smart Card",
                0x0E => "Video",
                0x0F => "Personal Healthcare",
                0x10 => "Audio/Video",
                0xE0 => "Wireless Controller",
                0xEF => "Misceláneo",
                0xFF => "Vendor-Specific",
                _ => "Desconocido",
            };
            crate::mesa_print!("  │     Clase:  ");
            set_color(palette::IRIS);
            crate::mesa_println!("{:#04X} — {}", dev.class, class_str);
            set_color(palette::TEXT);
        }

        crate::mesa_print!("  │     Interfaces: ");
        set_color(palette::SUBTLE);
        crate::mesa_print!("{}", dev.num_interfaces);
        set_color(palette::TEXT);
        crate::mesa_print!("  │  Endpoints: ");
        set_color(palette::SUBTLE);
        crate::mesa_println!("{}", dev.num_endpoints);
        set_color(palette::TEXT);

        if dev.config_val != 0 {
            crate::mesa_print!("  │     Configuración activa: ");
            set_color(palette::SUCCESS);
            crate::mesa_println!("{}", dev.config_val);
            set_color(palette::TEXT);
        }

        if dev.num_partitions > 0 {
            crate::mesa_print!("  │     📋 Particiones:");
            crate::mesa_println!();
            for p in 0..dev.num_partitions as usize {
                let ptype = dev.part_type[p];
                let type_str = match ptype {
                    0x0B | 0x0C => "FAT32",
                    0x83 => "Linux ext",
                    0x07 => "NTFS/exFAT",
                    _ => "desconocido",
                };
                set_color(palette::GOLD);
                crate::mesa_print!(
                    "  │       #{:<2}  {:#04x} {:<12}  LBA {:<10}  {} MB",
                    p + 1,
                    ptype,
                    type_str,
                    dev.part_lba_start[p],
                    dev.part_size_mb[p]
                );
                set_color(palette::TEXT);
                crate::mesa_println!();
            }
            crate::mesa_print!("  │");
            crate::mesa_println!();
        }

        // Barra de progreso de fases
        let phases = [
            (
                "Slot",
                matches!(
                    dev.phase,
                    UsbPhase::SlotEnabled
                        | UsbPhase::AddressAssigned
                        | UsbPhase::DeviceDescriptorRead
                        | UsbPhase::ConfigDescriptorRead
                        | UsbPhase::EndpointsConfigured
                        | UsbPhase::SetConfigurationDone
                ),
            ),
            (
                "Addr",
                matches!(
                    dev.phase,
                    UsbPhase::AddressAssigned
                        | UsbPhase::DeviceDescriptorRead
                        | UsbPhase::ConfigDescriptorRead
                        | UsbPhase::EndpointsConfigured
                        | UsbPhase::SetConfigurationDone
                ),
            ),
            (
                "DevDesc",
                matches!(
                    dev.phase,
                    UsbPhase::DeviceDescriptorRead
                        | UsbPhase::ConfigDescriptorRead
                        | UsbPhase::EndpointsConfigured
                        | UsbPhase::SetConfigurationDone
                ),
            ),
            (
                "CfgDesc",
                matches!(
                    dev.phase,
                    UsbPhase::ConfigDescriptorRead
                        | UsbPhase::EndpointsConfigured
                        | UsbPhase::SetConfigurationDone
                ),
            ),
            (
                "EPs",
                matches!(
                    dev.phase,
                    UsbPhase::EndpointsConfigured | UsbPhase::SetConfigurationDone
                ),
            ),
            (
                "SetCfg",
                matches!(dev.phase, UsbPhase::SetConfigurationDone),
            ),
        ];
        crate::mesa_print!("  │     Fases: ");
        for (name, done) in &phases {
            if *done {
                set_color(palette::SUCCESS);
                crate::mesa_print!("[✓{}]", name);
            } else {
                set_color(palette::MUTED);
                crate::mesa_print!("[·{}]", name);
            }
        }
        set_color(palette::TEXT);
        crate::mesa_println!();
        crate::mesa_println!("  │");
    }

    if !found_any {
        set_color(palette::SUBTLE);
        crate::mesa_println!("  │  (ningún dispositivo enumerado todavía)");
        set_color(palette::TEXT);
    }

    crate::mesa_println!("  └────────────────────────────────────────────");
    crate::mesa_println!();

    // ── Resumen ───────────────────────────────────────────────────────────────
    set_color(palette::FOAM);
    crate::mesa_print!("  Resumen: ");
    set_color(palette::TEXT);
    crate::mesa_print!("HC ");
    if hc_running && !hc_error {
        set_color(palette::SUCCESS);
        crate::mesa_print!("ACTIVO");
    } else {
        set_color(palette::ERROR);
        crate::mesa_print!("INACTIVO");
    }
    set_color(palette::TEXT);
    crate::mesa_print!("  │  ");
    set_color(palette::GOLD);
    crate::mesa_print!("{}/{}", connected, num_ports);
    set_color(palette::TEXT);
    crate::mesa_print!(" puertos usados  │  ");
    let fully_enum = devices
        .iter()
        .filter(|d| d.active && matches!(d.phase, UsbPhase::SetConfigurationDone))
        .count();
    set_color(if fully_enum > 0 {
        palette::SUCCESS
    } else {
        palette::SUBTLE
    });
    crate::mesa_print!("{} dispositivo(s) OK", fully_enum);
    set_color(palette::TEXT);
    crate::mesa_println!();
    crate::mesa_println!();
}
