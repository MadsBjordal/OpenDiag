//! PID 0x01: MIL status, number of stored codes and readiness monitors.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Monitor {
    pub name: String,
    /// The vehicle has this monitor.
    pub available: bool,
    /// The monitor has finished its self-test since codes were last cleared.
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorStatus {
    /// Malfunction indicator lamp ("check engine light") commanded on.
    pub mil_on: bool,
    /// Number of emissions-related stored codes the ECU reports.
    pub dtc_count: u8,
    pub compression_ignition: bool,
    pub monitors: Vec<Monitor>,
}

const SPARK: [&str; 8] = [
    "Catalyst",
    "Heated catalyst",
    "Evaporative system",
    "Secondary air system",
    "A/C refrigerant",
    "Oxygen sensor",
    "Oxygen sensor heater",
    "EGR system",
];

const COMPRESSION: [&str; 8] = [
    "NMHC catalyst",
    "NOx / SCR aftertreatment",
    "",
    "Boost pressure",
    "",
    "Exhaust gas sensor",
    "Particulate filter",
    "EGR / VVT system",
];

/// Decodes the four data bytes A B C D after `41 01`.
pub fn decode(d: &[u8]) -> Option<MonitorStatus> {
    if d.len() < 4 {
        return None;
    }
    let (a, b, c, dd) = (d[0], d[1], d[2], d[3]);
    let compression_ignition = b & 0x08 != 0;
    let mut monitors = Vec::new();
    for (i, name) in ["Misfire", "Fuel system", "Comprehensive components"].iter().enumerate() {
        monitors.push(Monitor {
            name: (*name).into(),
            available: b & (1 << i) != 0,
            complete: b & (1 << (i + 4)) == 0,
        });
    }
    let names = if compression_ignition { COMPRESSION } else { SPARK };
    for (i, name) in names.iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        let available = c & (1 << i) != 0;
        if available {
            monitors.push(Monitor { name: (*name).into(), available, complete: dd & (1 << i) == 0 });
        }
    }
    Some(MonitorStatus { mil_on: a & 0x80 != 0, dtc_count: a & 0x7F, compression_ignition, monitors })
}

/// Builds the four bytes (the inverse of [`decode`]); used by the simulator.
pub fn encode(mil_on: bool, dtc_count: u8, compression: bool, avail: u8, incomplete: u8, base_incomplete: u8) -> [u8; 4] {
    let a = ((mil_on as u8) << 7) | (dtc_count & 0x7F);
    let b = 0x07 | ((compression as u8) << 3) | ((base_incomplete & 0x07) << 4);
    [a, b, avail, incomplete & avail]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_spark_engine() {
        // MIL on, 2 codes; misfire/fuel/components available and complete;
        // catalyst, evap, O2, O2 heater, EGR available; catalyst incomplete.
        let s = decode(&[0x82, 0x07, 0xE5, 0x01]).unwrap();
        assert!(s.mil_on);
        assert_eq!(s.dtc_count, 2);
        assert!(!s.compression_ignition);
        let cat = s.monitors.iter().find(|m| m.name == "Catalyst").unwrap();
        assert!(cat.available && !cat.complete);
        let o2 = s.monitors.iter().find(|m| m.name == "Oxygen sensor").unwrap();
        assert!(o2.complete);
        assert!(s.monitors.iter().all(|m| m.name != "Heated catalyst"));
        assert_eq!(encode(true, 2, false, 0xE5, 0x01, 0), [0x82, 0x07, 0xE5, 0x01]);
    }

    #[test]
    fn decodes_diesel() {
        let s = decode(&[0x00, 0x0F, 0x48, 0x40]).unwrap();
        assert!(s.compression_ignition);
        let pf = s.monitors.iter().find(|m| m.name == "Particulate filter").unwrap();
        assert!(pf.available && !pf.complete);
        assert!(s.monitors.iter().any(|m| m.name == "Boost pressure" && m.complete));
        assert!(decode(&[0x00]).is_none());
    }
}
