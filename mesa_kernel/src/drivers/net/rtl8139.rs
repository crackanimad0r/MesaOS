//! Driver para Realtek RTL8139
//! Soporta Rx/Tx completo

use x86_64::instructions::port::Port;
use crate::pci;
use crate::memory;
use alloc::vec::Vec;
use spin::Mutex;

// Constantes
const VENDOR_ID: u16 = 0x10EC;
const DEVICE_ID: u16 = 0x8139;

// Buffer sizes
const RX_BUFFER_SIZE: usize = 8192 + 16 + 1500; // 8KB + overhead
const TX_BUFFER_SIZE: usize = 1536; // Max packet size

// Registros RTL8139
const REG_MAC0: u16 = 0x00;
const REG_TSD0: u16 = 0x10;  // Transmit Status Descriptor 0-3
const REG_TSAD0: u16 = 0x20; // Transmit Start Address Descriptor 0-3
const REG_RBSTART: u16 = 0x30; // Rx Buffer Start Address
const REG_CMD: u16 = 0x37;
const REG_CAPR: u16 = 0x38;  // Current Address of Packet Read
const REG_CBR: u16 = 0x3A;   // Current Buffer Address
const REG_IMR: u16 = 0x3C;   // Interrupt Mask Register
const REG_ISR: u16 = 0x3E;   // Interrupt Status Register
const REG_TCR: u16 = 0x40;   // Transmit Configuration Register
const REG_RCR: u16 = 0x44;   // Receive Configuration Register
const REG_CONFIG1: u16 = 0x52;

// Command bits
const CMD_RESET: u8 = 0x10;
const CMD_RX_ENABLE: u8 = 0x08;
const CMD_TX_ENABLE: u8 = 0x04;

// Interrupt bits
const INT_ROK: u16 = 0x01;  // Receive OK
const INT_TOK: u16 = 0x04;  // Transmit OK
const INT_PUN: u16 = 0x20;  // Packet Underrun / Link Change
const INT_LINKCHG: u16 = 0x20; // Link Change

// Rx Config
const RCR_AAP: u32 = 0x01;     // Accept All Packets
const RCR_APM: u32 = 0x02;     // Accept Physical Match
const RCR_AM: u32 = 0x04;      // Accept Multicast
const RCR_AB: u32 = 0x08;      // Accept Broadcast
const RCR_WRAP: u32 = 0x80;    // Wrap mode
const RCR_RBLEN_8K: u32 = 0x00; // 8K buffer

// Tx Config
const TCR_IFG_STANDARD: u32 = 0x03000000;

pub struct Rtl8139 {
    io_base: u16,
    mac: [u8; 6],
    rx_buffer: u64,  // Physical address
    tx_buffers: [u64; 4],  // Physical addresses
    tx_index: u8,
    rx_offset: u16,
}

impl Rtl8139 {
    pub const fn new() -> Self {
        Self {
            io_base: 0,
            mac: [0; 6],
            rx_buffer: 0,
            tx_buffers: [0; 4],
            tx_index: 0,
            rx_offset: 0,
        }
    }

    pub fn init(&mut self) -> Result<(), &'static str> {
        let device = pci::find_device(VENDOR_ID, DEVICE_ID).ok_or("RTL8139 not found")?;
        
        // Enable Bus Mastering
        self.enable_bus_mastering(&device);
        
        // Get IO Base
        let bar0 = self.read_pci_config(&device, 0x10);
        if bar0 & 1 == 0 {
            return Err("BAR0 is not IO mapped");
        }
        
        self.io_base = (bar0 & !3) as u16;
        crate::serial_println!("[NET] RTL8139 IO Base: {:#x}", self.io_base);
        
        // Power on
        self.power_on();
        
        // Reset
        self.reset()?;
        
        // Read MAC
        self.read_mac();
        
        // Allocate buffers
        self.allocate_buffers()?;
        
        // Configure Rx
        self.configure_rx();
        
        // Configure Tx
        self.configure_tx();
        
        // Enable Rx/Tx
        unsafe {
            let mut cmd_port: Port<u8> = Port::new(self.io_base + REG_CMD);
            cmd_port.write(CMD_RX_ENABLE | CMD_TX_ENABLE);
        }
        
