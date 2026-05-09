pub mod handshake;
pub mod rings;
pub mod context;
pub mod msix;

// ─── Timeouts (en iteraciones de spin_loop) ───────────────────────────────────
const TIMEOUT_HCRST:     usize = 100_000_000;
const TIMEOUT_CNR:       usize = 100_000_000;
const TIMEOUT_HCH:       usize = 100_000_000;
const TIMEOUT_PORT_RESET: usize = 100_000_000;
const TIMEOUT_PORT_ENABLE: usize = 100_000_000;
const TIMEOUT_EVENT:     usize = 100_000_000;

pub fn delay_ms(ms: u64) {
    let start = crate::curr_arch::get_ticks();
    // Aproximación: 1 tick = 55ms (PIT estándar 18.2Hz)
    let ticks_to_wait = (ms / 55) + 1;
    while crate::curr_arch::get_ticks().wrapping_sub(start) < ticks_to_wait {
        core::hint::spin_loop();
    }
}

use core::num::NonZeroUsize;
use xhci::accessor::Mapper;
use xhci::Registers;
use crate::memory::{pmm, vmm};
use crate::pci::PciDevice;
use alloc::boxed::Box;

use rings::{CommandRing, EventRing, TransferRing};

#[derive(Clone)]
pub struct XhciMapper;

impl Mapper for XhciMapper {
    unsafe fn map(&mut self, phys_base: usize, size: usize) -> NonZeroUsize {
        let virt = vmm::map_mmio(phys_base as u64, size as u64).unwrap_or_else(|_| vmm::phys_to_virt(phys_base as u64)) as usize;
        NonZeroUsize::new(virt).expect("invalid MMIO base")
    }
    fn unmap(&mut self, _virt_base: usize, _bytes: usize) {}
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct EventRingSegmentTableEntry {
    pub base_addr: u64,
    pub size: u16,
    pub rsvd1: u16,
    pub rsvd2: u32,
}

pub struct UsbDeviceInfo {
    pub slot: u8,
    pub port: u8,
    pub speed: u8,
    pub ctx_phys: u64,
    pub ep0_ring: TransferRing,
    pub extra_rings: [Option<TransferRing>; 32],
}

pub struct XhciDriver {
    pub regs: Registers<XhciMapper>,
    pub base_virt: u64,
    pub pci_bus: u8,
    pub pci_device: u8,
    pub pci_function: u8,
    pub ports: usize,
    pub next_slot: u8,
    pub cmd_ring: Option<CommandRing>,
    pub evt_ring: Option<EventRing>,
    pub dcbaa_virt: *mut u64,
    pub devices: [Option<Box<UsbDeviceInfo>>; 64],
}

unsafe impl Send for XhciDriver {}
unsafe impl Sync for XhciDriver {}

impl XhciDriver {
    pub fn new(device: &PciDevice, bar0_phys: u64) -> Option<Self> {
        if bar0_phys == 0 {
            return None;
        }

        let mapper = XhciMapper;
        let mut regs = unsafe { Registers::new(bar0_phys as usize, mapper) };
        let ports = regs.capability.hcsparams1.read_volatile().number_of_ports() as usize;

        crate::serial_println!("[XHCI] Controller at {} ports", ports);

        let base_virt = crate::memory::vmm::phys_to_virt(bar0_phys);

        Some(Self {
            regs,
            base_virt,
            pci_bus: device.bus,
            pci_device: device.device,
            pci_function: device.function,
            ports,
            next_slot: 1,
            cmd_ring: None,
            evt_ring: None,
            dcbaa_virt: core::ptr::null_mut(),
            devices: core::array::from_fn(|_| None),
        })
    }

    pub fn alloc_64b(&self, size: usize) -> Option<(u64, *mut u8)> {
        let frames = (size + 4095) / 4096;
        if let Some(phys) = pmm::alloc_frames(frames as usize) {
            let virt = vmm::phys_to_virt(phys) as *mut u8;
            unsafe { core::ptr::write_bytes(virt, 0, frames as usize * 4096); }
            Some((phys, virt))
        } else {
            None
        }
    }

