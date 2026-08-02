use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

// ── AMD I2C DesignWare Controller ──────────────────────────────────────

// FCH MMIO base (Ryzen 5000+)
const FCH_MMIO_BASE: usize = 0xFED80000;

// DesignWare I2C registers (32-bit, offset from MMIO base)
const DW_IC_CON: u32 = 0x00;
const DW_IC_TAR: u32 = 0x04;
const DW_IC_DATA_CMD: u32 = 0x10;
const DW_IC_SS_SCL_HCNT: u32 = 0x14;
const DW_IC_SS_SCL_LCNT: u32 = 0x18;
const DW_IC_INTR_MASK: u32 = 0x28;
const DW_IC_TX_TL: u32 = 0x34;
const DW_IC_RX_TL: u32 = 0x38;
const DW_IC_ENABLE: u32 = 0x6C;
const DW_IC_STATUS: u32 = 0x70;
const DW_IC_TX_ABRT_SOURCE: u32 = 0x78;
const DW_IC_ENABLE_STATUS: u32 = 0x94;
const DW_IC_COMP_VERSION: u32 = 0x9C;

// IC_CON bits
const DW_CON_MASTER: u32 = 0x01;
const DW_CON_SPEED_STD: u32 = 0x00;
const DW_CON_7BIT: u32 = 0x00;
const DW_CON_RESTART_EN: u32 = 0x20;
const DW_CON_TX_EMPTY_CTRL: u32 = 0x100;

// IC_STATUS bits
const DW_STAT_TFNF: u32 = 0x02;
const DW_STAT_TFE: u32 = 0x04;
const DW_STAT_RFNE: u32 = 0x08;
const DW_STAT_RFF: u32 = 0x10;

// IC_DATA_CMD bits
const DW_CMD_READ: u32 = 0x100;
const DW_CMD_RESTART: u32 = 0x200;
const DW_CMD_STOP: u32 = 0x400;

// Known AMD I2C DesignWare controller MMIO addresses (Ryzen 5000 series)
// I2C0 = 0xFEDC0000, I2C1 = 0xFED40000, I2C2-3 at sequential offsets
const KNOWN_I2C_ADDRESSES: &[usize] = &[
    0xFED40000, 0xFED50000, 0xFED60000, 0xFED70000, 0xFED80000, 0xFED90000, 0xFEDA0000, 0xFEDB0000,
    0xFEDC0000, 0xFEDD0000, 0xFEDE0000, 0xFEDF0000,
];

static I2C_BASE: AtomicU32 = AtomicU32::new(0);
static I2C_OK: AtomicBool = AtomicBool::new(false);
static I2C_NO_ABRT: AtomicBool = AtomicBool::new(false);

fn mmio_writel(base: usize, reg: u32, val: u32) {
    unsafe {
        ((base + reg as usize) as *mut u32).write_volatile(val);
    }
}

fn mmio_readl(base: usize, reg: u32) -> u32 {
    unsafe { ((base + reg as usize) as *const u32).read_volatile() }
}

fn dw_i2c_is_present(base: usize) -> bool {
    let ver = mmio_readl(base, DW_IC_COMP_VERSION);
    if ver == 0 || ver == 0xFFFFFFFF {
        return false;
    }
    let con = mmio_readl(base, DW_IC_CON);
    if con == 0xFFFFFFFF {
        return false;
    }
    true
}

fn dw_i2c_probe_controller() -> Option<usize> {
    for &addr in KNOWN_I2C_ADDRESSES {
        if !dw_i2c_is_present(addr) {
            continue;
        }
        crate::serial_println!(
            "[I2C] DesignWare en {:#010x} (ver={:#010x}), probando Elan...",
            addr,
            mmio_readl(addr, DW_IC_COMP_VERSION)
        );

        // Init controller and set target to 0x15 (Elan)
        if !dw_i2c_init(addr) {
            continue;
        }

        // Try to read FW version — if TX_ABRT stays 0 we got an ACK
        let fw = dw_i2c_read_word_at(addr, ELAN_CMD_FW_VER);

        let abort = mmio_readl(addr, DW_IC_TX_ABRT_SOURCE);
        if abort != 0 {
            // Device NACKed or other error — wrong controller or no device
            crate::serial_println!("  -> TX_ABRT={:#x} (no hay Elan aqui)", abort);
            continue;
        }

        if let Some(fw_ver) = fw {
            if fw_ver != 0 && fw_ver != 0xFFFF {
                crate::serial_println!("  -> Elan ENCONTRADO! FW: {:#06x}", fw_ver);
                return Some(addr);
            }
            crate::serial_println!("  -> Sin aborto pero FW={:#06x}, continuando...", fw_ver);
        } else {
            crate::serial_println!("  -> dw_i2c_read_word_at falló (timeout/abort)");
        }
    }

    None
}

