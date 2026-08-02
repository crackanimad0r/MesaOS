// mesa_kernel/src/drivers/battery.rs
//! Driver de batería para MesaOS con reporte avanzado estilo Linux.
//!
//! Estrategia en dos capas:
//!   1. **AML** (`_BST`/`_BIF`/`_BTP`): Si el contexto AML está disponible, evaluamos
//!      los métodos estándar de ACPI para obtener el máximo detalle.
//!   2. **EC Fallback**: Si AML falla, leemos directamente el Embedded Controller
//!      usando los offsets estándar de HP/ThinkPad/Dell.
//!
//! El reporte `BatteryReport` expone toda la información disponible en una
//! estructura apta para imprimir estilo `upower -i` o `acpi -V`.

extern crate alloc;

use alloc::string::String;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use x86_64::instructions::port::Port;

// ── Constantes del Embedded Controller ───────────────────────────────────────

const EC_DAT: u16 = 0x62;
const EC_CMD: u16 = 0x66;
const EC_IBF: u8 = 0x02;
const EC_OBF: u8 = 0x01;
const EC_CMD_READ: u8 = 0x80;
const EC_CMD_WRITE: u8 = 0x81;

// ── Tipos públicos ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryTechnology {
    Unknown,
    LiIon,
    LiPoly,
    LiFe,
    NiCd,
    NiMH,
    LeadAcid,
    Rechargeable,
    Primary,
}

impl BatteryTechnology {
    pub fn as_str(&self) -> &'static str {
        match self {
            BatteryTechnology::Unknown => "Unknown",
            BatteryTechnology::LiIon => "Li-ion",
            BatteryTechnology::LiPoly => "Li-poly",
            BatteryTechnology::LiFe => "LiFe",
            BatteryTechnology::NiCd => "NiCd",
            BatteryTechnology::NiMH => "NiMH",
            BatteryTechnology::LeadAcid => "Lead-acid",
            BatteryTechnology::Rechargeable => "Rechargeable",
            BatteryTechnology::Primary => "Primary (non-rechargeable)",
        }
    }

    fn from_acpi(code: u64) -> Self {
        match code {
            0 => BatteryTechnology::Primary,
            1 => BatteryTechnology::Rechargeable,
            2 => BatteryTechnology::Unknown,
            _ => BatteryTechnology::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargingState {
    Unknown,
    Charging,
    Discharging,
    Full,
    NotCharging,
    /// Batería ausente o en estado crítico
    Absent,
}

impl ChargingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChargingState::Unknown => "unknown",
            ChargingState::Charging => "charging",
            ChargingState::Discharging => "discharging",
            ChargingState::Full => "fully-charged",
            ChargingState::NotCharging => "not charging",
            ChargingState::Absent => "absent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Unknown,
    Good,
    Overheat,
    Dead,
    Overvoltage,
    UnspecFailure,
    Cold,
    WatchdogTimerExpired,
    SafetyTimerExpired,
    /// Calculado a partir del desgaste (SOH)
    Worn(u8), // porcentaje
}

impl Health {
    pub fn as_str(&self) -> &'static str {
        match self {
            Health::Good => "good",
            Health::Overheat => "overheat",
            Health::Dead => "dead",
            Health::Overvoltage => "overvoltage",
            Health::UnspecFailure => "unspec failure",
            Health::Cold => "cold",
            Health::WatchdogTimerExpired => "watchdog timer expired",
            Health::SafetyTimerExpired => "safety timer expired",
            Health::Worn(p) => match p {
                90..=100 => "good",
                70..=89 => "fair",
                50..=69 => "poor",
                _ => "critical",
            },
            Health::Unknown => "unknown",
        }
    }
}

/// Estado mínimo de batería (compatibilidad con código existente)
#[derive(Debug, Clone, Copy)]
pub struct BatteryStatus {
    pub percentage: u8,
    pub is_charging: bool,
    pub present: bool,
    /// `true` si los datos provienen de AML `_BST` (más fiable)
    pub via_aml: bool,
}

/// Reporte completo de batería estilo Linux (`upower -i`)
#[derive(Debug, Clone)]
pub struct BatteryReport {
    /// Batería físicamente presente
    pub present: bool,
    /// Porcentaje 0..100
    pub percentage: u8,
    /// Estado de carga
    pub state: ChargingState,
    /// True si hay corriente entrando (cargando)
    pub is_charging: bool,
    /// Salud declarada
    pub health: Health,

    // Capacidades (mWh o mAh según unit)
    pub design_capacity: u64,
    pub full_charge: u64,
    pub current_capacity: u64,
    /// 0 = mA·h, 1 = mW·h
    pub capacity_unit: u8,
    pub unit_name: &'static str,

    // Eléctrico
    pub voltage_mv: u32, // mV
    pub rate_mw: i32,    // mW, positivo=cargando, negativo=descargando

    // Tiempo
    pub time_to_full_sec: u32,
    pub time_to_empty_sec: u32,

    // Térmico (décimas de °C)
    pub temperature_dc: i32,

    // Identidad
    pub model: String,
    pub serial: String,
    pub oem_info: String,
    pub manufacturer: String,
    pub technology: BatteryTechnology,
    pub design_voltage_mv: u32,
    pub design_capacity_warning: u64,
    pub design_capacity_low: u64,

    // Desgaste
    pub cycle_count: Option<u32>,
    pub soh_percent: Option<u8>,

    // Metadatos
    pub via_aml: bool,
    pub source: &'static str, // "AML" / "EC" / "Mock"
}

