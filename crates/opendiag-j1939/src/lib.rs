//! SAE J1939: the CAN network of trucks, buses and many machines (250 kbit/s,
//! 29-bit identifiers). Read only.
//!
//! Sources:
//! - ELM327 datasheet (ELM327DSL), "SAE J1939 Messages" and "Using J1939":
//!   protocol A, `AT MP` monitoring, PGN requests, the printed header formats
//!   (`6 0FEEE 00 …` with `AT JHF1`, `18 FE EE 00 …` with `AT JHF0`) and
//!   multi-segment answers (a byte-count line, then one line per segment).
//! - SAE J1939-21 (identifier layout, PDU1/PDU2, transport protocol),
//!   SAE J1939-71 (parameter groups and SPN scaling) and SAE J1939-73 (DM1/DM2
//!   layout, FMI meanings, lamp bits). The parameters used here are the ones
//!   the open FMS Standard (fms-standard.com) also publishes.
//!
//! Nothing here sends anything; see `opendiag-safety` for what may be requested.

mod params;
mod parse;

pub use params::{decode, spn_name, ParamDef, ParamValue, PARAMS};
pub use parse::{parse_elm, Nack, Parsed};

use serde::{Deserialize, Serialize};

/// DM1: active diagnostic trouble codes (broadcast about once a second).
pub const PGN_DM1: u32 = 0xFECA;
/// DM2: previously active diagnostic trouble codes (on request).
pub const PGN_DM2: u32 = 0xFECB;
/// Vehicle identification (VIN, ASCII, ends with `*`).
pub const PGN_VI: u32 = 0xFEEC;
/// Acknowledgement (positive / negative answer to a request).
pub const PGN_ACK: u32 = 0xE800;

/// One complete J1939 message (single frame, or reassembled multi-segment).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct J1939Message {
    pub priority: u8,
    pub pgn: u32,
    pub source: u8,
    /// Destination for PDU1 messages (`None` = broadcast PDU2).
    pub dest: Option<u8>,
    pub data: Vec<u8>,
}

/// Names of a few addresses from the J1939 preferred-address list.
pub fn address_name(sa: u8) -> String {
    match sa {
        0x00 => "Engine #1".into(),
        0x01 => "Engine #2".into(),
        0x03 => "Transmission #1".into(),
        0x0B => "Brakes (system controller)".into(),
        0x17 => "Instrument cluster #1".into(),
        0x21 => "Body controller".into(),
        0xF9 => "Diagnostic tool #1".into(),
        _ => format!("Control unit at address {sa} (0x{sa:02X})"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LampState {
    Off,
    On,
    Error,
    NotAvailable,
}

impl LampState {
    fn from_bits(b: u8) -> Self {
        match b & 0b11 {
            0 => Self::Off,
            1 => Self::On,
            2 => Self::Error,
            _ => Self::NotAvailable,
        }
    }
}

/// The warning lamps reported in DM1/DM2 byte 1 (J1939-73).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lamps {
    /// Malfunction indicator (emissions).
    pub mil: LampState,
    pub red_stop: LampState,
    pub amber_warning: LampState,
    pub protect: LampState,
}

/// One J1939 trouble code: suspect parameter (SPN) + failure mode (FMI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct J1939Dtc {
    pub spn: u32,
    pub fmi: u8,
    /// Occurrence count (127 = not available).
    pub occurrences: u8,
    /// Conversion method bit set: the ECU uses one of the older SPN layouts,
    /// so the SPN shown may be wrong.
    pub old_conversion: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticMessage {
    pub lamps: Lamps,
    pub dtcs: Vec<J1939Dtc>,
}

/// Decodes a DM1 or DM2 message: 2 lamp bytes, then 4 bytes per code.
pub fn decode_dm(data: &[u8]) -> Option<DiagnosticMessage> {
    if data.len() < 2 {
        return None;
    }
    let l = data[0];
    let lamps = Lamps {
        protect: LampState::from_bits(l),
        amber_warning: LampState::from_bits(l >> 2),
        red_stop: LampState::from_bits(l >> 4),
        mil: LampState::from_bits(l >> 6),
    };
    let mut dtcs = Vec::new();
    for c in data[2..].chunks(4) {
        if c.len() < 4 || c.iter().all(|b| *b == 0xFF) || c[..3].iter().all(|b| *b == 0) {
            // Padding, or the "no codes" pattern 00 00 00 00.
            continue;
        }
        let spn = c[0] as u32 | (c[1] as u32) << 8 | ((c[2] as u32) >> 5) << 16;
        dtcs.push(J1939Dtc {
            spn,
            fmi: c[2] & 0x1F,
            occurrences: c[3] & 0x7F,
            old_conversion: c[3] & 0x80 != 0,
        });
    }
    Some(DiagnosticMessage { lamps, dtcs })
}

/// Plain-language meaning of a failure mode identifier (J1939-73).
pub fn fmi_text(fmi: u8) -> &'static str {
    match fmi {
        0 => "Value above the normal range (most severe)",
        1 => "Value below the normal range (most severe)",
        2 => "Value erratic, intermittent or incorrect",
        3 => "Voltage above normal, or shorted to high",
        4 => "Voltage below normal, or shorted to low",
        5 => "Current below normal, or open circuit",
        6 => "Current above normal, or grounded circuit",
        7 => "Mechanical system not responding or out of adjustment",
        8 => "Abnormal frequency, pulse width or period",
        9 => "Abnormal update rate",
        10 => "Abnormal rate of change",
        11 => "Root cause not known",
        12 => "Bad intelligent device or component",
        13 => "Out of calibration",
        14 => "Special instructions (see the manufacturer's manual)",
        15 => "Value above the normal range (least severe)",
        16 => "Value above the normal range (moderately severe)",
        17 => "Value below the normal range (least severe)",
        18 => "Value below the normal range (moderately severe)",
        19 => "Received network data in error",
        20 => "Data drifted high",
        21 => "Data drifted low",
        31 => "Condition exists",
        _ => "Reserved failure mode",
    }
}

/// The VIN from a vehicle identification message (ASCII up to `*`).
pub fn decode_vin(data: &[u8]) -> Option<String> {
    let end = data.iter().position(|b| *b == b'*').unwrap_or(data.len());
    let v: String = data[..end]
        .iter()
        .take_while(|b| **b != 0xFF && **b != 0)
        .map(|b| *b as char)
        .collect();
    let v = v.trim().to_string();
    if v.is_empty() || !v.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(v)
}

#[cfg(test)]
mod tests;