fn dw_i2c_init(base: usize) -> bool {
    unsafe {
        // 1. Disable controller
        mmio_writel(base, DW_IC_ENABLE, 0);

        // 2. Configure: master, standard speed, 7-bit addr, restart + tx_empty_ctrl enabled
        mmio_writel(
            base,
            DW_IC_CON,
            DW_CON_MASTER
                | DW_CON_SPEED_STD
                | DW_CON_7BIT
                | DW_CON_RESTART_EN
                | DW_CON_TX_EMPTY_CTRL,
        );

        // 3. Set target address (Elan is at 0x15)
        mmio_writel(base, DW_IC_TAR, 0x15);

        // 4. Set timing for ~100kHz standard mode
        // Input clock is typically 50MHz on AMD, so 250 counts = 5us each half = 10us period = 100kHz
        mmio_writel(base, DW_IC_SS_SCL_HCNT, 250);
        mmio_writel(base, DW_IC_SS_SCL_LCNT, 250);

        // 5. Set FIFO thresholds
        mmio_writel(base, DW_IC_TX_TL, 0);
        mmio_writel(base, DW_IC_RX_TL, 0);

        // 6. Disable interrupts (polling mode)
        mmio_writel(base, DW_IC_INTR_MASK, 0);

        // 7. Enable controller
        mmio_writel(base, DW_IC_ENABLE, 1);

        // 8. Wait for enable to take effect
        for _ in 0..1000 {
            if (mmio_readl(base, DW_IC_ENABLE_STATUS) & 1) != 0 {
                break;
            }
        }

        crate::serial_println!(
            "[I2C] Controlador DesignWare inicializado en {:#010x}",
            base
        );
        true
    }
}

/// Low-level read_block that takes an explicit MMIO base
fn dw_i2c_read_block_at(base: usize, cmd: u8, buf: &mut [u8]) -> bool {
    let n = buf.len();
    if n == 0 {
        return false;
    }

    // Clear any pending TX abort
    let _ = mmio_readl(base, DW_IC_TX_ABRT_SOURCE);

    // Wait for TX FIFO empty (all previous commands done)
    let mut timeout = 5000u32;
    while (mmio_readl(base, DW_IC_STATUS) & DW_STAT_TFE) == 0 {
        timeout -= 1;
        if timeout == 0 {
            return false;
        }
    }

    // Combined write+read with TX_EMPTY_CTRL preventing premature STOP
    // Write command byte with RESTART (repeated start after this byte)
    mmio_writel(base, DW_IC_DATA_CMD, (cmd as u32) | DW_CMD_RESTART);

    // Queue ALL read commands — TX_EMPTY_CTRL prevents STOP between them
    for i in 0..n {
        timeout = 1000;
        while (mmio_readl(base, DW_IC_STATUS) & DW_STAT_TFNF) == 0 {
            timeout -= 1;
            if timeout == 0 {
                break;
            }
        }
        let stop = if i == n - 1 { DW_CMD_STOP } else { 0 };
        mmio_writel(base, DW_IC_DATA_CMD, DW_CMD_READ | stop);
    }

    // Read all received bytes from RX FIFO
    for i in 0..n {
        timeout = 10000;
        while (mmio_readl(base, DW_IC_STATUS) & DW_STAT_RFNE) == 0 {
            timeout -= 1;
            if timeout == 0 {
                // Timeout — check if TX_ABRT killed the transaction
                let abort = mmio_readl(base, DW_IC_TX_ABRT_SOURCE);
                I2C_NO_ABRT.store(false, Ordering::Relaxed);
                return false;
            }
        }
        buf[i] = mmio_readl(base, DW_IC_DATA_CMD) as u8;
    }

    // Final check: any silent abort during the transaction?
    let abort = mmio_readl(base, DW_IC_TX_ABRT_SOURCE);
    if abort != 0 {
        I2C_NO_ABRT.store(false, Ordering::Relaxed);
        return false;
    }

    I2C_NO_ABRT.store(true, Ordering::Relaxed);
    true
}

