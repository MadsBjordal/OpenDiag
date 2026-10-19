//! The "Scan Vehicle" workflow.

use crate::report::*;
use crate::{read_pid, Conn, Engine, Result};
use opendiag_elm327::ElmError;
use opendiag_obd::{dtc::parse_dtc_response, ecu_label, pid, readiness, vin, EcuMessage};
use opendiag_store::compat::TestResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub step: u8,
    pub total: u8,
    pub label: String,
}

const STEPS: [&str; 8] = [
    "Checking the adapter",
    "Checking communication with the vehicle",
    "Identifying the vehicle",
    "Finding control modules",
    "Reading fault codes",
    "Reading readiness monitors",
    "Reading freeze frame data",
    "Reading measurements",
];

/// Systems generic OBD-II cannot reach. Listed so the report never implies they were checked.
const NOT_COVERED: [&str; 11] = [
    "ABS / brakes",
    "Airbag / SRS",
    "Body control",
    "Instrument cluster",
    "Power steering",
    "Climate control",
    "Lighting",
    "Infotainment",
    "Parking assistance",
    "Suspension",
    "Battery management (hybrid / EV)",
];

const FREEZE_PIDS: [u8; 9] = [0x04, 0x05, 0x06, 0x07, 0x0B, 0x0C, 0x0D, 0x0F, 0x11];

fn category(ecu: &str) -> &'static str {
    match ecu.to_ascii_uppercase().as_str() {
        "7E8" | "18DAF110" => "Engine",
        "7E9" | "18DAF118" => "Transmission",
        _ => "Emissions-related module",
    }
}

pub(crate) struct DtcRead {
    pub findings: Vec<(String, DtcKind, String)>,
    pub answered: Vec<String>,
    pub errors: Vec<String>,
    pub permanent_supported: bool,
}

/// Reads stored, pending and permanent codes from every ECU.
pub(crate) fn read_all_dtcs(c: &mut Conn) -> DtcRead {
    let is_can = c.elm.is_can();
    let mut out = DtcRead {
        findings: Vec::new(),
        answered: Vec::new(),
        errors: Vec::new(),
        permanent_supported: false,
    };
    for (sid, kind) in [
        (0x03u8, DtcKind::Stored),
        (0x07, DtcKind::Pending),
        (0x0A, DtcKind::Permanent),
    ] {
        match c.req(&[sid]) {
            Ok(msgs) => {
                let mut per_ecu: std::collections::BTreeMap<String, Vec<&EcuMessage>> =
                    Default::default();
                for m in &msgs {
                    if m.data.first() == Some(&(sid + 0x40)) {
                        per_ecu.entry(m.ecu.clone()).or_default().push(m);
                    }
                }
                if sid == 0x0A && !per_ecu.is_empty() {
                    out.permanent_supported = true;
                }
                for (ecu, ms) in per_ecu {
                    if sid == 0x03 && !out.answered.contains(&ecu) {
                        out.answered.push(ecu.clone());
                    }
                    for m in ms {
                        for d in parse_dtc_response(&m.data, is_can) {
                            let item = (d.0.clone(), kind, ecu.clone());
                            if !out.findings.contains(&item) {
                                out.findings.push(item);
                            }
                        }
                    }
                }
            }
            Err(ElmError::NoData) => {}
            Err(e) => out.errors.push(format!(
                "{}: {e}",
                match sid {
                    0x03 => "Stored codes",
                    0x07 => "Pending codes",
                    _ => "Permanent codes",
                }
            )),
        }
    }
    out
}

pub(crate) fn read_readiness(
    c: &mut Conn,
    errors: &mut Vec<String>,
) -> Vec<(String, readiness::MonitorStatus)> {
    let mut out = Vec::new();
    match c.req(&[0x01, 0x01]) {
        Ok(msgs) => {
            for m in msgs {
                if m.data.len() >= 6 && m.data[0] == 0x41 && m.data[1] == 0x01 {
                    if let Some(s) = readiness::decode(&m.data[2..]) {
                        out.push((m.ecu.clone(), s));
                    }
                }
            }
        }
        Err(ElmError::NoData) => {}
        Err(e) => errors.push(format!("Readiness: {e}")),
    }
    out
}

