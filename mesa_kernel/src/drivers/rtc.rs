// mesa_kernel/src/drivers/rtc.rs

use x86_64::instructions::port::Port;
use spin::Mutex;
use alloc::format;

const CMOS_ADDRESS: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

const RTC_SECONDS: u8 = 0x00;
const RTC_MINUTES: u8 = 0x02;
const RTC_HOURS: u8 = 0x04;
const RTC_DAY: u8 = 0x07;
const RTC_MONTH: u8 = 0x08;
const RTC_YEAR: u8 = 0x09;
const RTC_STATUS_A: u8 = 0x0A;
const RTC_STATUS_B: u8 = 0x0B;

// ══════════════════════════════════════════════════════════════════════════════
// TIMEZONE CONFIGURATION - Europa Central (España, Francia, Alemania, etc.)
// ══════════════════════════════════════════════════════════════════════════════
// CET  (invierno): UTC+1
// CEST (verano):   UTC+2
//
// Cambio horario EU:

#[derive(Debug, Clone, Copy)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl DateTime {
    pub fn format(&self) -> alloc::string::String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day,
            self.hour, self.minute, self.second
        )
    }
    
    pub fn format_date(&self) -> alloc::string::String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    
    pub fn format_time(&self) -> alloc::string::String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
    
    pub fn format_time_short(&self) -> alloc::string::String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

static RTC_LOCK: Mutex<()> = Mutex::new(());

fn read_cmos(reg: u8) -> u8 {
    unsafe {
        let mut addr: Port<u8> = Port::new(CMOS_ADDRESS);
        let mut data: Port<u8> = Port::new(CMOS_DATA);
        addr.write(reg);
        data.read()
    }
}

fn is_updating() -> bool {
    read_cmos(RTC_STATUS_A) & 0x80 != 0
}

fn bcd_to_binary(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}

// ══════════════════════════════════════════════════════════════════════════════
// CÁLCULO DEL DÍA DE LA SEMANA (Algoritmo de Zeller simplificado)
// ══════════════════════════════════════════════════════════════════════════════
// Devuelve: 0=Domingo, 1=Lunes, ..., 6=Sábado

fn day_of_week(year: u16, month: u8, day: u8) -> u8 {
    let y = year as i32;
    let m = month as i32;
    let d = day as i32;
    
    // Ajustar enero y febrero como meses 13 y 14 del año anterior
    let (y, m) = if m < 3 {
        (y - 1, m + 12)
    } else {
        (y, m)
    };
    
    let q = d;
    let k = y % 100;
    let j = y / 100;
    
    let h = (q + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 - 2 * j) % 7;
    
    // Convertir: Zeller da 0=Sábado, queremos 0=Domingo
    let dow = ((h + 6) % 7) as u8;
    dow
}

// ══════════════════════════════════════════════════════════════════════════════
// CÁLCULO DEL ÚLTIMO DOMINGO DEL MES
// ══════════════════════════════════════════════════════════════════════════════

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            // Año bisiesto
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn last_sunday_of_month(year: u16, month: u8) -> u8 {
    let last_day = days_in_month(year, month);
    let dow = day_of_week(year, month, last_day);
    
    // dow: 0=Domingo, 1=Lunes, ...
    // Si el último día es domingo (0), ese es el último domingo
    // Si no, retroceder (dow) días
    last_day - dow
}

// ══════════════════════════════════════════════════════════════════════════════
// DETECTAR SI ESTAMOS EN HORARIO DE VERANO (DST)
// ══════════════════════════════════════════════════════════════════════════════
// Horario de verano en EU:
// - Empieza: último domingo de marzo a las 02:00 UTC
// - Termina: último domingo de octubre a las 03:00 UTC (02:00 en horario de verano)

fn is_dst(year: u16, month: u8, day: u8, hour: u8) -> bool {
    // Último domingo de marzo
    let march_switch = last_sunday_of_month(year, 3);
    // Último domingo de octubre
    let october_switch = last_sunday_of_month(year, 10);
    
    match month {
        // Enero, Febrero: invierno
        1 | 2 => false,
        
        // Marzo: depende del día
        3 => {
            if day < march_switch {
                false
            } else if day > march_switch {
                true
            } else {
                // Es el día del cambio: verano a partir de las 02:00 UTC
                hour >= 2
            }
        }
        
        // Abril - Septiembre: verano
        4 | 5 | 6 | 7 | 8 | 9 => true,
        
        // Octubre: depende del día
        10 => {
            if day < october_switch {
                true
            } else if day > october_switch {
                false
            } else {
                // Es el día del cambio: invierno a partir de las 03:00 UTC
                // (que en verano serían las 03:00 CEST = 01:00 UTC)
                // Simplificado: antes de las 3 (hora local verano) es verano
                hour < 3
            }
        }
        
        // Noviembre, Diciembre: invierno
        11 | 12 => false,
        
        _ => false,
    }
}