fn dw_i2c_read_block(cmd: u8, buf: &mut [u8]) -> bool {
    let base = I2C_BASE.load(Ordering::Relaxed) as usize;
    if base == 0 {
        return false;
    }
    dw_i2c_read_block_at(base, cmd, buf)
}

fn dw_i2c_read_word(cmd: u8) -> Option<u16> {
    let mut buf = [0u8; 2];
    if dw_i2c_read_block(cmd, &mut buf) {
        Some((buf[1] as u16) << 8 | buf[0] as u16)
    } else {
        None
    }
}

fn dw_i2c_read_word_at(base: usize, cmd: u8) -> Option<u16> {
    let mut buf = [0u8; 2];
    if dw_i2c_read_block_at(base, cmd, &mut buf) {
        Some((buf[1] as u16) << 8 | buf[0] as u16)
    } else {
        None
    }
}

fn dw_i2c_write_byte(cmd: u8, data: u8) -> bool {
    let base = I2C_BASE.load(Ordering::Relaxed) as usize;
    if base == 0 {
        return false;
    }

    // Clear any pending abort
    let _ = mmio_readl(base, DW_IC_TX_ABRT_SOURCE);

    // Wait for TX FIFO empty
    let mut timeout = 5000u32;
    while (mmio_readl(base, DW_IC_STATUS) & DW_STAT_TFE) == 0 {
        timeout -= 1;
        if timeout == 0 {
            return false;
        }
    }

    // Write command byte (register address for Elan)
    mmio_writel(base, DW_IC_DATA_CMD, cmd as u32);

    // Wait for TX FIFO empty (byte sent, TX_EMPTY_CTRL holds STOP)
    timeout = 5000;
    while (mmio_readl(base, DW_IC_STATUS) & DW_STAT_TFE) == 0 {
        timeout -= 1;
        if timeout == 0 {
            return false;
        }
    }

    // Write data byte with stop
    mmio_writel(base, DW_IC_DATA_CMD, data as u32 | DW_CMD_STOP);

    // Wait for completion
    timeout = 5000;
    while (mmio_readl(base, DW_IC_STATUS) & DW_STAT_TFE) == 0 {
        timeout -= 1;
        if timeout == 0 {
            return false;
        }
    }

    // Check for abort
    let abort = mmio_readl(base, DW_IC_TX_ABRT_SOURCE);
    if abort != 0 {
        I2C_NO_ABRT.store(false, Ordering::Relaxed);
        return false;
    }

    true
}

// ── SMBus controller (AMD FCH) ─────────────────────────────────────────

const AMD_SMBUS_VENDOR: u16 = 0x1022;
const AMD_SMBUS_DEVICE: u16 = 0x790b;

// PIIX4-compatible SMBus register offsets (from IO base)
const SMBHSTSTS: u16 = 0x00;
const SMBHSTCNT: u16 = 0x02;
const SMBHSTCMD: u16 = 0x03;
const SMBHSTADD: u16 = 0x04;
const SMBHSTDAT0: u16 = 0x05;
const SMBHSTDAT1: u16 = 0x06;
const SMBHSTBLKDAT: u16 = 0x07;

// Status bits
const STS_HOST_BUSY: u8 = 0x01;
const STS_COMPLETE: u8 = 0x10;
const STS_FAILED: u8 = 0x04 | 0x08;

// Control bits
const CNT_START: u8 = 0x40;
const CNT_PROTO_BYTE_DATA: u8 = 0x02;
const CNT_PROTO_WORD_DATA: u8 = 0x03;
const CNT_PROTO_BLOCK_DATA: u8 = 0x04;
const CNT_PROTO_I2C_BLOCK: u8 = 0x05;

