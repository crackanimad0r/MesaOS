//! Driver ATA/IDE básico (PIO mode)
//! Soporta discos SATA/PATA en modo legacy

use spin::Mutex;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};
use alloc::string::ToString;  // AGREGAR ESTA LÍNEA

/// Tamaño de sector (512 bytes)
pub const SECTOR_SIZE: usize = 512;

/// Puertos ATA Primary
const ATA_PRIMARY_DATA: u16 = 0x1F0;
const ATA_PRIMARY_ERROR: u16 = 0x1F1;
const ATA_PRIMARY_SECTOR_COUNT: u16 = 0x1F2;
const ATA_PRIMARY_LBA_LOW: u16 = 0x1F3;
const ATA_PRIMARY_LBA_MID: u16 = 0x1F4;
const ATA_PRIMARY_LBA_HIGH: u16 = 0x1F5;
const ATA_PRIMARY_DRIVE: u16 = 0x1F6;
const ATA_PRIMARY_COMMAND: u16 = 0x1F7;
const ATA_PRIMARY_STATUS: u16 = 0x1F7;

/// Comandos ATA
const ATA_CMD_READ_PIO: u8 = 0x20;
const ATA_CMD_WRITE_PIO: u8 = 0x30;
const ATA_CMD_IDENTIFY: u8 = 0xEC;
const ATA_CMD_FLUSH: u8 = 0xE7;

/// Bits de status
const ATA_SR_BSY: u8 = 0x80;  // Busy
const ATA_SR_DRDY: u8 = 0x40; // Drive ready
const ATA_SR_DRQ: u8 = 0x08;  // Data request ready
const ATA_SR_ERR: u8 = 0x01;  // Error

/// Información del disco
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub sectors: u64,
    pub size_mb: u64,
    pub model: [u8; 40],
    pub serial: [u8; 20],
}

impl DiskInfo {
    /// Obtiene el modelo como string
    pub fn model_string(&self) -> alloc::string::String {
        let mut s = alloc::string::String::new();
        for chunk in self.model.chunks(2) {
            if chunk.len() == 2 {
                s.push(chunk[1] as char);
                s.push(chunk[0] as char);
            }
        }
        s.trim().to_string()
    }
    
    /// Obtiene el serial como string
    pub fn serial_string(&self) -> alloc::string::String {
        let mut s = alloc::string::String::new();
        for chunk in self.serial.chunks(2) {
            if chunk.len() == 2 {
                s.push(chunk[1] as char);
                s.push(chunk[0] as char);
            }
        }
        s.trim().to_string()
    }
}

/// Driver ATA
pub struct AtaDriver {
    data_port: Port<u16>,
    error_port: PortReadOnly<u8>,
    sector_count_port: PortWriteOnly<u8>,
    lba_low_port: PortWriteOnly<u8>,
    lba_mid_port: PortWriteOnly<u8>,
    lba_high_port: PortWriteOnly<u8>,
    drive_port: PortWriteOnly<u8>,
    command_port: PortWriteOnly<u8>,
    status_port: PortReadOnly<u8>,
    
    info: Option<DiskInfo>,
}

impl AtaDriver {
    /// Crea un nuevo driver ATA (Primary Master)
    pub const fn new() -> Self {
        Self {
            data_port: Port::new(ATA_PRIMARY_DATA),
            error_port: PortReadOnly::new(ATA_PRIMARY_ERROR),
            sector_count_port: PortWriteOnly::new(ATA_PRIMARY_SECTOR_COUNT),
            lba_low_port: PortWriteOnly::new(ATA_PRIMARY_LBA_LOW),
            lba_mid_port: PortWriteOnly::new(ATA_PRIMARY_LBA_MID),
            lba_high_port: PortWriteOnly::new(ATA_PRIMARY_LBA_HIGH),
            drive_port: PortWriteOnly::new(ATA_PRIMARY_DRIVE),
            command_port: PortWriteOnly::new(ATA_PRIMARY_COMMAND),
            status_port: PortReadOnly::new(ATA_PRIMARY_STATUS),
            info: None,
        }
    }
    
    /// Inicializa el disco
    pub fn init(&mut self) -> Result<(), &'static str> {
        crate::serial_println!("[ATA] Detectando disco...");
        
        // Seleccionar drive master (0xA0 = master, 0xB0 = slave)
        unsafe {
            self.drive_port.write(0xA0);
        }
        