    pub fn init(&mut self) -> Result<(), &'static str> {
        crate::serial_println!("[XHCI] BIOS Handshake...");
        handshake::perform_bios_handoff(self.base_virt);

        crate::serial_println!("[XHCI] Configurando MSI-X (no-fatal)...");
        msix::configure_msix(self.pci_bus, self.pci_device, self.pci_function);

        crate::serial_println!("[XHCI] Sending Reset...");
        let mut usbcmd = self.regs.operational.usbcmd.read_volatile();
        usbcmd.set_host_controller_reset();
        self.regs.operational.usbcmd.write_volatile(usbcmd);
        let mut timeout = 0usize;
        while self.regs.operational.usbcmd.read_volatile().host_controller_reset() {
            timeout += 1;
            if timeout > TIMEOUT_HCRST { return Err("HCRST timeout"); }
            core::hint::spin_loop();
        }
        timeout = 0;
        while self.regs.operational.usbsts.read_volatile().controller_not_ready() {
            timeout += 1;
            if timeout > TIMEOUT_CNR { return Err("CNR timeout"); }
            core::hint::spin_loop();
        }

        let max_slots = self.regs.capability.hcsparams1.read_volatile().number_of_device_slots();
        let mut config = self.regs.operational.config.read_volatile();
        config.set_max_device_slots_enabled(max_slots);
        self.regs.operational.config.write_volatile(config);

        let (dcbaa_phys, dcbaa_virt) = self.alloc_64b(2048).unwrap();
        self.dcbaa_virt = dcbaa_virt as *mut u64;
        let mut dcbaap = self.regs.operational.dcbaap.read_volatile();
        dcbaap.set(dcbaa_phys);
        self.regs.operational.dcbaap.write_volatile(dcbaap);

        // Calculate and allocate Scratchpad buffers if required by hardware
        let hcsparams2_raw = unsafe { core::ptr::read_volatile((self.base_virt + 0x08) as *const u32) };
        let max_scratchpads_hi = (hcsparams2_raw >> 21) & 0x1F;
        let max_scratchpads_lo = (hcsparams2_raw >> 27) & 0x1F;
        let max_scratchpads = (max_scratchpads_hi << 5) | max_scratchpads_lo;

        if max_scratchpads > 0 {
            crate::serial_println!("[XHCI] Hardware requires {} scratchpad buffers", max_scratchpads);
            let (sp_arr_phys, sp_arr_virt) = self.alloc_64b((max_scratchpads as usize) * 8).unwrap();
            let sp_arr = sp_arr_virt as *mut u64;
            for i in 0..max_scratchpads {
                let (buf_phys, _) = self.alloc_64b(4096).unwrap(); // 4KB page per scratchpad
                unsafe { core::ptr::write_volatile(sp_arr.add(i as usize), buf_phys); }
            }
            // Scratchpad Array pointer MUST be placed at DCBAA[0]
            unsafe { core::ptr::write_volatile(self.dcbaa_virt, sp_arr_phys); }
        } else {
            unsafe { core::ptr::write_volatile(self.dcbaa_virt, 0); }
        }

        let (cmd_phys, cmd_virt) = self.alloc_64b(4096).unwrap();
        self.cmd_ring = Some(CommandRing::new(cmd_virt, cmd_phys, 256));
        let mut crcr = self.regs.operational.crcr.read_volatile();
        crcr.set_ring_cycle_state();
        crcr.set_command_ring_pointer(cmd_phys);
        self.regs.operational.crcr.write_volatile(crcr);

        let (evt_phys, evt_virt) = self.alloc_64b(4096).unwrap();
        self.evt_ring = Some(EventRing::new(evt_virt, evt_phys, 256));

        let (erst_phys, erst_virt) = self.alloc_64b(64).unwrap();
        let erst = erst_virt as *mut EventRingSegmentTableEntry;
        unsafe {
            (*erst).base_addr = evt_phys;
            (*erst).size = 256;
        }