// Elan constants
const ELAN_ADDR: u8 = 0x15;
const ELAN_CMD_REPORT: u8 = 0x00;
const ELAN_CMD_ENABLE: u8 = 0x01;
const ELAN_CMD_FW_VER: u8 = 0x02;
const ELAN_CMD_PRODUCT: u8 = 0x03;
const ELAN_REPORT_LEN: usize = 8;

static SMBUS_IO: AtomicU16 = AtomicU16::new(0);
static SMBUS_OK: AtomicBool = AtomicBool::new(false);
static TOUCHPAD_OK: AtomicBool = AtomicBool::new(false);

// Cached screen dimensions for coordinate scaling
static SCREEN_W: AtomicU16 = AtomicU16::new(0);
static SCREEN_H: AtomicU16 = AtomicU16::new(0);

// Last absolute position for delta computation
static LAST_X: AtomicU16 = AtomicU16::new(0);
static LAST_Y: AtomicU16 = AtomicU16::new(0);
// Last button state
static LAST_BUTTONS: AtomicU16 = AtomicU16::new(0);

// Debug counters and parsed coordinates
static POLL_OK: AtomicU16 = AtomicU16::new(0);
static POLL_FAIL: AtomicU16 = AtomicU16::new(0);
static LAST_REPORT: [AtomicU16; 4] = [
    AtomicU16::new(0),
    AtomicU16::new(0),
    AtomicU16::new(0),
    AtomicU16::new(0),
];
static LAST_PARSED: [AtomicU16; 3] = [AtomicU16::new(0), AtomicU16::new(0), AtomicU16::new(0)];

// ── SMBus primitives (direct IO port access via shim) ──────────────────

/// Quick non-blocking check: is the SMBus controller idle?
fn smbus_is_idle() -> bool {
    let io = SMBUS_IO.load(Ordering::Relaxed);
    let sts = unsafe { crate::linux::io::inb(io + SMBHSTSTS) };
    (sts & STS_HOST_BUSY) == 0
}

fn smbus_wait_idle() -> bool {
    for _ in 0..500 {
        if smbus_is_idle() {
            return true;
        }
    }
    false
}

fn smbus_sts_clear() {
    let io = SMBUS_IO.load(Ordering::Relaxed);
    let sts = unsafe { crate::linux::io::inb(io + SMBHSTSTS) };
    if sts != 0 {
        unsafe { crate::linux::io::outb(io + SMBHSTSTS, sts) };
    }
}

fn smbus_read_block(cmd: u8, buf: &mut [u8], poll_mode: bool) -> Option<usize> {
    if !SMBUS_OK.load(Ordering::Relaxed) {
        return None;
    }
    let io = SMBUS_IO.load(Ordering::Relaxed);

    if !smbus_wait_idle() {
        return None;
    }
    smbus_sts_clear();

    unsafe {
        crate::linux::io::outb(io + SMBHSTCMD, cmd);
        crate::linux::io::outb(io + SMBHSTADD, (ELAN_ADDR << 1) | 1);
        crate::linux::io::outb(io + SMBHSTCNT, CNT_START | CNT_PROTO_BLOCK_DATA);

        let max_loops = if poll_mode { 2000 } else { 5000 };
        for _ in 0..max_loops {
            let sts = crate::linux::io::inb(io + SMBHSTSTS);
            if (sts & STS_COMPLETE) != 0 {
                let n = crate::linux::io::inb(io + SMBHSTDAT0) as usize;
                let n = n.min(buf.len());
                for i in 0..n {
                    buf[i] = crate::linux::io::inb(io + SMBHSTBLKDAT);
                }
                smbus_sts_clear();
                return Some(n);
            }
            if (sts & STS_FAILED) != 0 {
                smbus_sts_clear();
                return None;
            }
        }
    }
    None
}