        // Esperar un poco
        self.wait_400ns();
        
        // Enviar comando IDENTIFY
        unsafe {
            self.sector_count_port.write(0);
            self.lba_low_port.write(0);
            self.lba_mid_port.write(0);
            self.lba_high_port.write(0);
            self.command_port.write(ATA_CMD_IDENTIFY);
        }
        
        // Leer status
        let status = unsafe { self.status_port.read() };
        if status == 0 {
            return Err("No disk detected");
        }
        
        // Esperar a que el disco esté listo
        self.wait_not_busy()?;
        
        // Verificar que es un disco ATA (no ATAPI)
        let lba_mid = unsafe { Port::<u8>::new(ATA_PRIMARY_LBA_MID).read() };
        let lba_high = unsafe { Port::<u8>::new(ATA_PRIMARY_LBA_HIGH).read() };
        
        if lba_mid != 0 || lba_high != 0 {
            return Err("Not an ATA disk (probably ATAPI)");
        }
        
        // Esperar DRQ
        self.wait_drq()?;
        
        // Leer datos de identificación (256 words = 512 bytes)
        let mut identify_data = [0u16; 256];
        for i in 0..256 {
            identify_data[i] = unsafe { self.data_port.read() };
        }
        
        // Parsear información
        let sectors = if identify_data[83] & (1 << 10) != 0 {
            // LBA48
            let low = identify_data[100] as u64;
            let mid = identify_data[101] as u64;
            let high = identify_data[102] as u64;
            let highest = identify_data[103] as u64;
            low | (mid << 16) | (high << 32) | (highest << 48)
        } else {
            // LBA28
            let low = identify_data[60] as u64;
            let high = identify_data[61] as u64;
            low | (high << 16)
        };
        
        let size_mb = (sectors * SECTOR_SIZE as u64) / 1024 / 1024;
        
        // Extraer modelo (words 27-46, 40 bytes)
        let mut model = [0u8; 40];
        for i in 0..20 {
            let word = identify_data[27 + i];
            model[i * 2] = (word & 0xFF) as u8;
            model[i * 2 + 1] = (word >> 8) as u8;
        }
        
        // Extraer serial (words 10-19, 20 bytes)
        let mut serial = [0u8; 20];
        for i in 0..10 {
            let word = identify_data[10 + i];
            serial[i * 2] = (word & 0xFF) as u8;
            serial[i * 2 + 1] = (word >> 8) as u8;
        }
        
        let info = DiskInfo {
            sectors,
            size_mb,
            model,
            serial,
        };
        
        crate::serial_println!("[ATA] Disco detectado: {} ({} MB)", 
            info.model_string(), info.size_mb);
        