static BATTERY_LOCK: Mutex<()> = Mutex::new(());
static AML_BATTERY_MISSING: AtomicBool = AtomicBool::new(false);

// ── Primitivas del Embedded Controller (públicas para aml_handler) ────────────

fn wait_ec_ibf() -> bool {
    let mut cmd: Port<u8> = Port::new(EC_CMD);
    for _ in 0..100_000 {
        let s = unsafe { cmd.read() };
        if (s & EC_IBF) == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn wait_ec_obf() -> bool {
    let mut cmd: Port<u8> = Port::new(EC_CMD);
    for _ in 0..100_000 {
        let s = unsafe { cmd.read() };
        if (s & EC_OBF) != 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// Lee un byte del Embedded Controller.
/// Expuesta públicamente para uso desde `aml_handler`.
pub fn ec_read(addr: u8) -> Option<u8> {
    unsafe {
        let mut cmd: Port<u8> = Port::new(EC_CMD);
        let mut dat: Port<u8> = Port::new(EC_DAT);
        if !wait_ec_ibf() {
            return None;
        }
        cmd.write(EC_CMD_READ);
        if !wait_ec_ibf() {
            return None;
        }
        dat.write(addr);
        if !wait_ec_obf() {
            return None;
        }
        Some(dat.read())
    }
}

/// Escribe un byte en el Embedded Controller.
/// Expuesta públicamente para uso desde `aml_handler`.
pub fn ec_write(addr: u8, value: u8) -> Option<()> {
    unsafe {
        let mut cmd: Port<u8> = Port::new(EC_CMD);
        let mut dat: Port<u8> = Port::new(EC_DAT);
        if !wait_ec_ibf() {
            return None;
        }
        cmd.write(EC_CMD_WRITE);
        if !wait_ec_ibf() {
            return None;
        }
        dat.write(addr);
        if !wait_ec_ibf() {
            return None;
        }
        dat.write(value);
        Some(())
    }
}

/// Lee una word (16 bits) en little-endian del EC.
pub fn ec_read_word(addr: u8) -> Option<u16> {
    let lo = ec_read(addr)?;
    let hi = ec_read(addr.wrapping_add(1))?;
    Some(((hi as u16) << 8) | (lo as u16))
}

// ── Inicialización ────────────────────────────────────────────────────────────

pub fn init() {
    crate::serial_println!("[BATTERY] Inicializando driver de batería...");
    // IMPORTANTE: init() se llama antes de memory::init() (sin heap disponible).
    // Usamos SOLO el EC directo, que no requiere alocación dinámica.
    // read_status() (con AML) se usará después desde update_status() en el shell loop.
    let status = try_ec_direct_status();
    if status.present {
        crate::serial_println!(
            "[BATTERY] {}% EC directo (cargando: {})",
            status.percentage,
            status.is_charging
        );
    } else {
        crate::serial_println!("[BATTERY] Sin batería detectada (desktop / VM)");
    }
}

// ── Lectura AML `_BST`/`_BIF`/`_BTP` ──────────────────────────────────────────

fn aml_string_value(v: aml::AmlValue) -> Option<String> {
    use aml::value::AmlValue;
    match v {
        AmlValue::String(s) => {
            // La cadena AML puede traer padding NUL; la limpiamos
            let trimmed: String = s.chars().filter(|c| *c != '\0').collect();
            Some(trimmed)
        }
        AmlValue::Integer(i) => Some(alloc::string::ToString::to_string(&i)),
        // Buffer/Integer/Boolean ya se manejan arriba; cualquier otro caso → None
        _ => None,
    }
}

/// Intenta evaluar `_BIF` y devuelve la información estática de la batería.
fn try_aml_bif(
    ctx: &mut aml::AmlContext,
    bat_path: &str,
) -> Option<(
    (u64, u8),
    BatteryTechnology,
    u32,
    u64,
    u64,
    u64,
    u64,
    String,
    String,
    String,
    String,
)> {
    use aml::{value::AmlValue, AmlName};
    let bif_str = alloc::format!("{}._BIF", bat_path);
    let name = AmlName::from_str(&bif_str).ok()?;
    let v = ctx.invoke_method(&name, aml::value::Args::EMPTY).ok()?;
    let pkg = match v {
        AmlValue::Package(p) => p,
        _ => return None,
    };
    if pkg.len() < 13 {
        return None;
    }

    // _BIF layout (ACPI 6.5 §11.1.2):
    //  0: Power Unit (0=mA·h, 1=mW·h)
    //  1: Design Capacity
    //  2: Last Full Charge Capacity
    //  3: Battery Technology (0=Primary, 1=Rechargeable, 2=Secondary)
    //  4: Design Voltage (mV)
    //  5: Design Capacity Warning
    //  6: Design Capacity Low
    //  7: Capacity Granularity 1
    //  8: Capacity Granularity 2
    //  9: Model Number
    // 10: Serial Number
    // 11: Battery Type
    // 12: OEM Info
    let unit = match &pkg[0] {
        AmlValue::Integer(i) => *i as u8,
        _ => 0,
    };
    let design_cap = match &pkg[1] {
        AmlValue::Integer(i) => *i,
        _ => 0,
    };
    let full_cap = match &pkg[2] {
        AmlValue::Integer(i) => *i,
        _ => 0,
    };
    let tech_code = match &pkg[3] {
        AmlValue::Integer(i) => *i,
        _ => 2,
    };
    let design_voltage = match &pkg[4] {
        AmlValue::Integer(i) => *i as u32,
        _ => 0,
    };
    let warn_cap = match &pkg[5] {
        AmlValue::Integer(i) => *i,
        _ => 0,
    };
    let low_cap = match &pkg[6] {
        AmlValue::Integer(i) => *i,
        _ => 0,
    };
    let model = aml_string_value(pkg[9].clone()).unwrap_or_default();
    let serial = aml_string_value(pkg[10].clone()).unwrap_or_default();
    let btype = aml_string_value(pkg[11].clone()).unwrap_or_default();
    let oem = aml_string_value(pkg[12].clone()).unwrap_or_default();

    // Mapeo de tecnología: si _BIF trae string, lo usamos
    let technology = if !btype.is_empty() {
        let bt = btype.to_uppercase();
        if bt.contains("LION") || bt.contains("LI-ION") {
            BatteryTechnology::LiIon
        } else if bt.contains("LIPOLY") || bt.contains("LI-POLY") || bt.contains("LIPO") {
            BatteryTechnology::LiPoly
        } else if bt.contains("LIFE") || bt.contains("LIFEPO4") {
            BatteryTechnology::LiFe
        } else if bt.contains("NICD") {
            BatteryTechnology::NiCd
        } else if bt.contains("NIMH") {
            BatteryTechnology::NiMH
        } else if bt.contains("LEAD") || bt.contains("PB") || bt.contains("ACID") {
            BatteryTechnology::LeadAcid
        } else if bt.contains("PRIMARY") {
            BatteryTechnology::Primary
        } else {
            BatteryTechnology::Unknown
        }
    } else {
        BatteryTechnology::from_acpi(tech_code)
    };

    Some((
        (design_cap, unit),
        technology,
        design_voltage,
        warn_cap,
        low_cap,
        full_cap,
        0,
        model,
        serial,
        btype,
        oem,
    ))
}

/// Intenta evaluar `_BIF` extendido (HP/Modern) que añade Cycle Count al final.
fn try_aml_bif_ext(ctx: &mut aml::AmlContext, bat_path: &str) -> Option<u32> {
    use aml::{value::AmlValue, AmlName};
    let bif_str = alloc::format!("{}._BIF", bat_path);
    let name = AmlName::from_str(&bif_str).ok()?;
    let v = ctx.invoke_method(&name, aml::value::Args::EMPTY).ok()?;
    let pkg = match v {
        AmlValue::Package(p) => p,
        _ => return None,
    };
    if pkg.len() < 14 {
        return None;
    }
    match &pkg[13] {
        AmlValue::Integer(i) if *i < 0xFFFF_FFFF => Some(*i as u32),
        _ => None,
    }
}

/// Intenta evaluar `_BST` y devuelve estado dinámico.
fn try_aml_bst(
    ctx: &mut aml::AmlContext,
    bat_path: &str,
) -> Option<(ChargingState, u64, u64, u32)> {
    use aml::{value::AmlValue, AmlName};
    let bst_str = alloc::format!("{}._BST", bat_path);
    let name = AmlName::from_str(&bst_str).ok()?;
    let v = ctx.invoke_method(&name, aml::value::Args::EMPTY).ok()?;
    let pkg = match v {
        AmlValue::Package(p) => p,
        _ => return None,
    };
    if pkg.len() < 4 {
        return None;
    }

    // _BST layout (ACPI 6.5 §11.1.3):
    //  0: Battery State (bit0=charging, bit1=discharging, bit2=critical)
    //  1: Battery Present Rate (mA or mW; 0xFFFFFFFF si desconocido)
    //  2: Battery Remaining Capacity
    //  3: Battery Present Voltage (mV)
    let state_bits = match &pkg[0] {
        AmlValue::Integer(i) => *i as u32,
        _ => return None,
    };
    let rate = match &pkg[1] {
        AmlValue::Integer(i) => *i,
        _ => 0,
    };
    let remaining = match &pkg[2] {
        AmlValue::Integer(i) => *i,
        _ => return None,
    };
    let voltage = match &pkg[3] {
        AmlValue::Integer(i) => *i as u32,
        _ => 0,
    };

    // Decodificar estado (ACPI spec):
    //  bit0: discharging
    //  bit1: charging
    //  bit2: critical (capacidad baja)
    // Si no hay ningún bit puesto y remaining == full, está "Full"
    let state = if state_bits & 0x04 != 0 {
        ChargingState::Discharging // critical
    } else if state_bits & 0x01 != 0 {
        ChargingState::Discharging
    } else if state_bits & 0x02 != 0 {
        ChargingState::Charging
    } else {
        ChargingState::Full
    };
    Some((state, rate, remaining, voltage))
}

/// Compone un `BatteryReport` usando AML cuando está disponible.
fn try_aml_report() -> Option<BatteryReport> {
    if AML_BATTERY_MISSING.load(Ordering::Relaxed) {
        return None;
    }
    let bat_paths = [r"\_SB.BAT0", r"\_SB.BAT1", r"\_SB.PCI0.BAT0", r"\_SB.PWRB"];
    let mut saw_aml_context = false;

    for bat_path in &bat_paths {
        let result = crate::acpi::with_aml(|ctx| -> Result<BatteryReport, aml::AmlError> {
            // 1) _BST: estado dinámico
            let (state, rate, remaining, voltage) =
                try_aml_bst(ctx, bat_path).ok_or(aml::AmlError::Unimplemented)?;

            // 2) _BIF: información estática
            let (
                caps_unit,
                technology,
                design_voltage,
                warn_cap,
                low_cap,
                full_cap,
                _gran1,
                model,
                serial,
                _btype_str,
                oem,
            ) = try_aml_bif(ctx, bat_path).ok_or(aml::AmlError::Unimplemented)?;

            let (design_cap, unit) = caps_unit;
            let unit_name = if unit == 1 { "mWh" } else { "mAh" };

            // Porcentaje: usar remaining/full cuando full es válido
            let percentage: u8 = if full_cap > 0 && remaining <= full_cap {
                ((remaining * 100) / full_cap) as u8
            } else {
                99
            };

            // Rate (en mW si unit==1, si no, mA — convertimos a mW con voltage)
            // _BST entrega rate en la misma unidad que _BIF.
            let rate_mw: i32 = if rate == 0 || rate == 0xFFFF_FFFF || rate == 0x7FFF_FFFF {
                0
            } else if unit == 1 {
                // mW
                if state == ChargingState::Charging {
                    rate as i32
                } else if state == ChargingState::Discharging {
                    -(rate as i32)
                } else {
                    0
                }
            } else {
                // mA → mW con voltage
                let v = voltage as i32;
                if v > 0 {
                    let mw = ((rate as i32) * v) / 1000;
                    if state == ChargingState::Charging {
                        mw
                    } else if state == ChargingState::Discharging {
                        -mw
                    } else {
                        0
                    }
                } else {
                    0
                }
            };

            // Tiempos (rate viene en mW o mA; _BIF capacity en misma unidad)
            let (time_to_full, time_to_empty) = if rate > 0 && rate != 0xFFFF_FFFF {
                if state == ChargingState::Charging && full_cap > remaining {
                    // segundos = (full - remaining) * 3600 / rate
                    let secs = ((full_cap - remaining) as u64) * 3600 / rate as u64;
                    (secs as u32, 0u32)
                } else if state == ChargingState::Discharging && remaining > 0 {
                    let secs = ((remaining as u64) * 3600) / rate as u64;
                    (0u32, secs as u32)
                } else {
                    (0u32, 0u32)
                }
            } else {
                (0u32, 0u32)
            };

            // Cycle count (extensión BIF[13])
            let cycle_count = try_aml_bif_ext(ctx, bat_path);

            // SOH = (full_cap / design_cap) * 100
            let soh = if design_cap > 0 && full_cap > 0 && full_cap <= design_cap {
                Some(((full_cap * 100) / design_cap) as u8)
            } else {
                None
            };

            let health = match soh {
                Some(p) => Health::Worn(p),
                None => Health::Good,
            };

            Ok(BatteryReport {
                present: true,
                percentage,
                state,
                is_charging: state == ChargingState::Charging,
                health,
                design_capacity: design_cap,
                full_charge: full_cap,
                current_capacity: remaining,
                capacity_unit: unit,
                unit_name,
                voltage_mv: voltage,
                rate_mw,
                time_to_full_sec: time_to_full,
                time_to_empty_sec: time_to_empty,
                temperature_dc: 0,
                model,
                serial,
                oem_info: oem,
                manufacturer: String::new(),
                technology,
                design_voltage_mv: design_voltage,
                design_capacity_warning: warn_cap,
                design_capacity_low: low_cap,
                cycle_count,
                soh_percent: soh,
                via_aml: true,
                source: "AML",
            })
        });

        match result {
            Some(Ok(r)) => {
                crate::serial_println!(
                    "[BATTERY] AML OK: {}% rate={}mW v={}mV ({} _BIF)",
                    r.percentage,
                    r.rate_mw,
                    r.voltage_mv,
                    r.unit_name
                );
                return Some(r);
            }
            Some(Err(_)) => {
                saw_aml_context = true;
            }
            None => {} // AML ctx no disponible aún
        }
    }
    if saw_aml_context {
        AML_BATTERY_MISSING.store(true, Ordering::Relaxed);
    }
    None
}

// ── Fallback EC directo ───────────────────────────────────────────────────────

/// Estado mínimo leido del EC (compatibilidad con código pre-existente)
fn try_ec_direct_status() -> BatteryStatus {
    let hp_offsets = [0xD0u8, 0xD8u8];
    let mut percentage = 0u8;
    let mut is_charging = false;
    let mut present = false;

    for &off in &hp_offsets {
        if let Some(v) = ec_read(off) {
            if v > 0 && v <= 100 {
                percentage = v;
                present = true;
                break;
            }
        }
    }
    if present {
        if let Some(s) = ec_read(0x8A) {
            is_charging = (s & 0x01) != 0;
        }
    }
    BatteryStatus {
        percentage,
        is_charging,
        present,
        via_aml: false,
    }
}

/// Reporte desde el EC. Mucho más rico que `try_ec_direct_status`.
fn try_ec_direct_report() -> BatteryReport {
    // Mapeo de offsets EC (HP / ThinkPad / ASUS / Dell) más comunes
    // SLOT 1 (HP)
    const OFF_PCT_S1: u8 = 0xD0;
    const OFF_REMAIN_S1: u8 = 0xD1;
    const OFF_DESIGN_S1: u8 = 0xD2;
    const OFF_FULL_S1: u8 = 0xD3;
    const OFF_VOLT_S1: u8 = 0xD4;
    const OFF_RATE_S1: u8 = 0xD5;
    const OFF_TEMP_S1: u8 = 0xD6;
    const OFF_STATUS_S1: u8 = 0xD7;
    // SLOT 2 (HP secundario / ThinkPad)
    const OFF_PCT_S2: u8 = 0xD8;
    const OFF_REMAIN_S2: u8 = 0xD9;
    const OFF_DESIGN_S2: u8 = 0xDA;
    const OFF_FULL_S2: u8 = 0xDB;
    const OFF_VOLT_S2: u8 = 0xDC;
    const OFF_RATE_S2: u8 = 0xDD;
    const OFF_TEMP_S2: u8 = 0xDE;
    const OFF_STATUS_S2: u8 = 0xDF;

    // Detectamos qué slot tiene una batería conectada
    let (pct, remain, design, full, voltage_raw, rate_raw, temp_raw, status_byte) = {
        let s1 = ec_read(OFF_PCT_S1);
        if let Some(p) = s1 {
            if p > 0 && p <= 100 {
                (
                    p,
                    ec_read(OFF_REMAIN_S1),
                    ec_read(OFF_DESIGN_S1),
                    ec_read(OFF_FULL_S1),
                    ec_read_word(OFF_VOLT_S1),
                    ec_read_word(OFF_RATE_S1),
                    ec_read(OFF_TEMP_S1),
                    ec_read(OFF_STATUS_S1),
                )
            } else {
                let s2 = ec_read(OFF_PCT_S2);
                if let Some(p2) = s2 {
                    if p2 > 0 && p2 <= 100 {
                        (
                            p2,
                            ec_read(OFF_REMAIN_S2),
                            ec_read(OFF_DESIGN_S2),
                            ec_read(OFF_FULL_S2),
                            ec_read_word(OFF_VOLT_S2),
                            ec_read_word(OFF_RATE_S2),
                            ec_read(OFF_TEMP_S2),
                            ec_read(OFF_STATUS_S2),
                        )
                    } else {
                        return mock_report();
                    }
                } else {
                    return mock_report();
                }
            }
        } else {
            return mock_report();
        }
    };

    let design_cap = design.unwrap_or(0) as u64;
    let full_cap = full.unwrap_or(0) as u64;
    let remain_cap = remain.unwrap_or(0) as u64;
    let voltage_mv = voltage_raw.unwrap_or(0) as u32;
    let rate_raw_v = rate_raw.unwrap_or(0);
    let temp_k = temp_raw.unwrap_or(0);
    let status_bits = status_byte.unwrap_or(0);

    // Estado de carga
    // Bit 0: charging  |  Bit 1: discharging  |  Bit 2: critical
    let (state, is_charging) = if status_bits & 0x01 != 0 {
        (ChargingState::Charging, true)
    } else if status_bits & 0x02 != 0 {
        (ChargingState::Discharging, false)
    } else if pct == 100 {
        (ChargingState::Full, false)
    } else {
        (ChargingState::NotCharging, false)
    };

    // Rate (mW = mA * mV / 1000). El EC entrega mA en la mayoría de firmware.
    let rate_mw: i32 = if rate_raw_v == 0 || rate_raw_v == 0xFFFF {
        0
    } else if voltage_mv > 0 {
        let mw = ((rate_raw_v as i32) * (voltage_mv as i32)) / 1000;
        if is_charging {
            mw
        } else {
            -mw
        }
    } else {
        0
    };

    // Temperatura en décimas de °C: convención EC típica Kelvin - 2731
    let temperature_dc: i32 = if temp_k > 0 {
        (temp_k as i32) * 10 - 2731
    } else {
        0
    };

    // Tiempos
    let (time_to_full, time_to_empty) = if rate_raw_v > 0 && rate_raw_v != 0xFFFF {
        if is_charging && full_cap > remain_cap {
            let secs = ((full_cap - remain_cap) as u64) * 3600 / rate_raw_v as u64;
            (secs as u32, 0u32)
        } else if !is_charging && remain_cap > 0 {
            let secs = ((remain_cap as u64) * 3600) / rate_raw_v as u64;
            (0u32, secs as u32)
        } else {
            (0u32, 0u32)
        }
    } else {
        (0u32, 0u32)
    };

    let soh = if design_cap > 0 && full_cap > 0 && full_cap <= design_cap {
        Some(((full_cap * 100) / design_cap) as u8)
    } else {
        None
    };

    let health = match soh {
        Some(p) => Health::Worn(p),
        None => Health::Unknown,
    };

    BatteryReport {
        present: true,
        percentage: pct,
        state,
        is_charging,
        health,
        design_capacity: design_cap,
        full_charge: full_cap,
        current_capacity: remain_cap,
        capacity_unit: 0, // 0 = mAh (convención EC)
        unit_name: "mAh",
        voltage_mv,
        rate_mw,
        time_to_full_sec: time_to_full,
        time_to_empty_sec: time_to_empty,
        temperature_dc,
        model: String::from("EC Battery"),
        serial: String::new(),
        oem_info: String::new(),
        manufacturer: String::new(),
        technology: BatteryTechnology::Unknown,
        design_voltage_mv: 0,
        design_capacity_warning: 0,
        design_capacity_low: 0,
        cycle_count: None,
        soh_percent: soh,
        via_aml: false,
        source: "EC",
    }
}

/// Reporte “mock” para cuando NO hay batería (desktop / VM).
/// Devuelve `present: false` y datos neutros.
fn mock_report() -> BatteryReport {
    BatteryReport {
        present: false,
        percentage: 0,
        state: ChargingState::Absent,
        is_charging: false,
        health: Health::Unknown,
        design_capacity: 0,
        full_charge: 0,
        current_capacity: 0,
        capacity_unit: 0,
        unit_name: "mAh",
        voltage_mv: 0,
        rate_mw: 0,
        time_to_full_sec: 0,
        time_to_empty_sec: 0,
        temperature_dc: 0,
        model: String::from("N/A"),
        serial: String::new(),
        oem_info: String::new(),
        manufacturer: String::new(),
        technology: BatteryTechnology::Unknown,
        design_voltage_mv: 0,
        design_capacity_warning: 0,
        design_capacity_low: 0,
        cycle_count: None,
        soh_percent: None,
        via_aml: false,
        source: "None",
    }
}

// ── API pública ───────────────────────────────────────────────────────────────

/// Compatibilidad: estado mínimo de la batería (lo usa la status bar).
pub fn read_status() -> BatteryStatus {
    let _lock = BATTERY_LOCK.lock();
    // 1) Si el contexto AML está disponible, intentar el camino AML
    if let Some(r) = try_aml_report() {
        return BatteryStatus {
            percentage: r.percentage,
            is_charging: r.is_charging,
            present: r.present,
            via_aml: true,
        };
    }
    // 2) Fallback EC
    let r = try_ec_direct_report();
    BatteryStatus {
        percentage: r.percentage,
        is_charging: r.is_charging,
        present: r.present,
        via_aml: false,
    }
}

/// Devuelve el reporte completo de la batería. Equivalente a `upower -i` / `acpi -V`.
pub fn read_report() -> BatteryReport {
    let _lock = BATTERY_LOCK.lock();
    if let Some(r) = try_aml_report() {
        return r;
    }
    try_ec_direct_report()
}

// ── Renderizado del reporte estilo Linux ──────────────────────────────────────

/// Formatea una cantidad de segundos como `H:MM:SS` / `M:SS`.
fn format_duration(secs: u32) -> String {
    if secs == 0 || secs == 0xFFFF_FFFF {
        return String::from("--:--");
    }
    let h = secs / 3600;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    if h > 0 {
        alloc::format!("{}:{:02}:{:02}", h, m, s)
    } else {
        alloc::format!("{}:{:02}", m, s)
    }
}

/// Dibuja una barra de progreso Unicode `[████████░░░░░░░░░░]`.
fn build_bar(pct: u8, width: usize) -> String {
    let pct = pct.min(100) as usize;
    let filled = (pct * width) / 100;
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s.push(']');
    s
}

/// Color de icono sugerido para la barra, según el porcentaje.
fn bar_color(pct: u8) -> &'static str {
    if pct >= 60 {
        "SUCCESS"
    } else if pct >= 25 {
        "GOLD"
    } else if pct >= 10 {
        "LOVE"
    } else {
        "ERROR"
    }
}

/// Imprime el reporte completo de batería en la consola (formato Linux).
/// `verbose` añade secciones extendidas (id. de firmware, métodos de lectura, etc.).
pub fn print_report(verbose: bool) {
    use crate::drivers::framebuffer::ui::palette;
    let r = read_report();

    // ── Encabezado ───────────────────────────────────────────────────────
    crate::drivers::framebuffer::set_color(palette::IRIS);
    crate::mesa_println!("╔══════════════════════════════════════════════════════════════╗");
    crate::mesa_println!("║              MESA OS  -  B A T T E R Y   R E P O R T         ║");
    crate::mesa_println!("╚══════════════════════════════════════════════════════════════╝");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!();

    if !r.present {
        crate::drivers::framebuffer::set_color(palette::GOLD);
        crate::mesa_println!("  [WARN] No se ha detectado ninguna batería.");
        crate::drivers::framebuffer::set_color(palette::SUBTLE);
        crate::mesa_println!("         Probablemente estás en un PC de escritorio o una VM.");
        crate::drivers::framebuffer::set_color(palette::TEXT);
        crate::mesa_println!();
        if verbose {
            crate::mesa_println!("  Fuente de datos: {}", r.source);
        }
        return;
    }

    // ── Bloque 1: estado general ─────────────────────────────────────────
    crate::drivers::framebuffer::set_color(palette::FOAM);
    crate::mesa_println!("  Estado de la batería");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("  ──────────────────────────────────────────");

    // Nombre nativo
    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "native-path");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("BAT0");

    // Modelo
    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "model");
    let model = if r.model.is_empty() {
        String::from("(unknown)")
    } else {
        r.model.clone()
    };
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("{}", model);

    // Serial
    if verbose || !r.serial.is_empty() {
        crate::drivers::framebuffer::set_color(palette::SUBTLE);
        crate::mesa_print!("    {:<24}", "serial");
        crate::drivers::framebuffer::set_color(palette::TEXT);
        if r.serial.is_empty() {
            crate::mesa_println!("(none)");
        } else {
            crate::mesa_println!("{}", r.serial);
        }
    }

    // Tecnología
    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "technology");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("{}", r.technology.as_str());

    // ── Bloque 2: estado de carga ────────────────────────────────────────
    crate::mesa_println!();
    crate::drivers::framebuffer::set_color(palette::FOAM);
    crate::mesa_println!("  Carga");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("  ──────────────────────────────────────────");

    // state
    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "state");
    let state_color = match r.state {
        ChargingState::Charging => palette::SUCCESS,
        ChargingState::Discharging => palette::FOAM,
        ChargingState::Full => palette::SUCCESS,
        ChargingState::NotCharging => palette::GOLD,
        _ => palette::SUBTLE,
    };
    crate::drivers::framebuffer::set_color(state_color);
    crate::mesa_println!("{}", r.state.as_str());
    crate::drivers::framebuffer::set_color(palette::TEXT);

    // percentage (con barra de progreso)
    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "percentage");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_print!("{}% ", r.percentage);
    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_println!("{}", build_bar(r.percentage, 30));
    crate::drivers::framebuffer::set_color(palette::TEXT);

    // ── Bloque 3: energía ────────────────────────────────────────────────
    crate::mesa_println!();
    crate::drivers::framebuffer::set_color(palette::FOAM);
    crate::mesa_println!("  Energía");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("  ──────────────────────────────────────────");

    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "energy-full-design");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    if r.design_capacity > 0 {
        crate::mesa_println!("{} {}", r.design_capacity, r.unit_name);
    } else {
        crate::mesa_println!("(unknown)");
    }

    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "energy-full");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    if r.full_charge > 0 {
        crate::mesa_println!("{} {}", r.full_charge, r.unit_name);
    } else {
        crate::mesa_println!("(unknown)");
    }

    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "energy-now");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    if r.current_capacity > 0 {
        crate::mesa_println!("{} {}", r.current_capacity, r.unit_name);
    } else {
        crate::mesa_println!("(unknown)");
    }

    // ── Bloque 4: eléctrico ──────────────────────────────────────────────
    crate::mesa_println!();
    crate::drivers::framebuffer::set_color(palette::FOAM);
    crate::mesa_println!("  Eléctrico");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("  ──────────────────────────────────────────");

    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "voltage");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    if r.voltage_mv > 0 {
        crate::mesa_println!(
            "{}.{:02} V",
            r.voltage_mv / 1000,
            (r.voltage_mv % 1000) / 10
        );
    } else {
        crate::mesa_println!("(unknown)");
    }

    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "energy-rate");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    if r.rate_mw != 0 {
        let abs = r.rate_mw.unsigned_abs();
        if r.rate_mw > 0 {
            crate::mesa_println!("+{}.{:02} W (charging)", abs / 1000, (abs % 1000) / 10);
        } else {
            crate::mesa_println!("-{}.{:02} W (discharging)", abs / 1000, (abs % 1000) / 10);
        }
    } else {
        crate::mesa_println!("0.00 W (idle)");
    }

    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "design-voltage");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    if r.design_voltage_mv > 0 {
        crate::mesa_println!(
            "{}.{:02} V",
            r.design_voltage_mv / 1000,
            (r.design_voltage_mv % 1000) / 10
        );
    } else if r.voltage_mv > 0 {
        crate::mesa_println!(
            "{}.{:02} V (current)",
            r.voltage_mv / 1000,
            (r.voltage_mv % 1000) / 10
        );
    } else {
        crate::mesa_println!("(unknown)");
    }

    // ── Bloque 5: tiempos estimados ──────────────────────────────────────
    crate::mesa_println!();
    crate::drivers::framebuffer::set_color(palette::FOAM);
    crate::mesa_println!("  Tiempo estimado");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("  ──────────────────────────────────────────");

    match r.state {
        ChargingState::Charging => {
            crate::drivers::framebuffer::set_color(palette::SUBTLE);
            crate::mesa_print!("    {:<24}", "time-to-full");
            crate::drivers::framebuffer::set_color(palette::SUCCESS);
            crate::mesa_println!("{}", format_duration(r.time_to_full_sec));
            crate::drivers::framebuffer::set_color(palette::TEXT);
        }
        ChargingState::Discharging => {
            crate::drivers::framebuffer::set_color(palette::SUBTLE);
            crate::mesa_print!("    {:<24}", "time-to-empty");
            let color = if r.percentage <= 15 {
                palette::LOVE
            } else {
                palette::FOAM
            };
            crate::drivers::framebuffer::set_color(color);
            crate::mesa_println!("{}", format_duration(r.time_to_empty_sec));
            crate::drivers::framebuffer::set_color(palette::TEXT);
        }
        ChargingState::Full => {
            crate::drivers::framebuffer::set_color(palette::SUBTLE);
            crate::mesa_print!("    {:<24}", "time-to-full");
            crate::drivers::framebuffer::set_color(palette::SUCCESS);
            crate::mesa_println!("0:00 (fully charged)");
            crate::drivers::framebuffer::set_color(palette::TEXT);
        }
        _ => {
            crate::drivers::framebuffer::set_color(palette::SUBTLE);
            crate::mesa_print!("    {:<24}", "time-to-empty");
            crate::drivers::framebuffer::set_color(palette::TEXT);
            crate::mesa_println!("{}", format_duration(r.time_to_empty_sec));
        }
    }

    // ── Bloque 6: salud y desgaste ────────────────────────────────────────
    crate::mesa_println!();
    crate::drivers::framebuffer::set_color(palette::FOAM);
    crate::mesa_println!("  Salud y desgaste");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("  ──────────────────────────────────────────");

    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "health");
    let health_str = r.health.as_str();
    let health_color = match r.health {
        Health::Good => palette::SUCCESS,
        Health::Worn(p) if p >= 70 => palette::FOAM,
        Health::Worn(p) if p >= 50 => palette::GOLD,
        _ => palette::LOVE,
    };
    crate::drivers::framebuffer::set_color(health_color);
    crate::mesa_println!("{}", health_str);
    crate::drivers::framebuffer::set_color(palette::TEXT);

    if let Some(soh) = r.soh_percent {
        crate::drivers::framebuffer::set_color(palette::SUBTLE);
        crate::mesa_print!("    {:<24}", "capacity (SOH)");
        let color = if soh >= 80 {
            palette::SUCCESS
        } else if soh >= 60 {
            palette::FOAM
        } else if soh >= 40 {
            palette::GOLD
        } else {
            palette::LOVE
        };
        crate::drivers::framebuffer::set_color(color);
        crate::mesa_print!("{}% ", soh);
        crate::drivers::framebuffer::set_color(palette::SUBTLE);
        crate::mesa_println!("{}", build_bar(soh, 30));
        crate::drivers::framebuffer::set_color(palette::TEXT);
    }

    if let Some(cycles) = r.cycle_count {
        crate::drivers::framebuffer::set_color(palette::SUBTLE);
        crate::mesa_print!("    {:<24}", "cycle-count");
        crate::drivers::framebuffer::set_color(palette::TEXT);
        crate::mesa_println!("{}", cycles);
    } else if verbose {
        crate::drivers::framebuffer::set_color(palette::SUBTLE);
        crate::mesa_print!("    {:<24}", "cycle-count");
        crate::drivers::framebuffer::set_color(palette::MUTED);
        crate::mesa_println!("(not exposed by firmware)");
        crate::drivers::framebuffer::set_color(palette::TEXT);
    }

    // ── Bloque 7: térmico (solo si lo tenemos) ───────────────────────────
    if r.temperature_dc != 0 {
        crate::mesa_println!();
        crate::drivers::framebuffer::set_color(palette::FOAM);
        crate::mesa_println!("  Térmico");
        crate::drivers::framebuffer::set_color(palette::TEXT);
        crate::mesa_println!("  ──────────────────────────────────────────");
        crate::drivers::framebuffer::set_color(palette::SUBTLE);
        crate::mesa_print!("    {:<24}", "temperature");
        let temp_c = r.temperature_dc as f32 / 10.0;
        let color = if temp_c < 50.0 {
            palette::SUCCESS
        } else if temp_c < 65.0 {
            palette::GOLD
        } else {
            palette::LOVE
        };
        crate::drivers::framebuffer::set_color(color);
        crate::mesa_println!("{:.1} °C", temp_c);
        crate::drivers::framebuffer::set_color(palette::TEXT);
    }

    // ── Bloque 8: metadatos de fuente (siempre el último) ────────────────
    crate::mesa_println!();
    crate::drivers::framebuffer::set_color(palette::FOAM);
    crate::mesa_println!("  Metadatos");
    crate::drivers::framebuffer::set_color(palette::TEXT);
    crate::mesa_println!("  ──────────────────────────────────────────");

    crate::drivers::framebuffer::set_color(palette::SUBTLE);
    crate::mesa_print!("    {:<24}", "source");
    let src_color = if r.via_aml {
        palette::SUCCESS
    } else {
        palette::FOAM
    };
    crate::drivers::framebuffer::set_color(src_color);
    crate::mesa_println!(
        "{} ({})",
        r.source,
        if r.via_aml {
            "ACPI _BST/_BIF"
        } else if r.source == "EC" {
            "Embedded Controller"
        } else {
            "n/a"
        }
    );
    crate::drivers::framebuffer::set_color(palette::TEXT);

    if verbose {
        if !r.oem_info.is_empty() {
            crate::drivers::framebuffer::set_color(palette::SUBTLE);
            crate::mesa_print!("    {:<24}", "oem-info");
            crate::drivers::framebuffer::set_color(palette::TEXT);
            crate::mesa_println!("{}", r.oem_info);
        }
        if r.design_capacity_warning > 0 {
            crate::drivers::framebuffer::set_color(palette::SUBTLE);
            crate::mesa_print!("    {:<24}", "warning-capacity");
            crate::drivers::framebuffer::set_color(palette::TEXT);
            crate::mesa_println!("{} {}", r.design_capacity_warning, r.unit_name);
        }
        if r.design_capacity_low > 0 {
            crate::drivers::framebuffer::set_color(palette::SUBTLE);
            crate::mesa_print!("    {:<24}", "low-capacity");
            crate::drivers::framebuffer::set_color(palette::TEXT);
            crate::mesa_println!("{} {}", r.design_capacity_low, r.unit_name);
        }
    }

    crate::mesa_println!();
}

/// Variante compacta de `print_report` (una sola línea por métrica clave).
/// Ideal para scripts / logs / status bar.
pub fn print_report_brief() {
    let r = read_report();
    if !r.present {
        return;
    }
    let dt = if r.time_to_empty_sec > 0 && r.state == ChargingState::Discharging {
        format_duration(r.time_to_empty_sec)
    } else if r.time_to_full_sec > 0 && r.state == ChargingState::Charging {
        format_duration(r.time_to_full_sec)
    } else {
        String::from("--:--")
    };
    let rate = if r.rate_mw > 0 {
        alloc::format!("+{}.{:02}W", r.rate_mw / 1000, (r.rate_mw % 1000) / 10)
    } else if r.rate_mw < 0 {
        let a = (-r.rate_mw) as u32;
        alloc::format!("-{}.{:02}W", a / 1000, (a % 1000) / 10)
    } else {
        String::from("0.00W")
    };
    crate::mesa_println!(
        "bat: {}% {} {} {} ({})",
        r.percentage,
        r.state.as_str(),
        rate,
        dt,
        r.source
    );
}