/// Calcula el offset de timezone actual (1 o 2)
fn get_timezone_offset(year: u16, month: u8, day: u8, hour: u8) -> i8 {
    let base = crate::config::get_tz_offset();
    if is_dst(year, month, day, hour) {
        base + 1  // Verano (+1 sobre base)
    } else {
        base      // Invierno (base)
    }
}

/// Aplica el offset de timezone a una fecha/hora UTC
fn apply_timezone(dt: DateTime) -> DateTime {
    let offset = get_timezone_offset(dt.year, dt.month, dt.day, dt.hour);
    
    let mut hour = dt.hour as i8 + offset;
    let mut day = dt.day;
    let mut month = dt.month;
    let mut year = dt.year;
    
    if hour >= 24 {
        hour -= 24;
        day += 1;
        
        let days = days_in_month(year, month);
        if day > days {
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
    } else if hour < 0 {
        hour += 24;
        if day > 1 {
            day -= 1;
        } else {
            month = if month > 1 { month - 1 } else { 12 };
            if month == 12 {
                year -= 1;
            }
            day = days_in_month(year, month);
        }
    }
    
    DateTime {
        year,
        month,
        day,
        hour: hour as u8,
        minute: dt.minute,
        second: dt.second,
    }
}

pub fn init() {
    // Leer fecha actual para mostrar timezone
    let utc = read_utc();
    let offset = get_timezone_offset(utc.year, utc.month, utc.day, utc.hour);
    let tz_name = if offset == 2 { "CEST (verano)" } else { "CET (invierno)" };
    crate::serial_println!("[RTC] Driver inicializado - Timezone: UTC+{} {}", offset, tz_name);
}

/// Lee la fecha y hora UTC del RTC (sin timezone)
pub fn read_utc() -> DateTime {
    let _lock = RTC_LOCK.lock();
    
    for _ in 0..1000 {
        if !is_updating() {
            break;
        }
    }
    
    let mut second = read_cmos(RTC_SECONDS);
    let mut minute = read_cmos(RTC_MINUTES);
    let mut hour = read_cmos(RTC_HOURS);
    let mut day = read_cmos(RTC_DAY);
    let mut month = read_cmos(RTC_MONTH);
    let mut year = read_cmos(RTC_YEAR) as u16;
    
    let status_b = read_cmos(RTC_STATUS_B);
    
    if status_b & 0x04 == 0 {
        second = bcd_to_binary(second);
        minute = bcd_to_binary(minute);
        hour = bcd_to_binary(hour & 0x7F) | (hour & 0x80);
        day = bcd_to_binary(day);
        month = bcd_to_binary(month);
        year = bcd_to_binary(year as u8) as u16;
    }
    
    if status_b & 0x02 == 0 && hour & 0x80 != 0 {
        hour = ((hour & 0x7F) + 12) % 24;
    }
    
    year += 2000;
    
    DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    }
}

/// Lee la fecha y hora actual con timezone automático (CET/CEST)
pub fn read() -> DateTime {
    let utc = read_utc();
    apply_timezone(utc)
}

pub fn get_datetime() -> alloc::string::String {
    read().format()
}

pub fn get_time() -> alloc::string::String {
    read().format_time()
}

/// Devuelve true si actualmente es horario de verano
pub fn is_summer_time() -> bool {
    let utc = read_utc();
    is_dst(utc.year, utc.month, utc.day, utc.hour)
}

/// Devuelve el offset actual (1=CET, 2=CEST)
pub fn current_timezone_offset() -> i8 {
    let utc = read_utc();
    get_timezone_offset(utc.year, utc.month, utc.day, utc.hour)
}

/// Devuelve el nombre del timezone actual
pub fn timezone_name() -> &'static str {
    if is_summer_time() {
        "CEST"
    } else {
        "CET"
    }
}