/// I2C block read: host specifies byte count (no leading byte count from device)
fn smbus_i2c_block_read(cmd: u8, buf: &mut [u8], poll_mode: bool) -> Option<usize> {
    if !SMBUS_OK.load(Ordering::Relaxed) {
        return None;
    }
    let io = SMBUS_IO.load(Ordering::Relaxed);

    if !smbus_wait_idle() {
        return None;
    }
    smbus_sts_clear();

    let count = buf.len().min(32) as u8;

    unsafe {
        crate::linux::io::outb(io + SMBHSTCMD, cmd);
        crate::linux::io::outb(io + SMBHSTADD, (ELAN_ADDR << 1) | 1);
        crate::linux::io::outb(io + SMBHSTCNT, CNT_START | CNT_PROTO_I2C_BLOCK | count);

        let max_loops = if poll_mode { 2000 } else { 5000 };
        for _ in 0..max_loops {
            let sts = crate::linux::io::inb(io + SMBHSTSTS);
            if (sts & STS_COMPLETE) != 0 {
                for i in 0..count as usize {
                    buf[i] = crate::linux::io::inb(io + SMBHSTBLKDAT);
                }
                smbus_sts_clear();
                return Some(count as usize);
            }
            if (sts & STS_FAILED) != 0 {
                smbus_sts_clear();
                return None;
            }
        }
    }
    None
}

fn smbus_read_word(cmd: u8) -> Option<u16> {
    if !SMBUS_OK.load(Ordering::Relaxed) {
        return None;
    }
    let io = SMBUS_IO.load(Ordering::Relaxed);

    if !smbus_wait_idle() {
        return None;
    }
    smbus_sts_clear();

    unsafe {
        crate::linux::io::outb(io + SMBHSTCMD, cmd);
        crate::linux::io::outb(io + SMBHSTADD, (ELAN_ADDR << 1) | 1);
        crate::linux::io::outb(io + SMBHSTCNT, CNT_START | CNT_PROTO_WORD_DATA);

        for _ in 0..500 {
            let sts = crate::linux::io::inb(io + SMBHSTSTS);
            if (sts & STS_COMPLETE) != 0 {
                let lo = crate::linux::io::inb(io + SMBHSTDAT0) as u16;
                let hi = crate::linux::io::inb(io + SMBHSTDAT1) as u16;
                smbus_sts_clear();
                return Some(lo | (hi << 8));
            }
            if (sts & STS_FAILED) != 0 {
                smbus_sts_clear();
                return None;
            }
        }
    }
    None
}

fn smbus_write_byte(cmd: u8, data: u8) -> bool {
    if !SMBUS_OK.load(Ordering::Relaxed) {
        return false;
    }
    let io = SMBUS_IO.load(Ordering::Relaxed);

    if !smbus_wait_idle() {
        return false;
    }
    smbus_sts_clear();

    unsafe {
        crate::linux::io::outb(io + SMBHSTCMD, cmd);
        crate::linux::io::outb(io + SMBHSTADD, ELAN_ADDR << 1);
        crate::linux::io::outb(io + SMBHSTDAT0, data);
        crate::linux::io::outb(io + SMBHSTCNT, CNT_START | CNT_PROTO_BYTE_DATA);

        for _ in 0..500 {
            let sts = crate::linux::io::inb(io + SMBHSTSTS);
            if (sts & STS_COMPLETE) != 0 {
                smbus_sts_clear();
                return true;
            }
            if (sts & STS_FAILED) != 0 {
                smbus_sts_clear();
                return false;
            }
        }
    }
    false
}

// ── SMBus controller discovery ────────────────────────────────────────