        crate::serial_println!("[NET] RTL8139 fully initialized and enabled");
        Ok(())
    }
    
    fn enable_bus_mastering(&self, device: &crate::pci::PciDevice) {
        unsafe {
            let addr = (1 << 31) | ((device.bus as u32) << 16) | 
                      ((device.device as u32) << 11) | ((device.function as u32) << 8) | 0x04;
            let mut p_addr: Port<u32> = Port::new(0xCF8);
            let mut p_data: Port<u32> = Port::new(0xCFC);
            
            p_addr.write(addr);
            let mut cmd = p_data.read();
            
            if cmd & 0x04 == 0 {
                cmd |= 0x04;
                p_addr.write(addr);
                p_data.write(cmd);
            }
        }
    }
    
    fn read_pci_config(&self, device: &crate::pci::PciDevice, offset: u8) -> u32 {
        unsafe {
            let addr = (1 << 31) | ((device.bus as u32) << 16) | 
                      ((device.device as u32) << 11) | ((device.function as u32) << 8) | (offset as u32 & 0xFC);
            let mut p_addr: Port<u32> = Port::new(0xCF8);
            let mut p_data: Port<u32> = Port::new(0xCFC);
            p_addr.write(addr);
            p_data.read()
        }
    }
    
    fn power_on(&self) {
        unsafe {
            let mut config1_port: Port<u8> = Port::new(self.io_base + REG_CONFIG1);
            let config1 = config1_port.read();
            config1_port.write(config1 & !0x03); // Clear SLEEP and PWRDWN
        }
    }
    
    fn reset(&self) -> Result<(), &'static str> {
        crate::serial_println!("[NET] Resetting RTL8139...");
        unsafe {
            let mut port: Port<u8> = Port::new(self.io_base + REG_CMD);
            port.write(CMD_RESET);
            
            for _ in 0..10000 {
                if port.read() & CMD_RESET == 0 {
                    return Ok(());
                }
            }
        }
        Err("RTL8139 Reset timeout")
    }
    
    fn read_mac(&mut self) {
        unsafe {
            for i in 0..6 {
                let mut port: Port<u8> = Port::new(self.io_base + REG_MAC0 + i as u16);
                self.mac[i] = port.read();
            }
        }
        crate::serial_println!("[NET] MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.mac[0], self.mac[1], self.mac[2], self.mac[3], self.mac[4], self.mac[5]);
    }
    
    fn allocate_buffers(&mut self) -> Result<(), &'static str> {
        // Allocate Rx buffer (need 3 frames for 8KB + overhead)
        self.rx_buffer = memory::pmm::alloc_frames(3).ok_or("Failed to allocate Rx buffer")?;
        
        // Allocate 4 Tx buffers
        for i in 0..4 {
            let tx_phys = memory::pmm::alloc_frame().ok_or("Failed to allocate Tx buffer")?;
            self.tx_buffers[i] = tx_phys;
        }
        
        crate::serial_println!("[NET] Buffers allocated (Rx: {:#x})", self.rx_buffer);
        Ok(())
    }
    
    fn configure_rx(&mut self) {
        unsafe {
            // Set Rx buffer address
            let mut rbstart_port: Port<u32> = Port::new(self.io_base + REG_RBSTART);
            rbstart_port.write(self.rx_buffer as u32);
            
            // Reset CAPR (Current Address of Packet Read)
            let mut capr_port: Port<u16> = Port::new(self.io_base + REG_CAPR);
            capr_port.write(0);
            self.rx_offset = 0;
            
            // Clear interrupts
            let mut isr_port: Port<u16> = Port::new(self.io_base + REG_ISR);
            isr_port.write(0xFFFF);
            
            // Configure Rx: accept all packets. 
            // WRAP=0: Packet wraps to start of buffer (matches our modulo logic).
            let mut rcr_port: Port<u32> = Port::new(self.io_base + REG_RCR);
            rcr_port.write(RCR_AAP | RCR_APM | RCR_AM | RCR_AB | RCR_RBLEN_8K);
            
            // Set IMR (Interrupt Mask Register) - even if we poll, some cards like it
            let mut imr_port: Port<u16> = Port::new(self.io_base + REG_IMR);
            imr_port.write(INT_ROK | INT_TOK);
        }
    }
    
    fn configure_tx(&self) {
        unsafe {
            // Configure Tx
            let mut tcr_port: Port<u32> = Port::new(self.io_base + REG_TCR);
            tcr_port.write(TCR_IFG_STANDARD);
        }
    }
    
    pub fn send_packet(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() > TX_BUFFER_SIZE {
            return Err("Packet too large");
        }
        
        let idx = self.tx_index as usize;
        let tx_phys = self.tx_buffers[idx];
        
        // Copy data to Tx buffer (using HHDM offset)
        let tx_virt = memory::vmm::phys_to_virt(tx_phys);
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), tx_virt as *mut u8, data.len());
        }
        
        // Write Tx address and size (with OWN bit to start transmission)
        unsafe {
            let mut tsad_port: Port<u32> = Port::new(self.io_base + REG_TSAD0 + (idx as u16 * 4));
            tsad_port.write(tx_phys as u32);
            
            let mut tsd_port: Port<u32> = Port::new(self.io_base + REG_TSD0 + (idx as u16 * 4));
            tsd_port.write((data.len() as u32) & 0x1FFF); // Length in bits 0-12
        }
        
        crate::serial_println!("[NET] Sent packet: {} bytes (desc {})", data.len(), idx);
        
        // Wait and check status
        unsafe {
            let mut tsd_port: Port<u32> = Port::new(self.io_base + REG_TSD0 + (idx as u16 * 4));
            for i in 0..1000 {
                let status = tsd_port.read();
                if status & (1 << 15) != 0 { // OWN bit (1 means hardware done)
                    if status & (1 << 13) != 0 { // TOK bit
                        // crate::serial_println!("[NET] Tx OK (desc {})", idx);
                    } else if status & (1 << 14) != 0 { // TUN bit
                        crate::serial_println!("[NET] Tx UNDERRUN! (desc {})", idx);
                    }
                    break;
                }
                if i == 999 {
                    crate::serial_println!("[NET] Tx TIMEOUT! (desc {}, status={:#x})", idx, status);
                }
                core::hint::spin_loop();
            }
        }
        
        // Next descriptor
        self.tx_index = (self.tx_index + 1) % 4;
        
        Ok(())
    }
    
    pub fn poll_rx(&mut self) -> Option<Vec<u8>> {
        unsafe {
            let mut isr_port: Port<u16> = Port::new(self.io_base + REG_ISR);
            let isr = isr_port.read();
            if isr != 0 {
                // Escribir 1s para limpiar los bits que leímos (Ack)
                // Es vital limpiar incluso los bits que no procesamos para que no se bloquee.
                isr_port.write(isr); 
                
                if isr & !(INT_ROK | INT_TOK | INT_PUN | INT_LINKCHG) != 0 {
                    crate::serial_println!("[NET] ISR Error status: {:#x}", isr);
                }
            }

            let mut cmd_port: Port<u8> = Port::new(self.io_base + REG_CMD);
            let cmd = cmd_port.read();
            if cmd & CMD_RX_ENABLE == 0 {
                crate::serial_println!("[NET] WARNING: RX_ENABLE lost! cmd={:#x}", cmd);
                return None;
            }
            
            let mut cbr_port: Port<u16> = Port::new(self.io_base + REG_CBR);
            let cbr = cbr_port.read();
            
            if self.rx_offset == cbr {
                return None; // No new packets
            }
            
            crate::serial_println!("[NET] Rx packet available (offset={}, cbr={})", self.rx_offset, cbr);
            
            // Read packet header (4 bytes: status(16) + length(16))
            let rx_virt_base = memory::vmm::phys_to_virt(self.rx_buffer);
            
            // Helper to read 32-bit word with wrap-around
            let read_u32_wrap = |offset: u16| -> u32 {
                let off = offset as u64;
                if off <= 8192 - 4 {
                    unsafe { core::ptr::read_volatile((rx_virt_base + off) as *const u32) }
                } else {
                    // Wrap-around read
                    let mut bytes = [0u8; 4];
                    for i in 0..4 {
                        let b_off = (offset as u64 + i as u64) % 8192;
                        bytes[i] = unsafe { core::ptr::read_volatile((rx_virt_base + b_off) as *const u8) };
                    }
                    u32::from_le_bytes(bytes)
                }
            };

            let header = read_u32_wrap(self.rx_offset);
            
            // RTL8139 format: status is LOW 16 bits, length is HIGH 16 bits
            let status = (header & 0xFFFF) as u16;
            let length = (header >> 16) as u16;
            
            if status & 0x01 == 0 { // ROK bit
                // Wait... if CBR != offset but ROK is 0, something is weird.
                // Just return and wait for next poll.
                // crate::serial_println!("[NET] ROK bit not set for packet @{}: status={:#x}", self.rx_offset, status);
                // Advance past this packet to avoid getting stuck
                let packet_total = (length as u16 + 4 + 3) & !3;
                self.rx_offset = (self.rx_offset + packet_total) % 8192;
                let mut capr_port: Port<u16> = Port::new(self.io_base + REG_CAPR);
                let capr_val = self.rx_offset.wrapping_sub(16) & 0x1FFF;
                capr_port.write(capr_val);
                return None;
            }
            
            // crate::serial_println!("[NET] Packet @{}: status={:#x}, length={}", self.rx_offset, status, length);
            
            if length < 4 || length > 1518 {
                crate::serial_println!("[NET] Invalid packet length: {}", length);
                // Something is wrong, maybe reset offset
                // Advance past this packet to avoid getting stuck
                let packet_total = (length as u16 + 4 + 3) & !3;
                self.rx_offset = (self.rx_offset + packet_total) % 8192;
                let mut capr_port: Port<u16> = Port::new(self.io_base + REG_CAPR);
                let capr_val = self.rx_offset.wrapping_sub(16) & 0x1FFF;
                capr_port.write(capr_val);
                return None;
            }
            
            // Read packet data (skip 4-byte CRC at end)
            let packet_len = (length as usize).saturating_sub(4);
            let mut packet = Vec::with_capacity(packet_len);
            
            for i in 0..packet_len {
                let b_off = (self.rx_offset as u64 + 4 + i as u64) % 8192;
                packet.push(unsafe { core::ptr::read_volatile((rx_virt_base + b_off) as *const u8) });
            }
            
            // Update offset: Header + Data + padding to DWORD
            let packet_total = (length as u16 + 4 + 3) & !3;
            self.rx_offset = (self.rx_offset + packet_total) % 8192;

            let mut capr_port: Port<u16> = Port::new(self.io_base + REG_CAPR);
            // Inform hardware we've processed this packet
            let capr_val = self.rx_offset.wrapping_sub(16) & 0x1FFF;
            capr_port.write(capr_val);
            
            // Debug: Dump raw packet from hardware
            // crate::serial_println!("[NET] Raw Packet Data:");
            // crate::net::hex_dump(&packet, 32);

            Some(packet)
        }
    }
    
    pub fn get_mac(&self) -> [u8; 6] {
        self.mac
    }
}