        self.info = Some(info);
        Ok(())
    }
    
    /// Lee un sector
    pub fn read_sector(&mut self, lba: u64, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        if lba >= self.info.as_ref().ok_or("Disk not initialized")?.sectors {
            return Err("LBA out of range");
        }
        
        self.access_sector(lba, ATA_CMD_READ_PIO)?;
        
        // Leer datos (256 words)
        let buf16 = unsafe {
            core::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u16, 256)
        };
        
        for i in 0..256 {
            buf16[i] = unsafe { self.data_port.read() };
        }
        
        Ok(())
    }
    
    /// Escribe un sector
    pub fn write_sector(&mut self, lba: u64, buffer: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        if lba >= self.info.as_ref().ok_or("Disk not initialized")?.sectors {
            return Err("LBA out of range");
        }
        
        self.access_sector(lba, ATA_CMD_WRITE_PIO)?;
        
        // Escribir datos (256 words)
        let buf16 = unsafe {
            core::slice::from_raw_parts(buffer.as_ptr() as *const u16, 256)
        };
        
        for i in 0..256 {
            unsafe { self.data_port.write(buf16[i]); }
        }
        
        // Flush cache
        unsafe {
            self.command_port.write(ATA_CMD_FLUSH);
        }
        self.wait_not_busy()?;
        
        Ok(())
    }
    
    /// Prepara el acceso a un sector
    fn access_sector(&mut self, lba: u64, cmd: u8) -> Result<(), &'static str> {
        self.wait_not_busy()?;
        
        unsafe {
            // Seleccionar drive + LBA mode + bits superiores de LBA
            self.drive_port.write(0xE0 | ((lba >> 24) & 0x0F) as u8);
            
            // Número de sectores (1)
            self.sector_count_port.write(1);
            
            // LBA
            self.lba_low_port.write((lba & 0xFF) as u8);
            self.lba_mid_port.write(((lba >> 8) & 0xFF) as u8);
            self.lba_high_port.write(((lba >> 16) & 0xFF) as u8);
            
            // Enviar comando
            self.command_port.write(cmd);
        }
        
        self.wait_drq()?;
        Ok(())
    }
    
    /// Espera a que el disco no esté busy
    fn wait_not_busy(&mut self) -> Result<(), &'static str> {
        for _ in 0..1000000 {
            let status = unsafe { self.status_port.read() };
            if status & ATA_SR_BSY == 0 {
                if status & ATA_SR_ERR != 0 {
                    return Err("ATA error");
                }
                return Ok(());
            }
        }
        Err("ATA timeout (busy)")
    }
    
    /// Espera a que DRQ esté listo
    fn wait_drq(&mut self) -> Result<(), &'static str> {
        for _ in 0..1000000 {
            let status = unsafe { self.status_port.read() };
            if status & ATA_SR_DRQ != 0 {
                return Ok(());
            }
            if status & ATA_SR_ERR != 0 {
                return Err("ATA error");
            }
        }
        Err("ATA timeout (DRQ)")
    }
    
    /// Espera 400ns (4 lecturas de status)
    fn wait_400ns(&mut self) {
        for _ in 0..4 {
            unsafe { self.status_port.read(); }
        }
    }
    
    /// Obtiene información del disco
    pub fn info(&self) -> Option<&DiskInfo> {
        self.info.as_ref()
    }
}

unsafe impl Send for AtaDriver {}

/// Driver ATA global
static ATA: Mutex<AtaDriver> = Mutex::new(AtaDriver::new());

/// Inicializa el driver ATA
pub fn init() -> Result<(), &'static str> {
    ATA.lock().init()
}

/// Lee un sector del disco
pub fn read_sector(lba: u64, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
    ATA.lock().read_sector(lba, buffer)
}

/// Escribe un sector al disco
pub fn write_sector(lba: u64, buffer: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
    ATA.lock().write_sector(lba, buffer)
}

pub struct AtaBlockDevice;

impl crate::drivers::block::BlockDevice for AtaBlockDevice {
    fn read(&self, lba: u64, count: u16, buffer: &mut [u8]) -> Result<(), &'static str> {
        read_sectors(lba, count as usize, buffer)
    }
    fn write(&self, lba: u64, count: u16, buffer: &[u8]) -> Result<(), &'static str> {
        let expected_len = count as usize * SECTOR_SIZE;
        if buffer.len() < expected_len {
            return Err("Buffer too small for write");
        }
        write_sectors(lba, &buffer[..expected_len])
    }
    fn capacity(&self) -> u64 {
        ATA.lock().info().map(|i| i.sectors).unwrap_or(0)
    }
}

/// Obtiene información del disco
pub fn disk_info() -> Option<DiskInfo> {
    ATA.lock().info().cloned()
}

/// Lee múltiples sectores
pub fn read_sectors(start_lba: u64, count: usize, buffer: &mut [u8]) -> Result<(), &'static str> {
    if buffer.len() < count * SECTOR_SIZE {
        return Err("Buffer too small");
    }
    
    for i in 0..count {
        let sector_buf = &mut buffer[i * SECTOR_SIZE..(i + 1) * SECTOR_SIZE];
        let mut sector = [0u8; SECTOR_SIZE];
        read_sector(start_lba + i as u64, &mut sector)?;
        sector_buf.copy_from_slice(&sector);
    }
    
    Ok(())
}

/// Escribe múltiples sectores
pub fn write_sectors(start_lba: u64, buffer: &[u8]) -> Result<(), &'static str> {
    let count = (buffer.len() + SECTOR_SIZE - 1) / SECTOR_SIZE;
    
    for i in 0..count {
        let start = i * SECTOR_SIZE;
        let end = core::cmp::min(start + SECTOR_SIZE, buffer.len());
        let mut sector = [0u8; SECTOR_SIZE];
        sector[..end - start].copy_from_slice(&buffer[start..end]);
        write_sector(start_lba + i as u64, &sector)?;
    }
    
    Ok(())
}