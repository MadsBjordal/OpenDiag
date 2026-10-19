//! Service 01 / 02 parameter IDs (PIDs) with the SAE J1979 scaling formulas.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Engine,
    Fuel,
    Air,
    Exhaust,
    Vehicle,
    Electrical,
    Status,
}

pub struct PidDef {
    pub pid: u8,
    /// Stable id used in the UI, dashboards and CSV files.
    pub key: &'static str,
    pub name: &'static str,
    pub unit: &'static str,
    pub bytes: usize,
    pub min: f64,
    pub max: f64,
    pub category: Category,
    /// Shown on the default live data dashboard.
    pub common: bool,
    decode: fn(&[u8]) -> f64,
}

fn a(d: &[u8]) -> f64 {
    d[0] as f64
}
fn ab(d: &[u8]) -> f64 {
    (d[0] as f64) * 256.0 + d[1] as f64
}
fn pct(d: &[u8]) -> f64 {
    a(d) * 100.0 / 255.0
}
fn temp(d: &[u8]) -> f64 {
    a(d) - 40.0
}
fn trim(d: &[u8]) -> f64 {
    (a(d) - 128.0) * 100.0 / 128.0
}
fn o2_volts(d: &[u8]) -> f64 {
    a(d) / 200.0
}

macro_rules! pid {
    ($pid:expr, $key:expr, $name:expr, $unit:expr, $bytes:expr, $min:expr, $max:expr, $cat:ident, $common:expr, $f:expr) => {
        PidDef {
            pid: $pid,
            key: $key,
            name: $name,
            unit: $unit,
            bytes: $bytes,
            min: $min,
            max: $max,
            category: Category::$cat,
            common: $common,
            decode: $f,
        }
    };
}