pub(crate) fn read_freeze_frames(c: &mut Conn, errors: &mut Vec<String>) -> Vec<FreezeFrame> {
    let mut frames: Vec<FreezeFrame> = Vec::new();
    let msgs = match c.req(&[0x02, 0x02, 0x00]) {
        Ok(m) => m,
        Err(ElmError::NoData) => return frames,
        Err(e) => {
            errors.push(format!("Freeze frame: {e}"));
            return frames;
        }
    };
    for m in msgs {
        if m.data.len() >= 5 && m.data[0] == 0x42 && m.data[1] == 0x02 {
            if let Some(d) = opendiag_obd::dtc::Dtc::from_bytes(m.data[3], m.data[4]) {
                frames.push(FreezeFrame {
                    ecu: m.ecu.clone(),
                    dtc: Some(d.0),
                    values: Vec::new(),
                });
            }
        }
    }
    if frames.is_empty() {
        return frames;
    }
    // Which PIDs were frozen.
    let mut supported: std::collections::BTreeMap<String, Vec<u8>> = Default::default();
    if let Ok(msgs) = c.req(&[0x02, 0x00, 0x00]) {
        for m in msgs {
            if m.data.len() >= 7 && m.data[0] == 0x42 && m.data[1] == 0x00 {
                supported.insert(m.ecu.clone(), pid::supported_from_bitmap(0, &m.data[3..7]));
            }
        }
    }
    for p in FREEZE_PIDS {
        if !supported.values().any(|s| s.contains(&p)) {
            continue;
        }
        let Some(def) = pid::def(p) else { continue };
        match c.req(&[0x02, p, 0x00]) {
            Ok(msgs) => {
                for m in msgs {
                    if m.data.len() >= 3 && m.data[0] == 0x42 && m.data[1] == p {
                        if let (Some(v), Some(f)) = (
                            def.decode(&m.data[3..]),
                            frames.iter_mut().find(|f| f.ecu == m.ecu),
                        ) {
                            f.values.push(Measurement {
                                key: def.key.into(),
                                name: def.name.into(),
                                value: v,
                                unit: def.unit.into(),
                                ecu: m.ecu.clone(),
                                source: "vehicle (freeze frame)".into(),
                            });
                        }
                    }
                }
            }
            Err(ElmError::NoData) => {}
            Err(e) => {
                errors.push(format!("Freeze frame {}: {e}", def.name));
                break;
            }
        }
    }
    frames
}

fn read_identity(c: &mut Conn, errors: &mut Vec<String>) -> VehicleIdentity {
    let is_can = c.elm.is_can();
    let mut ident = VehicleIdentity {
        vin: None,
        calibration_ids: Vec::new(),
        ecu_names: Vec::new(),
        obd_standard: None,
        fuel_type: None,
        note: "The VIN identifies the vehicle model. It does not prove which ECU hardware or software is fitted.".into(),
    };
    let mut info_pids: std::collections::BTreeMap<String, Vec<u8>> = Default::default();
    match c.req(&[0x09, 0x00]) {
        Ok(msgs) => {
            for m in msgs {
                if m.data.len() >= 6 && m.data[0] == 0x49 && m.data[1] == 0x00 {
                    // CAN answers carry the 4 bitmap bytes right after the PID; legacy adds a message count byte.
                    let bm = if is_can {
                        &m.data[2..6]
                    } else {
                        &m.data[m.data.len() - 4..]
                    };
                    info_pids.insert(m.ecu.clone(), pid::supported_from_bitmap(0, bm));
                }
            }
        }
        Err(ElmError::NoData) => {}
        Err(e) => errors.push(format!("Vehicle information: {e}")),
    }
    for (p, label) in [
        (0x02u8, "VIN"),
        (0x04, "Calibration ID"),
        (0x0A, "ECU name"),
    ] {
        if !info_pids.values().any(|s| s.contains(&p)) {
            continue;
        }
        match c.req(&[0x09, p]) {
            Ok(msgs) => {
                let ecus: Vec<String> = {
                    let mut e: Vec<String> = msgs.iter().map(|m| m.ecu.clone()).collect();
                    e.dedup();
                    e
                };
                for ecu in ecus {
                    let mine: Vec<&EcuMessage> = msgs.iter().filter(|m| m.ecu == ecu).collect();
                    let Some(text) = vin::info_text(&mine, p, is_can) else {
                        continue;
                    };
                    match p {
                        0x02 => {
                            if ident.vin.is_none() {
                                if let Some(v) = vin::vin_from_text(&text) {
                                    ident.vin = Some(vin::decode(&v));
                                } else {
                                    errors.push(format!("VIN: the answer \"{text}\" is not a VIN"));
                                }
                            }
                        }
                        0x04 => ident.calibration_ids.push((ecu.clone(), text)),
                        _ => ident.ecu_names.push((ecu.clone(), text)),
                    }
                }
            }
            Err(ElmError::NoData) => {}
            Err(e) => errors.push(format!("{label}: {e}")),
        }
    }
    for (p, set) in [(0x1Cu8, 0u8), (0x51, 1)] {
        if c.ecu_supporting(p).is_none() {
            continue;
        }
        if let Ok(msgs) = c.req(&[0x01, p]) {
            if let Some(m) = msgs
                .iter()
                .find(|m| m.data.len() >= 3 && m.data[0] == 0x41 && m.data[1] == p)
            {
                if set == 0 {
                    ident.obd_standard = Some(pid::obd_standard(m.data[2]).into());
                } else {
                    ident.fuel_type = Some(pid::fuel_type(m.data[2]).into());
                }
            }
        }
    }
    ident
}