        let mut interrupter = self.regs.interrupter_register_set.interrupter_mut(0);
        let mut erstsz = interrupter.erstsz.read_volatile();
        erstsz.set(1);
        interrupter.erstsz.write_volatile(erstsz);

        let mut erdp = interrupter.erdp.read_volatile();
        erdp.set_event_ring_dequeue_pointer(evt_phys);
        interrupter.erdp.write_volatile(erdp);

        let mut erstba = interrupter.erstba.read_volatile();
        erstba.set(erst_phys);
        interrupter.erstba.write_volatile(erstba);

        let mut iman = interrupter.iman.read_volatile();
        iman.set_interrupt_enable();
        interrupter.iman.write_volatile(iman);

        let mut usbcmd = self.regs.operational.usbcmd.read_volatile();
        usbcmd.set_run_stop();
        self.regs.operational.usbcmd.write_volatile(usbcmd);
        timeout = 0;
        while self.regs.operational.usbsts.read_volatile().hc_halted() {
            timeout += 1;
            if timeout > TIMEOUT_HCH { return Err("HCH timeout"); }
            core::hint::spin_loop();
        }

        crate::serial_println!("[XHCI] Initialized and Running.");
        Ok(())
    }

    pub fn send_command(&mut self, raw: [u32; 4]) {
        if let Some(ref mut ring) = self.cmd_ring {
            ring.push_raw(raw);
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            let mut db = self.regs.doorbell.read_volatile_at(0);
            db.set_doorbell_target(0);
            self.regs.doorbell.write_volatile_at(0, db);
        }
    }