/// The numeric standard PIDs OpenDiag decodes. Others are shown as raw bytes.
pub static PIDS: &[PidDef] = &[
    pid!(
        0x04,
        "engine_load",
        "Calculated engine load",
        "%",
        1,
        0.0,
        100.0,
        Engine,
        true,
        pct
    ),
    pid!(
        0x05,
        "coolant_temp",
        "Engine coolant temperature",
        "°C",
        1,
        -40.0,
        215.0,
        Engine,
        true,
        temp
    ),
    pid!(
        0x06,
        "stft_b1",
        "Short term fuel trim, bank 1",
        "%",
        1,
        -100.0,
        99.2,
        Fuel,
        true,
        trim
    ),
    pid!(
        0x07,
        "ltft_b1",
        "Long term fuel trim, bank 1",
        "%",
        1,
        -100.0,
        99.2,
        Fuel,
        true,
        trim
    ),
    pid!(
        0x08,
        "stft_b2",
        "Short term fuel trim, bank 2",
        "%",
        1,
        -100.0,
        99.2,
        Fuel,
        false,
        trim
    ),
    pid!(
        0x09,
        "ltft_b2",
        "Long term fuel trim, bank 2",
        "%",
        1,
        -100.0,
        99.2,
        Fuel,
        false,
        trim
    ),
    pid!(
        0x0A,
        "fuel_pressure",
        "Fuel pressure (gauge)",
        "kPa",
        1,
        0.0,
        765.0,
        Fuel,
        false,
        |d| a(d) * 3.0
    ),
    pid!(
        0x0B,
        "map",
        "Intake manifold absolute pressure",
        "kPa",
        1,
        0.0,
        255.0,
        Air,
        true,
        a
    ),
    pid!(
        0x0C,
        "rpm",
        "Engine speed",
        "rpm",
        2,
        0.0,
        16383.75,
        Engine,
        true,
        |d| ab(d) / 4.0
    ),
    pid!(
        0x0D,
        "speed",
        "Vehicle speed",
        "km/h",
        1,
        0.0,
        255.0,
        Vehicle,
        true,
        a
    ),
    pid!(
        0x0E,
        "timing_advance",
        "Timing advance (cyl. 1)",
        "°",
        1,
        -64.0,
        63.5,
        Engine,
        false,
        |d| a(d) / 2.0 - 64.0
    ),
    pid!(
        0x0F,
        "iat",
        "Intake air temperature",
        "°C",
        1,
        -40.0,
        215.0,
        Air,
        true,
        temp
    ),
    pid!(
        0x10,
        "maf",
        "Mass air flow rate",
        "g/s",
        2,
        0.0,
        655.35,
        Air,
        true,
        |d| ab(d) / 100.0
    ),
    pid!(
        0x11,
        "throttle",
        "Throttle position",
        "%",
        1,
        0.0,
        100.0,
        Engine,
        true,
        pct
    ),
    pid!(
        0x14,
        "o2_b1s1",
        "Oxygen sensor voltage, bank 1 sensor 1",
        "V",
        2,
        0.0,
        1.275,
        Exhaust,
        true,
        o2_volts
    ),
    pid!(
        0x15,
        "o2_b1s2",
        "Oxygen sensor voltage, bank 1 sensor 2",
        "V",
        2,
        0.0,
        1.275,
        Exhaust,
        false,
        o2_volts
    ),
    pid!(
        0x16,
        "o2_b1s3",
        "Oxygen sensor voltage, bank 1 sensor 3",
        "V",
        2,
        0.0,
        1.275,
        Exhaust,
        false,
        o2_volts
    ),
    pid!(
        0x17,
        "o2_b1s4",
        "Oxygen sensor voltage, bank 1 sensor 4",
        "V",
        2,
        0.0,
        1.275,
        Exhaust,
        false,
        o2_volts
    ),
    pid!(
        0x18,
        "o2_b2s1",
        "Oxygen sensor voltage, bank 2 sensor 1",
        "V",
        2,
        0.0,
        1.275,
        Exhaust,
        false,
        o2_volts
    ),
    pid!(
        0x19,
        "o2_b2s2",
        "Oxygen sensor voltage, bank 2 sensor 2",
        "V",
        2,
        0.0,
        1.275,
        Exhaust,
        false,
        o2_volts
    ),
    pid!(
        0x1F,
        "run_time",
        "Run time since engine start",
        "s",
        2,
        0.0,
        65535.0,
        Engine,
        false,
        ab
    ),
    pid!(
        0x21,
        "distance_mil",
        "Distance travelled with MIL on",
        "km",
        2,
        0.0,
        65535.0,
        Status,
        false,
        ab
    ),
    pid!(
        0x23,
        "fuel_rail_pressure",
        "Fuel rail pressure (diesel / direct injection)",
        "kPa",
        2,
        0.0,
        655350.0,
        Fuel,
        false,
        |d| ab(d) * 10.0
    ),
    pid!(
        0x2F,
        "fuel_level",
        "Fuel tank level",
        "%",
        1,
        0.0,
        100.0,
        Fuel,
        false,
        pct
    ),
    pid!(
        0x31,
        "distance_since_clear",
        "Distance since codes cleared",
        "km",
        2,
        0.0,
        65535.0,
        Status,
        false,
        ab
    ),
    pid!(
        0x33,
        "baro",
        "Barometric pressure",
        "kPa",
        1,
        0.0,
        255.0,
        Air,
        false,
        a
    ),
    pid!(
        0x42,
        "module_voltage",
        "Control module voltage",
        "V",
        2,
        0.0,
        65.535,
        Electrical,
        true,
        |d| ab(d) / 1000.0
    ),
    pid!(
        0x43,
        "absolute_load",
        "Absolute load value",
        "%",
        2,
        0.0,
        25700.0,
        Engine,
        false,
        |d| ab(d) * 100.0 / 255.0
    ),
    pid!(
        0x44,
        "lambda_commanded",
        "Commanded air-fuel equivalence ratio",
        "λ",
        2,
        0.0,
        2.0,
        Fuel,
        false,
        |d| ab(d) * 2.0 / 65536.0
    ),
    pid!(
        0x45,
        "relative_throttle",
        "Relative throttle position",
        "%",
        1,
        0.0,
        100.0,
        Engine,
        false,
        pct
    ),
    pid!(
        0x46,
        "ambient_temp",
        "Ambient air temperature",
        "°C",
        1,
        -40.0,
        215.0,
        Air,
        false,
        temp
    ),
    pid!(
        0x49,
        "pedal_d",
        "Accelerator pedal position D",
        "%",
        1,
        0.0,
        100.0,
        Engine,
        false,
        pct
    ),
    pid!(
        0x4A,
        "pedal_e",
        "Accelerator pedal position E",
        "%",
        1,
        0.0,
        100.0,
        Engine,
        false,
        pct
    ),
    pid!(
        0x4C,
        "throttle_commanded",
        "Commanded throttle actuator",
        "%",
        1,
        0.0,
        100.0,
        Engine,
        false,
        pct
    ),
    pid!(
        0x5C,
        "oil_temp",
        "Engine oil temperature",
        "°C",
        1,
        -40.0,
        210.0,
        Engine,
        false,
        temp
    ),
    pid!(
        0x5E,
        "fuel_rate",
        "Engine fuel rate",
        "L/h",
        2,
        0.0,
        3276.75,
        Fuel,
        false,
        |d| ab(d) / 20.0
    ),
];

pub fn def(pid: u8) -> Option<&'static PidDef> {
    PIDS.iter().find(|p| p.pid == pid)
}

pub fn def_by_key(key: &str) -> Option<&'static PidDef> {
    PIDS.iter().find(|p| p.key == key)
}

impl PidDef {
    /// Decodes the data bytes after `41 <pid>`. `None` if too short.
    pub fn decode(&self, data: &[u8]) -> Option<f64> {
        // Oxygen sensor PIDs carry 2 bytes, but only byte A is needed for the voltage.
        let needed = if (0x14..=0x1B).contains(&self.pid) {
            1
        } else {
            self.bytes
        };
        if data.len() < needed {
            return None;
        }
        Some((self.decode)(data))
    }
}

/// A decoded measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PidValue {
    pub pid: u8,
    pub key: String,
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub ecu: String,
}