impl Engine {
    /// Runs the full scan. Every step that fails is reported, never hidden.
    pub fn scan(&mut self, progress: &mut dyn FnMut(ScanProgress)) -> Result<ScanReport> {
        let total = STEPS.len() as u8;
        let mut step = |n: u8| {
            progress(ScanProgress {
                step: n,
                total,
                label: STEPS[(n - 1) as usize].into(),
            })
        };
        step(1);
        let library_explain = |code: &str, lib: &opendiag_knowledge::DtcLibrary| lib.explain(code);
        let platform = self.platform.clone();
        let c = crate::vehicle_of(&mut self.conn)?;
        let simulated = c.simulated();
        let mut warnings: Vec<String> = c.elm.capabilities().notes.clone();
        let mut errors: Vec<String> = Vec::new();
        step(2);
        if let Err(e) = c.req(&[0x01, 0x00]) {
            return Err(crate::EngineError::NoVehicle(e.to_string()));
        }
        step(3);
        let identity = read_identity(c, &mut errors);
        if let Some(v) = &identity.vin {
            warnings.extend(v.warnings.iter().cloned());
            c.vin = v.valid_format.then(|| v.vin.clone());
        }
        c.calids = identity
            .calibration_ids
            .iter()
            .map(|(_, id)| id.clone())
            .collect();
        step(4);
        let ecus = c.ecus.clone();
        step(5);
        let dtcs = read_all_dtcs(c);
        errors.extend(dtcs.errors.iter().cloned());
        step(6);
        let readiness = read_readiness(c, &mut errors);
        step(7);
        let freeze_frames = read_freeze_frames(c, &mut errors);
        step(8);
        let mut measurements = Vec::new();
        for def in pid::PIDS.iter().filter(|p| p.common) {
            if c.ecu_supporting(def.pid).is_none() {
                continue;
            }
            match read_pid(c, def.pid) {
                Ok(m) => measurements.push(m),
                Err(e) => errors.push(format!("{}: {e}", def.name)),
            }
        }
        if c.elm.capabilities().voltage_reading == opendiag_core::Tri::Yes {
            if let Some(v) = c.elm.read_voltage() {
                measurements.push(Measurement {
                    key: "adapter_voltage".into(),
                    name: "Battery voltage at the diagnostic socket".into(),
                    value: v,
                    unit: "V".into(),
                    ecu: String::new(),
                    source: "adapter".into(),
                });
            }
        }
        if let Some(v) = measurements
            .iter()
            .find(|m| m.key == "adapter_voltage" || m.key == "module_voltage")
            .map(|m| m.value)
        {
            if v < 11.8 {
                warnings.push(format!("Battery voltage is low ({v:.1} V). Low voltage can cause false fault codes and unreliable communication."));
            }
        }
        if c.elm.capabilities().headers == opendiag_core::Tri::No {
            warnings.push("This adapter cannot show which module answered: results from several modules may be merged.".into());
        }
        if !dtcs.permanent_supported {
            warnings.push("Permanent codes (service 0A) are not supported by this vehicle or were not reported.".into());
        }

        let lib = &self.library;
        let findings: Vec<DtcFinding> = dtcs
            .findings
            .iter()
            .map(|(code, kind, ecu)| DtcFinding {
                code: code.clone(),
                ecu: ecu.clone(),
                ecu_label: ecu_label(ecu),
                kind: *kind,
                explanation: library_explain(code, lib),
            })
            .collect();

        let mut modules: Vec<ModuleResult> = ecus
            .iter()
            .map(|e| {
                let answered = dtcs.answered.contains(e);
                ModuleResult {
                    category: category(e).into(),
                    ecu: Some(e.clone()),
                    label: format!("{} ({})", category(e), ecu_label(e)),
                    status: if answered {
                        ModuleStatus::Scanned
                    } else {
                        ModuleStatus::PartlyScanned
                    },
                    detail: if answered {
                        "Generic OBD-II: fault codes, readiness and data read.".into()
                    } else {
                        "Answers OBD-II data requests but did not report fault codes.".into()
                    },
                }
            })
            .collect();
        for cat in NOT_COVERED {
            modules.push(ModuleResult {
                category: cat.into(),
                ecu: None,
                label: cat.into(),
                status: ModuleStatus::NotScanned,
                detail: "Not reachable with generic OBD-II. Manufacturer-specific diagnostics are planned (Phase 3).".into(),
            });
        }

        let scanned = modules
            .iter()
            .filter(|m| m.status == ModuleStatus::Scanned)
            .count();
        let health = if !findings.is_empty() {
            let stored = findings
                .iter()
                .filter(|f| f.kind == DtcKind::Stored)
                .count();
            Health {
                verdict: Verdict::FaultsFound,
                headline: format!("{} fault code(s) found ({} stored)", findings.len(), stored),
                detail: format!(
                    "Found in the {scanned} module(s) that could be scanned. {} other systems were not scanned.",
                    NOT_COVERED.len()
                ),
            }
        } else if !errors.is_empty() || scanned == 0 {
            Health {
                verdict: Verdict::Incomplete,
                headline: "Scan incomplete".into(),
                detail: "Some information could not be read (see communication problems). Do not treat this as a clean result.".into(),
            }
        } else {
            Health {
                verdict: Verdict::NoFaultsInScannedModules,
                headline: format!("No fault codes in the {scanned} scanned module(s)"),
                detail: format!(
                    "Only emissions-related modules were scanned. {} other systems (ABS, airbag, body…) were not checked, so this is not a full health check.",
                    NOT_COVERED.len()
                ),
            }
        };

        let operations = self
            .store
            .operations(c.session_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, what, _)| what == "clear_dtcs")
            .map(|(at, what, d)| Operation {
                at,
                what,
                detail: d
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
            .collect();
        let report = ScanReport {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: opendiag_store::now(),
            app: opendiag_core::APP_NAME.into(),
            app_version: opendiag_core::APP_VERSION.into(),
            platform,
            simulated,
            link: c.elm.link_info(),
            adapter: c.elm.capabilities().clone(),
            protocol: c.elm.protocol(),
            vehicle: identity,
            modules,
            dtcs: findings,
            readiness,
            freeze_frames,
            measurements,
            warnings,
            communication_errors: errors,
            health,
            notes: String::new(),
            operations,
            cautions: STANDARD_CAUTIONS.iter().map(|s| s.to_string()).collect(),
        };
        let session = c.session_id;
        let vin = c.vin.clone();
        let vehicle_id = match &vin {
            Some(v) => {
                let info = report.vehicle.vin.clone();
                let make = info.as_ref().and_then(|i| i.manufacturer.clone());
                let label = if simulated {
                    report
                        .link
                        .name
                        .trim_start_matches("Simulator: ")
                        .to_string()
                } else {
                    format!(
                        "{} (…{})",
                        make.clone().unwrap_or_else(|| "Vehicle".into()),
                        &v[v.len().saturating_sub(4)..]
                    )
                };
                let id = self.store.upsert_vehicle(&opendiag_store::Vehicle {
                    id: 0,
                    label,
                    vin: Some(v.clone()),
                    make,
                    model: None,
                    year: None,
                    kind: None,
                    notes: String::new(),
                    simulated,
                    created_at: String::new(),
                })?;
                self.store.set_session_vehicle(session, id)?;
                Some(id)
            }
            None => None,
        };
        if let Some(c) = self.conn.as_mut() {
            c.vehicle_id = vehicle_id;
        }
        let value =
            serde_json::to_value(&report).map_err(|e| crate::EngineError::Store(e.to_string()))?;
        let report_id = self.store.save_report(
            Some(session),
            vehicle_id,
            &report.title(),
            &value,
            simulated,
            report.dtc_count() as i64,
        )?;
        self.store.log_operation(session, "scan", &serde_json::json!({ "report_id": report_id, "dtcs": report.dtc_count(), "verdict": report.health.verdict }))?;
        let ok = |b: bool| {
            if b {
                TestResult::Works
            } else {
                TestResult::Fails
            }
        };
        self.record_result("read_dtcs", ok(!dtcs.answered.is_empty()), "");
        self.record_result("read_vehicle_info", ok(report.vehicle.vin.is_some()), "");
        self.record_result("read_readiness", ok(!report.readiness.is_empty()), "");
        if !report.measurements.is_empty() {
            self.record_result("live_data", TestResult::Works, "read during scan");
        }
        self.record_result("adapter_probe", TestResult::Works, "");
        let mut r = report;
        r.id = report_id.to_string();
        self.store
            .update_report(report_id, &serde_json::to_value(&r).unwrap_or_default())?;
        Ok(r)
    }

    pub fn last_report_id(&self) -> Option<i64> {
        self.store.reports(None).ok()?.first().map(|r| r.id)
    }
}
