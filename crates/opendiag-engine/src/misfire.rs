//! Misfire counts per cylinder from OBD service 06 (on-board monitoring test
//! results, CAN only). Read only. Call it repeatedly for a near-live view: how
//! often the car updates the counts is up to the manufacturer.

use crate::{vehicle_of, Engine, Result};
use opendiag_obd::mode06::{self as m6, TestRecord};
use opendiag_store::compat::TestResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CylinderMisfire {
    pub cylinder: u8,
    /// Misfires in the current (or last finished) driving cycle.
    pub current: Option<u32>,
    /// The car's weighted average over the last ten driving cycles.
    pub average: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MisfireReport {
    pub simulated: bool,
    /// The car reports misfire counts per cylinder.
    pub supported: bool,
    /// Why not, in plain words (when `supported` is false).
    pub reason: Option<String>,
    pub ecu: Option<String>,
    pub cylinders: Vec<CylinderMisfire>,
    /// Errors while reading single cylinders.
    pub notes: Vec<String>,
    pub at_ms: i64,
}

impl Engine {
    pub fn misfire_read(&mut self) -> Result<MisfireReport> {
        let c = vehicle_of(&mut self.conn)?;
        let simulated = c.simulated();
        let mut report = MisfireReport {
            simulated,
            supported: false,
            reason: None,
            ecu: None,
            cylinders: vec![],
            notes: vec![],
            at_ms: chrono::Utc::now().timestamp_millis(),
        };
        if !c.drv.is_can() {
            report.reason = Some(
                "This vehicle uses an older protocol (K-line or J1850). There, misfire counts are \
                 not in a standard format, so OpenDiag cannot read them."
                    .into(),
            );
            return Ok(report);
        }
        // Which ECU supports which test groups: asked once per connection.
        if c.mode06.is_none() {
            let mut found: Vec<(String, BTreeSet<u8>)> = vec![];
            let mut next = vec![0x00u8];
            while let Some(base) = next.pop() {
                let Ok(msgs) = c.req(&[0x06, base]) else {
                    break;
                };
                let mut more = false;
                for msg in &msgs {
                    let Some(mids) = m6::parse_supported(&msg.data) else {
                        continue;
                    };
                    more |= mids.contains(&base.wrapping_add(0x20)) && base < 0xE0;
                    match found.iter_mut().find(|(e, _)| *e == msg.ecu) {
                        Some((_, s)) => s.extend(mids),
                        None => found.push((msg.ecu.clone(), mids.into_iter().collect())),
                    }
                }
                if more {
                    next.push(base + 0x20);
                }
            }
            c.mode06 = Some(found);
        }
        let groups = c.mode06.clone().unwrap_or_default();
        let Some((ecu, mids)) = groups
            .into_iter()
            .find(|(_, s)| s.iter().any(|m| m6::misfire_cylinder(*m).is_some()))
        else {
            report.reason = Some(
                "The vehicle does not report misfire counts per cylinder (no answer to service 06 \
                 misfire tests). Many diesels and some petrol cars do not."
                    .into(),
            );
            self.record_result(
                "read_misfire_counts",
                TestResult::NotSupported,
                "no misfire OBDMIDs",
            );
            return Ok(report);
        };
        report.supported = true;
        report.ecu = Some(ecu.clone());
        for mid in mids
            .iter()
            .copied()
            .filter(|m| m6::misfire_cylinder(*m).is_some())
        {
            let cylinder = m6::misfire_cylinder(mid).unwrap_or(0);
            let records: Vec<TestRecord> = match c.req(&[0x06, mid]) {
                Ok(msgs) => msgs
                    .iter()
                    .filter(|m| m.ecu == ecu)
                    .filter_map(|m| m6::parse_records(&m.data))
                    .flatten()
                    .collect(),
                Err(e) => {
                    report.notes.push(format!("Cylinder {cylinder}: {e}"));
                    continue;
                }
            };
            let (current, average) = m6::misfire_counts(&records, mid);
            if current.is_none() && average.is_none() {
                report.notes.push(format!(
                    "Cylinder {cylinder}: the answer held no misfire counts."
                ));
            }
            report.cylinders.push(CylinderMisfire {
                cylinder,
                current,
                average,
            });
        }
        let any = report
            .cylinders
            .iter()
            .any(|c| c.current.is_some() || c.average.is_some());
        self.record_result(
            "read_misfire_counts",
            if any {
                TestResult::Works
            } else {
                TestResult::Partial
            },
            "",
        );
        Ok(report)
    }
}
