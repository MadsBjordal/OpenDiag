//! Generic OBD-II / EOBD (SAE J1979, ISO 15031-5) on top of any driver that can
//! send a request and return each ECU's answer.
//!
//! Everything here is pure: bytes in, typed values out. The formulas are the
//! public ones from SAE J1979 for the standard PIDs.

pub mod dtc;
pub mod mode06;
pub mod pid;
pub mod readiness;
pub mod response;
pub mod vin;

use serde::{Deserialize, Serialize};

/// One answer from one ECU, already joined (multi-frame reassembled).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcuMessage {
    /// Who answered: CAN id (e.g. "7E8", "18DAF110") or legacy source address ("10").
    /// "?" when the adapter could not show headers.
    pub ecu: String,
    pub data: Vec<u8>,
}

/// Service (mode) numbers used in Phase 1.
pub mod service {
    pub const CURRENT_DATA: u8 = 0x01;
    pub const FREEZE_FRAME: u8 = 0x02;
    pub const STORED_DTCS: u8 = 0x03;
    pub const CLEAR_DTCS: u8 = 0x04;
    pub const MONITOR_RESULTS: u8 = 0x06;
    pub const PENDING_DTCS: u8 = 0x07;
    pub const VEHICLE_INFO: u8 = 0x09;
    pub const PERMANENT_DTCS: u8 = 0x0A;
}

/// Best-effort name for a responding ECU, from the ISO 15765-4 address conventions.
/// This is only a convention, not an identification.
pub fn ecu_label(ecu: &str) -> String {
    let upper = ecu.to_ascii_uppercase();
    let n = match upper.as_str() {
        "7E8" | "18DAF110" => Some(1),
        "7E9" | "18DAF118" => Some(2),
        "7EA" | "18DAF119" => Some(3),
        "7EB" => Some(4),
        _ => None,
    };
    match n {
        Some(1) => "ECU #1 (usually engine)".into(),
        Some(2) => "ECU #2 (usually transmission)".into(),
        Some(n) => format!("ECU #{n}"),
        None if upper == "?" => "ECU (address not shown by adapter)".into(),
        None => format!("ECU {upper}"),
    }
}
