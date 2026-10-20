//! Trucks, buses and machines on SAE J1939: one read-only scan that collects
//! active and previous trouble codes and a set of engine values.

use crate::{Engine, EngineError, Result};
use opendiag_driver::DriverError;
use opendiag_j1939::{self as j, J1939Message, Lamps, ParamValue, PARAMS};
use opendiag_store::compat::TestResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct J1939Code {
    pub spn: u32,
    pub fmi: u8,
    pub occurrences: Option<u8>,
    pub spn_name: Option<String>,
    pub fmi_text: String,
    /// The ECU uses an older SPN layout, so the SPN may be wrong.
    pub old_conversion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct J1939Module {
    pub address: u8,
    pub name: String,
    pub lamps: Option<Lamps>,
    pub active: Vec<J1939Code>,
    /// `None` = the module did not answer the DM2 request.
    pub previous: Option<Vec<J1939Code>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct J1939Report {
    pub simulated: bool,
    pub modules: Vec<J1939Module>,
    pub values: Vec<ParamValue>,
    pub vin: Option<String>,
    /// What could not be read, in plain words.
    pub notes: Vec<String>,
}

fn module(m: &mut BTreeMap<u8, J1939Module>, sa: u8) -> &mut J1939Module {
    m.entry(sa).or_insert_with(|| J1939Module {
        address: sa,
        name: j::address_name(sa),
        lamps: None,
        active: vec![],
        previous: None,
    })
}

fn codes(data: &[u8]) -> Option<(Lamps, Vec<J1939Code>)> {
    let dm = j::decode_dm(data)?;
    let list = dm
        .dtcs
        .into_iter()
        .map(|d| J1939Code {
            spn: d.spn,
            fmi: d.fmi,
            occurrences: (d.occurrences != 0x7F).then_some(d.occurrences),
            spn_name: j::spn_name(d.spn).map(str::to_string),
            fmi_text: j::fmi_text(d.fmi).to_string(),
            old_conversion: d.old_conversion,
        })
        .collect();
    Some((dm.lamps, list))
}

impl Engine {
    /// Switches the adapter to J1939 and reads what the vehicle offers. Sends
    /// only requests on the safety policy's read-only list; most values are
    /// heard on the bus without asking. OBD-II needs a reconnect afterwards.
    pub fn j1939_scan(&mut self) -> Result<J1939Report> {
        let c = self.conn.as_mut().ok_or(EngineError::NotConnected)?;
        let simulated = c.simulated();
        let session = c.session_id;
        let port = c.drv.j1939().ok_or_else(|| {
            EngineError::Invalid(
                "This adapter cannot talk J1939. It needs an ELM327 (v1.4b or newer) or STN adapter; raw K-line cables cannot."
                    .into(),
            )
        })?;
        port.j1939_start()?;
        c.ecus.clear();
        c.supported.clear();
        c.vehicle_error = Some("Switched to J1939 (trucks). Reconnect to use OBD-II.".into());
        // Broadcast intervals are up to 1 s (DM1, temperatures); the simulator answers at once.
        let window = if simulated {
            Duration::from_millis(150)
        } else {
            Duration::from_millis(1300)
        };
        let mut notes = Vec::new();
        let mut heard_anything = false;

        let mut modules: BTreeMap<u8, J1939Module> = BTreeMap::new();

        // DM1: listen, and ask if nothing was heard.
        let mut dm1 = port.j1939_monitor(j::PGN_DM1, window)?;
        if dm1.messages.is_empty() {
            match port.j1939_request(j::PGN_DM1) {
                Ok(p) => dm1 = p,
                Err(DriverError::NoData) => {}
                Err(e) => return Err(e.into()),
            }
        }
        for m in dm1.messages.iter().filter(|m| m.pgn == j::PGN_DM1) {
            heard_anything = true;
            if let Some((lamps, list)) = codes(&m.data) {
                let md = module(&mut modules, m.source);
                md.lamps = Some(lamps);
                md.active = list;
            }
        }
        if dm1.messages.is_empty() {
            notes.push("No module sent its active fault codes (DM1).".into());
        }

        // DM2: previously active codes, on request.
        match port.j1939_request(j::PGN_DM2) {
            Ok(p) => {
                for m in p.messages.iter().filter(|m| m.pgn == j::PGN_DM2) {
                    heard_anything = true;
                    if let Some((_, list)) = codes(&m.data) {
                        module(&mut modules, m.source).previous = Some(list);
                    }
                }
            }
            Err(DriverError::NoData) => {
                notes.push("No module answered the request for previous fault codes (DM2).".into())
            }
            Err(e) => notes.push(format!("Previous fault codes (DM2): {e}")),
        }

        // Values: listen for broadcast groups, request the others.
        let mut values: BTreeMap<(String, u8), ParamValue> = BTreeMap::new();
        let mut pgns: Vec<(u32, bool)> = PARAMS.iter().map(|d| (d.pgn, d.broadcast)).collect();
        pgns.dedup();
        for (pgn, broadcast) in pgns {
            let got = if broadcast {
                port.j1939_monitor(pgn, window)
            } else {
                port.j1939_request(pgn)
            };
            let msgs: Vec<J1939Message> = match got {
                Ok(p) => p.messages,
                Err(DriverError::NoData) => vec![],
                Err(e @ (DriverError::Link(_) | DriverError::Timeout(_))) => return Err(e.into()),
                Err(e) => {
                    notes.push(format!("PGN {pgn}: {e}"));
                    vec![]
                }
            };
            for m in msgs.iter().filter(|m| m.pgn == pgn) {
                heard_anything = true;
                module(&mut modules, m.source);
                for v in j::decode(m) {
                    // The newest message wins.
                    values.insert((v.key.clone(), v.source), v);
                }
            }
        }
        let mut values: Vec<ParamValue> = values.into_values().collect();
        values.sort_by_key(|v| PARAMS.iter().position(|d| d.key == v.key));

        let vin = match port.j1939_request(j::PGN_VI) {
            Ok(p) => p.messages.iter().find_map(|m| j::decode_vin(&m.data)),
            Err(_) => None,
        };
        if vin.is_none() {
            notes.push("The VIN was not reported.".into());
        }
        if !heard_anything {
            notes.insert(
                0,
                "Nothing was heard on the J1939 network. Check the ignition, that the cable fits the vehicle's diagnostic socket (often a 9-pin Deutsch connector), and that the vehicle uses J1939 at 250 kbit/s."
                    .into(),
            );
        }
        let report = J1939Report {
            simulated,
            modules: modules.into_values().collect(),
            values,
            vin,
            notes,
        };
        let _ = self.store.log_operation(
            session,
            "j1939_scan",
            &serde_json::json!({
                "modules": report.modules.len(),
                "active_codes": report.modules.iter().map(|m| m.active.len()).sum::<usize>(),
                "values": report.values.len(),
            }),
        );
        if heard_anything {
            self.record_result(
                "j1939_read_codes",
                if dm1.messages.is_empty() {
                    TestResult::Partial
                } else {
                    TestResult::Works
                },
                "",
            );
            let any_value = report.values.iter().any(|v| v.value.is_some());
            self.record_result(
                "j1939_live_data",
                if any_value {
                    TestResult::Works
                } else {
                    TestResult::Partial
                },
                &format!("{} values", report.values.len()),
            );
        }
        Ok(report)
    }
}