static DRIVER: Mutex<Option<Rtl8139>> = Mutex::new(None);

pub fn init() -> Result<(), &'static str> {
    if crate::pci::find_device(VENDOR_ID, DEVICE_ID).is_some() {
        let mut driver = Rtl8139::new();
        driver.init()?;
        *DRIVER.lock() = Some(driver);
        Ok(())
    } else {
        Err("RTL8139 not present")
    }
}

pub fn send_packet(data: &[u8]) -> Result<(), &'static str> {
    DRIVER.lock().as_mut()
        .ok_or("Driver not initialized")?
        .send_packet(data)
}

pub fn poll_rx() -> Option<Vec<u8>> {
    DRIVER.lock().as_mut()?.poll_rx()
}

pub fn get_mac() -> Option<[u8; 6]> {
    DRIVER.lock().as_ref().map(|d| d.get_mac())
}

pub fn get_cbr() -> u16 {
    DRIVER.lock().as_ref().map(|d| {
        let mut cbr_port: Port<u16> = Port::new(d.io_base + 0x3A); // REG_CBR
        unsafe { cbr_port.read() }
    }).unwrap_or(0)
}

pub fn get_rx_offset() -> u16 {
    DRIVER.lock().as_ref().map(|d| d.rx_offset).unwrap_or(0)
}

pub fn get_cmd() -> u8 {
    DRIVER.lock().as_ref().map(|d| {
        let mut cmd_port: Port<u8> = Port::new(d.io_base + 0x37); // REG_CMD
        unsafe { cmd_port.read() }
    }).unwrap_or(0)
}
