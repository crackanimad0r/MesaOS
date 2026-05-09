// mesa_kernel/src/drivers/battery.rs

use x86_64::instructions::port::Port;
use spin::Mutex;

const EC_DAT: u16 = 0x62;
const EC_CMD: u16 = 0x66;

// EC Status bits
const EC_IBF: u8 = 0x02; // Input Buffer Full (EC is reading)
const EC_OBF: u8 = 0x01; // Output Buffer Full (EC has written)

const EC_CMD_READ: u8 = 0x80;

#[derive(Debug, Clone, Copy)]
pub struct BatteryStatus {
    pub percentage: u8,
    pub is_charging: bool,
    pub present: bool,
}

static BATTERY_LOCK: Mutex<()> = Mutex::new(());

fn wait_ec_ibf() -> bool {
    let mut cmd_port: Port<u8> = Port::new(EC_CMD);
    for _ in 0..100000 {
        let status = unsafe { cmd_port.read() };
        // Si IBF es 0, el buffer de entrada está vacío y podemos escribir
        if (status & EC_IBF) == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn wait_ec_obf() -> bool {
    let mut cmd_port: Port<u8> = Port::new(EC_CMD);
    for _ in 0..100000 {
        let status = unsafe { cmd_port.read() };
        // Si OBF es 1, el buffer de salida está lleno y podemos leer
        if (status & EC_OBF) != 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn ec_read(addr: u8) -> Option<u8> {
    unsafe {
        let mut cmd_port: Port<u8> = Port::new(EC_CMD);
        let mut dat_port: Port<u8> = Port::new(EC_DAT);

        if !wait_ec_ibf() { return None; }
        cmd_port.write(EC_CMD_READ);

        if !wait_ec_ibf() { return None; }
        dat_port.write(addr);

        if !wait_ec_obf() { return None; }
        Some(dat_port.read())
    }
}

pub fn init() {
    crate::serial_println!("[BATTERY] Inicializando driver de batería...");
    
    let status = read_status();
    if status.present {
        crate::serial_println!("[BATTERY] Batería detectada: {}% (Cargando: {})", status.percentage, status.is_charging);
    } else {
        crate::serial_println!("[BATTERY] Usando simulación de batería (EC no soportado o ausente)");
    }
}

pub fn read_status() -> BatteryStatus {
    let _lock = BATTERY_LOCK.lock();

    // Intentar leer de direcciones EC comunes para batería.
    // HP y otros portátiles suelen tener el nivel en registros alrededor de 0xB0 o 0x87, pero varía mucho.
    // Intentaremos leer un registro genérico. Si falla por timeout, retornamos fallback.
    
    // Aquí implementamos una lectura cautelosa. Si el EC no está (como en QEMU sin soporte explícito),
    // wait_ec_ibf fallará o devolverá 0xFF.
    
    // Dado que tu placa es una HP 887A (Victus/Omen), el registro 0xB0 generalmente
    // contiene la temperatura del CPU (por eso oscilaba entre 49 y 66).
    // El porcentaje de batería en portátiles HP modernos suele estar en 0x90, 0x89 o 0x8C.
    
    let mut present = false;
    let mut percentage = 100;
    let mut is_charging = true;

    // Probar offsets comunes de HP (Omen/Victus suelen usar 0x8C o 0x89 para batería)
    // Se elimina 0x90 ya que daba falsos positivos (8%).
    let hp_offsets = [0xD0, 0xD8];
    
    for &offset in &hp_offsets {
        if let Some(val) = ec_read(offset) {
            // Un porcentaje válido está entre 0 y 100, y típicamente no oscila locamente.
            if val <= 100 && val > 0 {
                percentage = val;
                present = true;
                break;
            }
        }
    }

    // Intentar leer el estado de carga en el registro siguiente al encontrado o en offsets comunes
    if present {
        // En HP el estado suele estar cerca o en 0x8A / 0x8D
        if let Some(state) = ec_read(0x8A) {
            is_charging = (state & 0x01) != 0;
        }
    }

    BatteryStatus {
        percentage,
        is_charging,
        present,
    }
}