    pub fn await_event(&mut self) -> Result<[u32; 4], &'static str> {
        let mut timeout = 0usize;
        loop {
            if let Some(ref mut ring) = self.evt_ring {
                if let Some(evt) = ring.poll() {
                    // Actualizar ERDP para indicar al hardware que procesamos el evento
                    let mut interrupter = self.regs.interrupter_register_set.interrupter_mut(0);
                    let curr_addr = ring.ring_phys + (ring.index as u64 * 16);
                    let mut erdp = interrupter.erdp.read_volatile();
                    erdp.set_event_ring_dequeue_pointer(curr_addr);
                    erdp.clear_event_handler_busy();
                    interrupter.erdp.write_volatile(erdp);

                    let mut iman = interrupter.iman.read_volatile();
                    iman.set_0_interrupt_pending();
                    interrupter.iman.write_volatile(iman);
                    return Ok(evt);
                }
            }
            timeout += 1;
            if timeout > TIMEOUT_EVENT {
                return Err("Event ring timeout");
            }
            core::hint::spin_loop();
        }
    }

    pub fn port_reset(&mut self, port_id: u8) -> Result<(), &'static str> {
        use xhci::registers::operational::PortRegisterSet;
        let p = self.regs.port_register_set.read_volatile_at(port_id as usize);
        if !p.portsc.current_connect_status() {
            return Err("No device");
        }

        if !p.portsc.port_power() {
            let mut new_portsc = p.portsc;
            new_portsc.set_port_power();
            self.regs.port_register_set.write_volatile_at(port_id as usize, PortRegisterSet { portsc: new_portsc, ..p });
            delay_ms(100); // 100ms para estabilizar puerto
        }

        let p = self.regs.port_register_set.read_volatile_at(port_id as usize);
        if !p.portsc.port_enabled_disabled() {
            let mut new_portsc = p.portsc;
            new_portsc.set_port_reset();
            self.regs.port_register_set.write_volatile_at(port_id as usize, PortRegisterSet { portsc: new_portsc, ..p });
            
            let mut timeout = 0usize;
            loop {
                let p2 = self.regs.port_register_set.read_volatile_at(port_id as usize);
                if !p2.portsc.port_reset() { break; }
                timeout += 1;
                if timeout > TIMEOUT_PORT_RESET {
                    return Err("Port reset timeout");
                }
                core::hint::spin_loop();
            }

            let p3 = self.regs.port_register_set.read_volatile_at(port_id as usize);
            let mut new_portsc = p3.portsc;
            new_portsc.set_0_port_reset_change();
            self.regs.port_register_set.write_volatile_at(port_id as usize, PortRegisterSet { portsc: new_portsc, ..p3 });
            
            delay_ms(20); // Recovery time

            let mut enabled_timeout = 0usize;
            loop {
                let px = self.regs.port_register_set.read_volatile_at(port_id as usize);
                if px.portsc.port_enabled_disabled() { break; }
                enabled_timeout += 1;
                if enabled_timeout >= TIMEOUT_PORT_ENABLE {
                    crate::serial_println!("[XHCI] Timeout esperando Port Enable en puerto {}", port_id);
                    return Err("Failed to enable port");
                }
                core::hint::spin_loop();
            }
        }
        Ok(())
    }

    pub fn enumerate_port(&mut self, port_id: u8) -> Result<(), &'static str> {
        use xhci::ring::trb::command;
        self.port_reset(port_id)?;

        let speed = self.regs.port_register_set.read_volatile_at(port_id as usize).portsc.port_speed();
        
        let mut enable_slot = command::EnableSlot::new();
        self.send_command(enable_slot.into_raw());

        let evt_raw = self.await_event()?;
        use xhci::ring::trb::event;
        use core::convert::TryFrom;
        
        let slot_id = if let Ok(event::Allowed::CommandCompletion(c)) = event::Allowed::try_from(evt_raw) {
            c.slot_id()
        } else {
            return Err("Unexpected TRB on Enable Slot");
        };

        crate::serial_println!("[XHCI] ENABLE_SLOT completed, valid Slot = {}", slot_id);

        let (out_ctx_phys, out_ctx_virt) = self.alloc_64b(2048).unwrap();
        unsafe { core::ptr::write_volatile(self.dcbaa_virt.add(slot_id as usize), out_ctx_phys); }

        let (ep0_phys, ep0_virt) = self.alloc_64b(4096).unwrap();
        let transfer_ring = TransferRing::new(ep0_virt, ep0_phys, 256);

        let (in_ctx_phys, in_ctx_virt) = self.alloc_64b(2048).unwrap();
        let csz = self.regs.capability.hccparams1.read_volatile().context_size();
        
        context::configure_input_context(in_ctx_virt, slot_id, port_id, speed, ep0_phys, csz);

        for _ in 0..10_000 { core::hint::spin_loop(); }

        let mut address_dev = command::AddressDevice::new();
        address_dev.set_slot_id(slot_id);
        address_dev.set_input_context_pointer(in_ctx_phys);
        address_dev.clear_block_set_address_request(); // BSR = 0

        self.send_command(address_dev.into_raw());
        let addr_evt = self.await_event()?;

        if let Ok(event::Allowed::CommandCompletion(c)) = event::Allowed::try_from(addr_evt) {
            if c.completion_code() != Ok(event::CompletionCode::Success) {
                crate::serial_println!("[XHCI] ADDRESS_DEVICE ERROR");
                return Err("ADDRESS_DEVICE Failed");
            }
        }

        crate::serial_println!("[XHCI] ADDRESS_DEVICE successful. Device initialized on slot {}", slot_id);

        self.devices[slot_id as usize] = Some(Box::new(UsbDeviceInfo {
            slot: slot_id,
            port: port_id,
            speed,
            ctx_phys: out_ctx_phys,
            ep0_ring: transfer_ring,
            extra_rings: core::array::from_fn(|_| None),
        }));

        // Trigger GET_DESCRIPTOR control transfer
        let mut desc: crate::drivers::usb::descriptors::DeviceDescriptor = unsafe { core::mem::zeroed() };
        let success = self.control_transfer(
            slot_id,
            0x80, // RequestType: DirIn | TypeStandard | RecipientDevice
            0x06, // GET_DESCRIPTOR
            0x0100, // Descriptor Type: Device, Index 0
            0x0000,
            &mut desc as *mut _ as *mut u8,
            18
        );

        if success {
            use core::ptr::{read_unaligned, addr_of};
            let vid = unsafe { read_unaligned(addr_of!(desc.vendor_id)) };
            let pid = unsafe { read_unaligned(addr_of!(desc.product_id)) };
            crate::serial_println!("[XHCI] Got Device Descriptor! Vendor: {:#06x}, Product: {:#06x}", vid, pid);

            // Fetch configuration descriptor
            let mut config_desc: crate::drivers::usb::descriptors::ConfigDescriptor = unsafe { core::mem::zeroed() };
            let success_config = self.control_transfer(
                slot_id,
                0x80, // DirIn
                0x06, // GET_DESCRIPTOR
                0x0200, // Config 0
                0x0000,
                &mut config_desc as *mut _ as *mut u8,
                9
            );

            if success_config {
                let total_len = config_desc.total_length as usize;
                let (buf_phys, buf_virt) = self.alloc_64b(total_len).unwrap();
                let success_full = self.control_transfer(
                    slot_id,
                    0x80,
                    0x06,
                    0x0200,
                    0x0000,
                    buf_virt,
                    total_len
                );

                if success_full {
                    let data = unsafe { core::slice::from_raw_parts(buf_virt, total_len) };
                    let iter = crate::drivers::usb::descriptors::DescriptorIter::new(data);
                    
                    let mut current_interface: Option<crate::drivers::usb::descriptors::InterfaceDescriptor> = None;
                    let mut bulk_in: Option<u8> = None;
                    let mut bulk_out: Option<u8> = None;
                    let mut bulk_mps: u16 = 0;

                    for (dtype, slice) in iter {
                        match dtype {
                            4 => { // Interface
                                let iface: crate::drivers::usb::descriptors::InterfaceDescriptor = unsafe { core::ptr::read(slice.as_ptr() as *const _) };
                                current_interface = Some(iface);
                            }
                            5 => { // Endpoint
                                let ep: crate::drivers::usb::descriptors::EndpointDescriptor = unsafe { core::ptr::read(slice.as_ptr() as *const _) };
                                if let Some(iface) = current_interface {
                                    if iface.interface_class == 0x08 { // Mass Storage
                                        let addr = ep.endpoint_address;
                                        if (addr & 0x80) != 0 {
                                            bulk_in = Some(addr & 0x0F);
                                        } else {
                                            bulk_out = Some(addr & 0x0F);
                                        }
                                        bulk_mps = ep.max_packet_size;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    if let (Some(bi), Some(bo)) = (bulk_in, bulk_out) {
                        crate::serial_println!("[XHCI] Detected Mass Storage Device! BI={} BO={} MPS={}", bi, bo, bulk_mps);
                        
                        // Set Configuration
                        if self.control_transfer(slot_id, 0x00, 0x09, config_desc.config_value as u16, 0, core::ptr::null_mut(), 0) {
                            crate::serial_println!("[XHCI] Configuration set to {}", config_desc.config_value);
                            
                            // Configure Endpoints
                            let in_dci = (bi * 2) + 1;
                            let out_dci = bo * 2;

                            if self.configure_endpoint(slot_id, in_dci, 6, bulk_mps).is_ok() &&
                               self.configure_endpoint(slot_id, out_dci, 2, bulk_mps).is_ok() {
                                
                                crate::serial_println!("[XHCI] Endpoints configured. Initializing MSC...");
                                
                                // Find our index in XHCI_CONTROLLERS
                                let ctrl_idx = {
                                    let controllers = crate::drivers::usb::XHCI_CONTROLLERS.lock();
                                    controllers.iter().position(|c| c.pci_bus == self.pci_bus && c.pci_device == self.pci_device).unwrap_or(0)
                                };

                                let mut msc = crate::drivers::usb::msc::MscDevice::new(ctrl_idx, slot_id, in_dci, out_dci, bulk_mps);
                                match msc.init(self) {
                                    Ok(_) => {
                                        crate::drivers::usb::msc::register(msc);
                                        crate::serial_println!("[XHCI] MSC Device registered successfully");
                                    }
                                    Err(e) => {
                                        crate::serial_println!("[XHCI] MSC Device initialization failed: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            crate::serial_println!("[XHCI] Failed to fetch device descriptor on slot {}", slot_id);
        }

        Ok(())
    }

    pub fn control_transfer(&mut self, slot: u8, request_type: u8, request: u8, value: u16, index: u16, data: *mut u8, len: usize) -> bool {
        use xhci::ring::trb::transfer;
        use xhci::ring::trb::event;
        use core::convert::TryFrom;

        let (data_phys, data_virt) = if len > 0 {
            let (p, v) = self.alloc_64b(len).unwrap();
            if (request_type & 0x80) == 0 { // OUT transfer direction
                unsafe { core::ptr::copy_nonoverlapping(data, v, len); }
            }
            (p, v)
        } else {
            (0, core::ptr::null_mut())
        };

        if let Some(ref mut d) = self.devices[slot as usize] {
            let mut setup = transfer::SetupStage::new();
            setup.set_request_type(request_type);
            setup.set_request(request);
            setup.set_value(value);
            setup.set_index(index);
            setup.set_length(len as u16);
            let trt = if len > 0 { if (request_type & 0x80) != 0 { transfer::TransferType::In } else { transfer::TransferType::Out } } else { transfer::TransferType::No };
            setup.set_transfer_type(trt);
            // SetupStage::new() automatically sets IDT=1 and Transfer Length=8

            d.ep0_ring.push_raw(setup.into_raw());

            if len > 0 {
                let mut data_stage = transfer::DataStage::new();
                data_stage.set_data_buffer_pointer(data_phys);
                data_stage.set_trb_transfer_length(len as u32);
                let dir = if (request_type & 0x80) != 0 { transfer::Direction::In } else { transfer::Direction::Out };
                data_stage.set_direction(dir);
                d.ep0_ring.push_raw(data_stage.into_raw());
            }

            // StatusStage: dirección siempre opuesta a la fase de datos.
            // Si data es IN (bit7=1) o no hay datos => Status es OUT (clear).
            // Si data es OUT (bit7=0)              => Status es IN  (set).
            let mut status = transfer::StatusStage::new();
            let status_is_in = len == 0 || (request_type & 0x80) == 0;
            if status_is_in {
                status.set_direction();
            } else {
                status.clear_direction();
            }
            status.set_interrupt_on_completion();

            d.ep0_ring.push_raw(status.into_raw());
        } else {
            return false;
        }

        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let mut db = self.regs.doorbell.read_volatile_at(slot as usize);
        db.set_doorbell_target(1); // Target 1 corresponds to EP0
        self.regs.doorbell.write_volatile_at(slot as usize, db);

        // Loop until we get the Transfer event for our EP
        let mut success = false;
        loop {
            if let Ok(evt_raw) = self.await_event() {
                if let Ok(event::Allowed::TransferEvent(t)) = event::Allowed::try_from(evt_raw) {
                    if t.slot_id() != slot || t.endpoint_id() != 1 {
                        crate::serial_println!("[XHCI] Control: Ignored event for slot {}, ep {}", t.slot_id(), t.endpoint_id());
                        continue;
                    }
                    let code = t.completion_code();
                    if code == Ok(event::CompletionCode::Success) || code == Ok(event::CompletionCode::ShortPacket) {
                        // Success or Short Packet
                        if len > 0 && (request_type & 0x80) != 0 {
                            unsafe { core::ptr::copy_nonoverlapping(data_virt, data, len); }
                        }
                        success = true;
                    } else {
                        crate::serial_println!("[XHCI] Control Transfer Error: {:?}", code);
                        success = false;
                    }
                    break;
                }
            } else {
                break;
            }
        }
        success
    }

    pub fn bulk_transfer(&mut self, slot: u8, dci: u8, data: *mut u8, len: usize, is_in: bool) -> bool {
        use xhci::ring::trb::transfer;
        use xhci::ring::trb::event;
        use core::convert::TryFrom;

        let (data_phys, data_virt) = {
            let (p, v) = self.alloc_64b(len).unwrap();
            if !is_in {
                unsafe { core::ptr::copy_nonoverlapping(data, v, len); }
            }
            (p, v)
        };

        if let Some(ref mut d) = self.devices[slot as usize] {
            if let Some(ref mut ring) = d.extra_rings[dci as usize] {
                let mut normal = transfer::Normal::new();
                normal.set_data_buffer_pointer(data_phys);
                normal.set_trb_transfer_length(len as u32);
                normal.set_interrupt_on_completion();
                
                ring.push_raw(normal.into_raw());
            } else {
                return false;
            }
        } else {
            return false;
        }

        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let mut db = self.regs.doorbell.read_volatile_at(slot as usize);
        db.set_doorbell_target(dci);
        self.regs.doorbell.write_volatile_at(slot as usize, db);

        let mut success = false;
        loop {
            if let Ok(evt_raw) = self.await_event() {
                if let Ok(event::Allowed::TransferEvent(t)) = event::Allowed::try_from(evt_raw) {
                    if t.slot_id() != slot || t.endpoint_id() != dci {
                        crate::serial_println!("[XHCI] Bulk: Ignored event for slot {}, ep {}", t.slot_id(), t.endpoint_id());
                        continue;
                    }
                    let code = t.completion_code();
                    if code == Ok(event::CompletionCode::Success) || code == Ok(event::CompletionCode::ShortPacket) {
                        if is_in {
                            unsafe { core::ptr::copy_nonoverlapping(data_virt, data, len); }
                        }
                        success = true;
                    } else {
                        crate::serial_println!("[XHCI] Bulk Transfer Error: {:?}", code);
                    }
                    break;
                }
            } else {
                break;
            }
        }
        success
    }

    pub fn configure_endpoint(&mut self, slot: u8, dci: u8, ep_type: u8, max_packet_size: u16) -> Result<(), &'static str> {
        use xhci::ring::trb::command;
        use xhci::context::EndpointType;
        
        let (ring_phys, ring_virt) = self.alloc_64b(4096).unwrap();
        let ring = TransferRing::new(ring_virt, ring_phys, 256);

        let (in_ctx_phys, in_ctx_virt) = self.alloc_64b(2048).unwrap();
        let csz = self.regs.capability.hccparams1.read_volatile().context_size();

        let out_ctx_phys = if let Some(ref d) = self.devices[slot as usize] {
            d.ctx_phys
        } else {
            return Err("Device not initialized");
        };
        let out_ctx_virt = crate::memory::vmm::phys_to_virt(out_ctx_phys) as *const u8;
        
        let offset = if csz { 64 } else { 32 };
        unsafe {
            core::ptr::copy_nonoverlapping(out_ctx_virt, in_ctx_virt.add(offset), 2048 - offset);
        }
        let ep_type_enum = match ep_type {
            2 => EndpointType::BulkOut,
            6 => EndpointType::BulkIn,
            _ => return Err("Unsupported EP type for now"),
        };

        context::add_endpoint_context(in_ctx_virt, dci, ep_type_enum, max_packet_size, ring_phys, csz);

        let mut config_ep = command::ConfigureEndpoint::new();
        config_ep.set_slot_id(slot);
        config_ep.set_input_context_pointer(in_ctx_phys);
        
        self.send_command(config_ep.into_raw());
        let evt = self.await_event()?;
        
        // Check success...
        
        if let Some(ref mut d) = self.devices[slot as usize] {
            d.extra_rings[dci as usize] = Some(ring);
        }

        Ok(())
    }

    pub fn scan_ports(&mut self) {
        for port in 0..self.ports {
            if self.enumerate_port(port as u8).is_ok() {
                crate::serial_println!("[XHCI] Port {} effectively enumerated", port);
            }
        }
    }
}