fn find_smbus_io_base() -> Option<u16> {
    // Method 1: PCI config offset 0x90 (standard SB800/FCH)
    if let Some(dev) = crate::pci::find_device(AMD_SMBUS_VENDOR, AMD_SMBUS_DEVICE) {
        let reg90 = crate::pci::pci_config_read(dev.bus, dev.device, dev.function, 0x90);
        let val = (reg90 & 0xFFFF) as u16;
        crate::serial_println!("[TOUCHPAD] SMBus PCI cfg 0x90 = {:#06x}", val);
        if val != 0 && (val & 0x01) != 0 {
            let base = val & 0xFFE0;
            crate::serial_println!("[TOUCHPAD] SMBus IO base (PCI cfg): {:#06x}", base);
            return Some(base);
        }

        // Method 2: MMIO at FCH PM base (Ryzen 5000+, rev >= 0x51)
        let rev =
            (crate::pci::pci_config_read(dev.bus, dev.device, dev.function, 0x08) & 0xFF) as u8;
        crate::serial_println!("[TOUCHPAD] SMBus rev={:#04x}", rev);
        if rev >= 0x51 {
            let pm_addr = 0xFED80300usize as *const u32;
            let pm_data = unsafe { pm_addr.read_volatile() };
            crate::serial_println!("[TOUCHPAD] FCH PM decode: {:#010x}", pm_data);
            if (pm_data & 0x10) != 0 {
                let base = (pm_data as u16) & 0xFF00;
                crate::serial_println!("[TOUCHPAD] SMBus IO base (FCH MMIO): {:#06x}", base);
                return Some(base);
            }
        }

        // Method 3: Probe known IO bases
        for &base in &[0x0B00u16, 0x0B20, 0x0E00, 0x0400] {
            let sts = unsafe { crate::linux::io::inb(base + SMBHSTSTS) };
            if sts != 0xFF {
                crate::serial_println!(
                    "[TOUCHPAD] SMBus alive at {:#06x} (sts={:#04x})",
                    base,
                    sts
                );
                return Some(base);
            }
        }
    }

    None
}

// ── Public API ────────────────────────────────────────────────────────

pub fn init(screen_w: u16, screen_h: u16) {
    crate::serial_println!("[TOUCHPAD] Inicializando driver Elan...");

    SCREEN_W.store(screen_w, Ordering::Relaxed);
    SCREEN_H.store(screen_h, Ordering::Relaxed);

    // ── Try I2C (DesignWare) first ──
    // dw_i2c_probe_controller() scans ALL known AMD I2C addresses,
    // initializes each, and probes for Elan (FW version + TX_ABRT check)
    crate::serial_println!("[TOUCHPAD] Buscando controlador I2C DesignWare...");
    if let Some(i2c_base) = dw_i2c_probe_controller() {
        crate::serial_println!(
            "[TOUCHPAD] Controlador I2C con Elan encontrado en {:#010x}",
            i2c_base
        );
        I2C_BASE.store(i2c_base as u32, Ordering::Relaxed);
        I2C_OK.store(true, Ordering::Relaxed);

        // Read FW version (probe already did this, but this confirms link)
        if let Some(fw_ver) = dw_i2c_read_word(ELAN_CMD_FW_VER) {
            crate::serial_println!("[TOUCHPAD] Elan FW: {:#06x}", fw_ver);

            if dw_i2c_write_byte(ELAN_CMD_ENABLE, 0x01) {
                crate::serial_println!("[TOUCHPAD] Touchpad enabled via I2C");
            }

            let mut pid_buf = [0u8; 4];
            if dw_i2c_read_block(ELAN_CMD_PRODUCT, &mut pid_buf) {
                crate::serial_println!(
                    "[TOUCHPAD] Product ID: {:02x}{:02x}{:02x}{:02x}",
                    pid_buf[0],
                    pid_buf[1],
                    pid_buf[2],
                    pid_buf[3]
                );
            }

            TOUCHPAD_OK.store(true, Ordering::Relaxed);
            crate::serial_println!("[TOUCHPAD] INICIALIZADO via I2C");
            return;
        } else {
            crate::serial_println!("[TOUCHPAD] Elan no responde en I2C, probando SMBus...");
        }
    }

    // ── Fallback: SMBus ──
    crate::serial_println!("[TOUCHPAD] Buscando controlador SMBus (fallback)...");
    let io_base = match find_smbus_io_base() {
        Some(base) => {
            SMBUS_IO.store(base, Ordering::Relaxed);
            smbus_sts_clear();
            SMBUS_OK.store(true, Ordering::Relaxed);
            base
        }
        None => {
            crate::serial_println!("[TOUCHPAD] NO SE ENCONTRO controlador SMBus ni I2C");
            crate::serial_println!("[TOUCHPAD] Usando solo mouse PS/2");
            return;
        }
    };
    crate::serial_println!("[TOUCHPAD] SMBus listo en IO {:#06x}", io_base);

    // Probe Elan via SMBus
    let probe = smbus_read_word(ELAN_CMD_FW_VER);
    if probe.is_none() {
        crate::serial_println!("[TOUCHPAD] No hay respuesta Elan en SMBus");
        crate::serial_println!("[TOUCHPAD] Usando solo mouse PS/2");
        return;
    }

    let fw_ver = probe.unwrap();
    crate::serial_println!("[TOUCHPAD] Elan detectado via SMBus! FW: {:#06x}", fw_ver);

    if smbus_write_byte(ELAN_CMD_ENABLE, 0x01) {
        crate::serial_println!("[TOUCHPAD] Touchpad enabled via SMBus");
    }

    let mut pid_buf = [0u8; 4];
    if let Some(n) = smbus_read_block(ELAN_CMD_PRODUCT, &mut pid_buf, false) {
        if n >= 4 {
            crate::serial_println!(
                "[TOUCHPAD] Product ID: {:02x}{:02x}{:02x}{:02x}",
                pid_buf[0],
                pid_buf[1],
                pid_buf[2],
                pid_buf[3]
            );
        }
    }

    TOUCHPAD_OK.store(true, Ordering::Relaxed);
    crate::serial_println!("[TOUCHPAD] INICIALIZADO via SMBus");
}