/// Decodes the PIDs listed in the 0x00 / 0x20 / 0x40 … "supported PIDs" bitmaps.
/// `base` is the request PID (0x00, 0x20, …); bit 7 of the first byte is `base + 1`.
pub fn supported_from_bitmap(base: u8, bitmap: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, byte) in bitmap.iter().take(4).enumerate() {
        for bit in 0..8 {
            if byte & (0x80 >> bit) != 0 {
                out.push(base + (i * 8 + bit) as u8 + 1);
            }
        }
    }
    out
}

/// Builds a bitmap (the inverse of [`supported_from_bitmap`]); used by the simulator.
pub fn bitmap_for(base: u8, supported: &[u8]) -> [u8; 4] {
    let mut out = [0u8; 4];
    for &p in supported {
        if p > base && (p as u16) <= base as u16 + 32 {
            let idx = (p - base - 1) as usize;
            out[idx / 8] |= 0x80 >> (idx % 8);
        }
    }
    out
}

/// PID 0x1C: which OBD standard the vehicle says it follows.
pub fn obd_standard(code: u8) -> &'static str {
    match code {
        1 => "OBD-II as defined by CARB",
        2 => "OBD as defined by the EPA",
        3 => "OBD and OBD-II",
        4 => "OBD-I",
        5 => "Not OBD compliant",
        6 => "EOBD (Europe)",
        7 => "EOBD and OBD-II",
        8 => "EOBD and OBD",
        9 => "EOBD, OBD and OBD-II",
        10 => "JOBD (Japan)",
        11 => "JOBD and OBD-II",
        12 => "JOBD and EOBD",
        13 => "JOBD, EOBD and OBD-II",
        _ => "Other / not listed",
    }
}

/// PID 0x51: fuel type.
pub fn fuel_type(code: u8) -> &'static str {
    match code {
        0 => "Not available",
        1 => "Gasoline",
        2 => "Methanol",
        3 => "Ethanol",
        4 => "Diesel",
        5 => "LPG",
        6 => "CNG",
        7 => "Propane",
        8 => "Electric",
        9 => "Bifuel running gasoline",
        10 => "Bifuel running methanol",
        11 => "Bifuel running ethanol",
        12 => "Bifuel running LPG",
        13 => "Bifuel running CNG",
        14 => "Bifuel running propane",
        15 => "Bifuel running electricity",
        16 => "Bifuel running electric and combustion engine",
        17 => "Hybrid gasoline",
        18 => "Hybrid ethanol",
        19 => "Hybrid diesel",
        20 => "Hybrid electric",
        21 => "Hybrid running electric and combustion engine",
        22 => "Hybrid regenerative",
        23 => "Bifuel running diesel",
        _ => "Other / not listed",
    }
}

/// PID 0x03 (byte A): fuel system status.
pub fn fuel_system_status(a: u8) -> &'static str {
    match a {
        0 => "Not reported",
        1 => "Open loop: engine not warm yet",
        2 => "Closed loop: using oxygen sensor feedback",
        4 => "Open loop: engine load or deceleration",
        8 => "Open loop: system fault",
        16 => "Closed loop, with a fault in at least one oxygen sensor",
        _ => "Unrecognised value",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(pid: u8, data: &[u8]) -> f64 {
        def(pid).unwrap().decode(data).unwrap()
    }

    #[test]
    fn standard_formulas() {
        assert_eq!(dec(0x0C, &[0x1A, 0xF8]), 1726.0);
        assert_eq!(dec(0x05, &[0x7B]), 83.0);
        assert_eq!(dec(0x0D, &[0x32]), 50.0);
        assert!((dec(0x11, &[0xFF]) - 100.0).abs() < 1e-9);
        assert_eq!(dec(0x06, &[0x80]), 0.0);
        assert_eq!(dec(0x06, &[0x00]), -100.0);
        assert_eq!(dec(0x10, &[0x01, 0x90]), 4.0);
        assert_eq!(dec(0x42, &[0x36, 0xB0]), 14.0);
        assert_eq!(dec(0x14, &[0x5A, 0xFF]), 0.45);
        assert_eq!(dec(0x0E, &[0x80]), 0.0);
        assert_eq!(dec(0x5E, &[0x00, 0x64]), 5.0);
    }

    #[test]
    fn too_short_is_none() {
        assert_eq!(def(0x0C).unwrap().decode(&[0x1A]), None);
        assert_eq!(def(0x05).unwrap().decode(&[]), None);
    }

    #[test]
    fn bitmap_round_trip() {
        // A real-world style bitmap: BE 3F A8 13.
        let s = supported_from_bitmap(0x00, &[0xBE, 0x3F, 0xA8, 0x13]);
        assert_eq!(
            s,
            vec![
                0x01, 0x03, 0x04, 0x05, 0x06, 0x07, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x13,
                0x15, 0x1C, 0x1F, 0x20
            ]
        );
        assert_eq!(bitmap_for(0x00, &s), [0xBE, 0x3F, 0xA8, 0x13]);
        assert_eq!(
            supported_from_bitmap(0x20, &[0x80, 0, 0, 0x01]),
            vec![0x21, 0x40]
        );
    }

    #[test]
    fn keys_are_unique() {
        let mut keys: Vec<_> = PIDS.iter().map(|p| p.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), PIDS.len());
    }
}
