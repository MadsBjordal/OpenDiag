//! Service 06: on-board monitoring test results, CAN format (SAE J1979 /
//! ISO 15031-5, "OBDMID" form). Only the CAN format is decoded: older
//! protocols (K-line, J1850) answer service 06 in a manufacturer-defined way.
//!
//! Each answer is `46` followed by 9-byte records:
//! `OBDMID TID UASID value(2) min(2) max(2)`.
//! Asking for OBDMID 00, 20, 40 … returns `46 MID bitmap(4)`: which OBDMIDs
//! the ECU supports, like PID support for service 01.

use serde::{Deserialize, Serialize};

/// OBDMID A1: misfire monitor, general data.
pub const MISFIRE_GENERAL: u8 = 0xA1;
/// OBDMIDs A2–AD: misfire counts for cylinders 1–12.
pub const MISFIRE_CYL_FIRST: u8 = 0xA2;
pub const MISFIRE_CYL_LAST: u8 = 0xAD;
/// Test ID 0B: EWMA (weighted average) misfire counts over the last ten driving cycles.
pub const TID_MISFIRE_AVERAGE: u8 = 0x0B;
/// Test ID 0C: misfire counts for the last/current driving cycle.
pub const TID_MISFIRE_CURRENT: u8 = 0x0C;
/// Unit and scaling ID 24: counts (1 per bit, unsigned).
pub const UASID_COUNTS: u8 = 0x24;

/// One test result record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRecord {
    pub mid: u8,
    pub tid: u8,
    pub uasid: u8,
    pub value: u16,
    pub min: u16,
    pub max: u16,
}

/// The OBDMID support ranges to ask for (00, 20, … E0).
pub const SUPPORT_RANGES: [u8; 8] = [0x00, 0x20, 0x40, 0x60, 0x80, 0xA0, 0xC0, 0xE0];

/// OBDMIDs announced in a support answer (`46 base bitmap(4) [base bitmap(4)]…`).
pub fn parse_supported(data: &[u8]) -> Option<Vec<u8>> {
    if data.first() != Some(&0x46) || data.len() < 6 {
        return None;
    }
    let mut out = Vec::new();
    for chunk in data[1..].chunks(5) {
        if chunk.len() < 5 || !chunk[0].is_multiple_of(0x20) {
            return None;
        }
        out.extend(crate::pid::supported_from_bitmap(chunk[0], &chunk[1..5]));
    }
    Some(out)
}

/// The test records in a service 06 answer. `None` = not a CAN-format answer.
pub fn parse_records(data: &[u8]) -> Option<Vec<TestRecord>> {
    if data.first() != Some(&0x46) || data.len() < 10 || !(data.len() - 1).is_multiple_of(9) {
        return None;
    }
    let w = |b: &[u8]| u16::from_be_bytes([b[0], b[1]]);
    Some(
        data[1..]
            .chunks(9)
            .map(|r| TestRecord {
                mid: r[0],
                tid: r[1],
                uasid: r[2],
                value: w(&r[3..5]),
                min: w(&r[5..7]),
                max: w(&r[7..9]),
            })
            .collect(),
    )
}

/// The cylinder number (1–12) of a misfire OBDMID.
pub fn misfire_cylinder(mid: u8) -> Option<u8> {
    (MISFIRE_CYL_FIRST..=MISFIRE_CYL_LAST)
        .contains(&mid)
        .then(|| mid - MISFIRE_CYL_FIRST + 1)
}

/// Misfire counts in one cylinder's records: (current/last drive, ten-drive average).
/// Only records in counts (UASID 24) are used; anything else is not a count.
pub fn misfire_counts(records: &[TestRecord], mid: u8) -> (Option<u32>, Option<u32>) {
    let get = |tid: u8| {
        records
            .iter()
            .find(|r| r.mid == mid && r.tid == tid && r.uasid == UASID_COUNTS)
            .map(|r| r.value as u32)
    };
    (get(TID_MISFIRE_CURRENT), get(TID_MISFIRE_AVERAGE))
}

/// Builds a record (for the simulator and tests).
pub fn encode_record(r: &TestRecord) -> [u8; 9] {
    let [v1, v2] = r.value.to_be_bytes();
    let [n1, n2] = r.min.to_be_bytes();
    let [x1, x2] = r.max.to_be_bytes();
    [r.mid, r.tid, r.uasid, v1, v2, n1, n2, x1, x2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_answer() {
        // A0 range: A1, A2, A3, A4, A5 supported.
        let s = parse_supported(&[0x46, 0xA0, 0xF8, 0x00, 0x00, 0x00]).unwrap();
        assert_eq!(s, vec![0xA1, 0xA2, 0xA3, 0xA4, 0xA5]);
        assert!(parse_supported(&[0x46, 0xA1, 0, 0, 0, 0]).is_none());
        assert!(parse_supported(&[0x7F, 0x06, 0x12]).is_none());
    }

    #[test]
    fn cylinder_records() {
        let cur = TestRecord {
            mid: 0xA2,
            tid: 0x0C,
            uasid: 0x24,
            value: 37,
            min: 0,
            max: 0xFFFF,
        };
        let avg = TestRecord {
            mid: 0xA2,
            tid: 0x0B,
            uasid: 0x24,
            value: 12,
            min: 0,
            max: 0xFFFF,
        };
        let mut msg = vec![0x46];
        msg.extend(encode_record(&avg));
        msg.extend(encode_record(&cur));
        let recs = parse_records(&msg).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(misfire_counts(&recs, 0xA2), (Some(37), Some(12)));
        assert_eq!(misfire_counts(&recs, 0xA3), (None, None));
        assert_eq!(misfire_cylinder(0xA2), Some(1));
        assert_eq!(misfire_cylinder(0xAD), Some(12));
        assert_eq!(misfire_cylinder(0xA1), None);
        // Wrong length (a legacy-format answer) is not decoded.
        assert!(parse_records(&[0x46, 0x01, 0x02, 0x03]).is_none());
    }

    #[test]
    fn other_units_are_not_counts() {
        let r = TestRecord {
            mid: 0xA2,
            tid: 0x0C,
            uasid: 0x01,
            value: 5,
            min: 0,
            max: 0,
        };
        assert_eq!(misfire_counts(&[r], 0xA2), (None, None));
    }
}