/// Called from timer interrupt to poll touchpad data
pub fn poll() {
    if !TOUCHPAD_OK.load(Ordering::Relaxed) {
        return;
    }

    let mut buf = [0u8; ELAN_REPORT_LEN];

    // Try I2C first, then SMBus fallback
    let got_data = if I2C_OK.load(Ordering::Relaxed) {
        dw_i2c_read_block(ELAN_CMD_REPORT, &mut buf)
    } else {
        false
    };

    let _n = if got_data {
        ELAN_REPORT_LEN
    } else {
        // Fallback: try SMBus block read
        match smbus_read_block(ELAN_CMD_REPORT, &mut buf, true) {
            Some(n) if n >= ELAN_REPORT_LEN => n,
            _ => match smbus_i2c_block_read(ELAN_CMD_REPORT, &mut buf, true) {
                Some(n) if n >= ELAN_REPORT_LEN => n,
                _ => {
                    POLL_FAIL.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            },
        }
    };

    POLL_OK.fetch_add(1, Ordering::Relaxed);
    // Store last report for debug
    LAST_REPORT[0].store(((buf[1] as u16) << 8) | buf[0] as u16, Ordering::Relaxed);
    LAST_REPORT[1].store(((buf[3] as u16) << 8) | buf[2] as u16, Ordering::Relaxed);
    LAST_REPORT[2].store(((buf[5] as u16) << 8) | buf[4] as u16, Ordering::Relaxed);
    LAST_REPORT[3].store(((buf[7] as u16) << 8) | buf[6] as u16, Ordering::Relaxed);

    // Parse Elan SMBus touch report (8 bytes)
    // Byte 0: report_id (0x00 = touch data)
    // Byte 1: [3:0]=finger_count, [4]=left_btn, [5]=right_btn, [6]=middle_btn
    // Byte 2: X low
    // Byte 3: X high
    // Byte 4: Y low
    // Byte 5: Y high
    // Byte 6: pressure
    // Byte 7: reserved
    let report_id = buf[0];
    if report_id != 0x00 {
        return;
    }

    let btn_finger = buf[1];
    let finger_count = btn_finger & 0x0F;
    let buttons = (btn_finger >> 4) & 0x07;

    let x_raw = (buf[3] as u16) << 8 | buf[2] as u16;
    let y_raw = (buf[5] as u16) << 8 | buf[4] as u16;

    // Store parsed values for debug
    LAST_PARSED[0].store(finger_count as u16, Ordering::Relaxed);
    LAST_PARSED[1].store(x_raw, Ordering::Relaxed);
    LAST_PARSED[2].store(y_raw, Ordering::Relaxed);

    let old_btns = LAST_BUTTONS.load(Ordering::Relaxed) as u8;
    LAST_BUTTONS.store(buttons as u16, Ordering::Relaxed);

    // Detect button changes and inject into mouse event buffer
    if (buttons & 0x01) != (old_btns & 0x01) {
        if (buttons & 0x01) != 0 {
            crate::drivers::mouse::inject_event(crate::drivers::mouse::MouseEvent::ButtonDown(
                crate::drivers::mouse::MouseButton::Left,
            ));
        } else {
            crate::drivers::mouse::inject_event(crate::drivers::mouse::MouseEvent::ButtonUp(
                crate::drivers::mouse::MouseButton::Left,
            ));
        }
    }
    if (buttons & 0x02) != (old_btns & 0x02) {
        if (buttons & 0x02) != 0 {
            crate::drivers::mouse::inject_event(crate::drivers::mouse::MouseEvent::ButtonDown(
                crate::drivers::mouse::MouseButton::Right,
            ));
        } else {
            crate::drivers::mouse::inject_event(crate::drivers::mouse::MouseEvent::ButtonUp(
                crate::drivers::mouse::MouseButton::Right,
            ));
        }
    }

    // If finger detected, inject relative movement
    if finger_count > 0 && x_raw > 0 && y_raw > 0 {
        let lx = LAST_X.load(Ordering::Relaxed);
        let ly = LAST_Y.load(Ordering::Relaxed);

        if lx != 0 && ly != 0 {
            // Compute relative delta from absolute positions
            let mut dx = x_raw as i32 - lx as i32;
            let mut dy = y_raw as i32 - ly as i32;

            // Apply sensitivity scaling for smooth cursor movement
            // Touchpad range: X ~0-3472, Y ~0-1776 (from Linux evtest)
            // At 62.5Hz polling, a fast swipe (~500cts in ~0.3s) = ~100 cts/tick
            const SENSITIVITY: i32 = 6;
            dx = dx * SENSITIVITY / 2;
            dy = dy * SENSITIVITY / 2;

            // Clamp to prevent cursor jumps
            dx = dx.clamp(-100, 100);
            dy = dy.clamp(-100, 100);

            if dx != 0 || dy != 0 {
                crate::drivers::mouse::inject_event(crate::drivers::mouse::MouseEvent::Move(
                    dx, dy,
                ));
            }
        }

        LAST_X.store(x_raw, Ordering::Relaxed);
        LAST_Y.store(y_raw, Ordering::Relaxed);
    } else {
        LAST_X.store(0, Ordering::Relaxed);
        LAST_Y.store(0, Ordering::Relaxed);
    }
}

pub fn is_present() -> bool {
    TOUCHPAD_OK.load(Ordering::Relaxed)
}

pub fn status() -> alloc::string::String {
    use alloc::format;
    let i2c_ok = I2C_OK.load(Ordering::Relaxed);
    let smbus_ok = SMBUS_OK.load(Ordering::Relaxed);
    let tp_ok = TOUCHPAD_OK.load(Ordering::Relaxed);
    let io_base = SMBUS_IO.load(Ordering::Relaxed);
    let i2c_base = I2C_BASE.load(Ordering::Relaxed);
    let poll_ok = POLL_OK.load(Ordering::Relaxed);
    let poll_fail = POLL_FAIL.load(Ordering::Relaxed);
    let r0 = LAST_REPORT[0].load(Ordering::Relaxed);
    let r1 = LAST_REPORT[1].load(Ordering::Relaxed);
    let r2 = LAST_REPORT[2].load(Ordering::Relaxed);
    let r3 = LAST_REPORT[3].load(Ordering::Relaxed);
    let fc = LAST_PARSED[0].load(Ordering::Relaxed);
    let px = LAST_PARSED[1].load(Ordering::Relaxed);
    let py = LAST_PARSED[2].load(Ordering::Relaxed);
    let no_abort = I2C_NO_ABRT.load(Ordering::Relaxed);
    format!(
        "Touchpad Elan:\n  I2C: {} @ {:#010x} (no_abort={})\n  SMBus: {} @ {:#06x}\n  Touchpad: {}\n  Polls (ok={}, fail={})\n  Raw: [{:#04x} {:#04x} {:#04x} {:#04x}]\n  Parsed: fingers={}, X={}, Y={}",
        if i2c_ok { "OK" } else { "NO" },
        i2c_base,
        no_abort,
        if smbus_ok { "OK" } else { "NO" },
        io_base,
        if tp_ok { "PRESENTE" } else { "AUSENTE" },
        poll_ok,
        poll_fail,
        r0, r1, r2, r3,
        fc, px, py,
    )
}
