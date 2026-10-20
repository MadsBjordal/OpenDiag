//! A small set of J1939-71 parameters (SPNs) with their documented scaling.
//! Only parameters also published in the open FMS Standard are included.

use crate::J1939Message;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct ParamDef {
    pub key: &'static str,
    pub name: &'static str,
    pub spn: u32,
    pub pgn: u32,
    /// First byte (0-based) and length in bytes (little endian).
    pub start: usize,
    pub len: usize,
    pub scale: f64,
    pub offset: f64,
    pub unit: &'static str,
    /// Sent all the time by the ECU (listened for), else requested.
    pub broadcast: bool,
}

#[allow(clippy::too_many_arguments)]
const fn p(
    key: &'static str,
    name: &'static str,
    spn: u32,
    pgn: u32,
    start: usize,
    len: usize,
    scale: f64,
    offset: f64,
    unit: &'static str,
    broadcast: bool,
) -> ParamDef {
    ParamDef {
        key,
        name,
        spn,
        pgn,
        start,
        len,
        scale,
        offset,
        unit,
        broadcast,
    }
}

pub const PARAMS: &[ParamDef] = &[
    // EEC1, PGN 61444
    p(
        "engine_speed",
        "Engine speed",
        190,
        0xF004,
        3,
        2,
        0.125,
        0.0,
        "rpm",
        true,
    ),
    // ET1, PGN 65262
    p(
        "coolant_temp",
        "Engine coolant temperature",
        110,
        0xFEEE,
        0,
        1,
        1.0,
        -40.0,
        "°C",
        true,
    ),
    // EFL/P1, PGN 65263
    p(
        "oil_pressure",
        "Engine oil pressure",
        100,
        0xFEEF,
        3,
        1,
        4.0,
        0.0,
        "kPa",
        true,
    ),
    // CCVS, PGN 65265
    p(
        "vehicle_speed",
        "Wheel-based vehicle speed",
        84,
        0xFEF1,
        1,
        2,
        1.0 / 256.0,
        0.0,
        "km/h",
        true,
    ),
    // LFE, PGN 65266
    p(
        "fuel_rate",
        "Engine fuel rate",
        183,
        0xFEF2,
        0,
        2,
        0.05,
        0.0,
        "L/h",
        true,
    ),
    // AMB, PGN 65269
    p(
        "ambient_temp",
        "Ambient air temperature",
        171,
        0xFEF5,
        3,
        2,
        0.03125,
        -273.0,
        "°C",
        true,
    ),
    // VEP1, PGN 65271
    p(
        "battery_voltage",
        "Battery potential",
        168,
        0xFEF7,
        4,
        2,
        0.05,
        0.0,
        "V",
        true,
    ),
    // DD, PGN 65276
    p(
        "fuel_level",
        "Fuel level 1",
        96,
        0xFEFC,
        1,
        1,
        0.4,
        0.0,
        "%",
        true,
    ),
    // HOURS, PGN 65253
    p(
        "engine_hours",
        "Total engine hours",
        247,
        0xFEE5,
        0,
        4,
        0.05,
        0.0,
        "h",
        false,
    ),
    // VD, PGN 65248
    p(
        "total_distance",
        "Total vehicle distance",
        245,
        0xFEE0,
        4,
        4,
        0.125,
        0.0,
        "km",
        false,
    ),
];

/// Name of an SPN if it is one of the parameters above.
pub fn spn_name(spn: u32) -> Option<&'static str> {
    PARAMS.iter().find(|d| d.spn == spn).map(|d| d.name)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamValue {
    pub key: String,
    pub name: String,
    pub spn: u32,
    pub pgn: u32,
    pub source: u8,
    pub value: Option<f64>,
    pub unit: String,
    /// Why there is no value: "not available" or "error" (sent by the ECU).
    pub status: Option<String>,
}

/// Decodes the known parameters in one message.
pub fn decode(msg: &J1939Message) -> Vec<ParamValue> {
    PARAMS
        .iter()
        .filter(|d| d.pgn == msg.pgn)
        .filter_map(|d| {
            let raw = msg.data.get(d.start..d.start + d.len)?;
            let v = raw.iter().rev().fold(0u64, |a, b| a << 8 | *b as u64);
            // J1939-71: the top of each range is reserved for "error" (xE…)
            // and "not available" (xF…); everything from xFB… on is not a value.
            let max_valid = match d.len {
                1 => 0xFA,
                2 => 0xFAFF,
                _ => 0xFAFF_FFFF,
            };
            let top = v >> (8 * (d.len - 1));
            let (value, status) = if v <= max_valid {
                (Some(v as f64 * d.scale + d.offset), None)
            } else if top == 0xFE {
                (None, Some("error".to_string()))
            } else {
                (None, Some("not available".to_string()))
            };
            Some(ParamValue {
                key: d.key.into(),
                name: d.name.into(),
                spn: d.spn,
                pgn: d.pgn,
                source: msg.source,
                value,
                unit: d.unit.into(),
                status,
            })
        })
        .collect()
}
