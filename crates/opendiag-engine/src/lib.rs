//! The OpenDiag diagnostic engine.
//!
//! One [`Engine`] per app. It owns the local database, the knowledge library and
//! at most one adapter connection, and offers the workflows the UI needs:
//! connect, scan, live data, clear codes (through the safety policy), reports,
//! garage, guided diagnostics and the compatibility database.
//!
//! The same engine runs on Windows, Linux, macOS and Android; only the byte link
//! to the adapter differs per platform.

pub mod catalog;
mod clear;
pub mod pdf;
pub mod report;
mod scan;

pub use clear::{ClearPlan, ClearResult, EcuClearResult};
pub use scan::ScanProgress;

use opendiag_core::{AdapterCapabilities, ByteLink, LinkInfo, ObdProtocol, SupportStatus};
use opendiag_definitions::Definition;
use opendiag_elm327::{Elm327, ElmError, LogEntry};
use opendiag_knowledge::{guided, DtcLibrary};
use opendiag_obd::{pid, EcuMessage};
use opendiag_safety::PolicyEngine;
use opendiag_sim::SimControl;
use opendiag_store::compat::{CompatFilter, CompatRecord, TestResult, Verification};
use opendiag_store::Store;
use report::Measurement;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// Version of the risk notice; accepting an older one does not count.
pub const TERMS_VERSION: &str = "2026-10-19";

/// Shortest pause between two vehicle requests, to avoid flooding the adapter or bus.
pub const MIN_REQUEST_GAP: Duration = Duration::from_millis(25);
/// Most live values per call.
pub const MAX_LIVE_KEYS: usize = 12;

#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum EngineError {
    #[error("No adapter is connected.")]
    NotConnected,
    #[error("The adapter is connected, but the vehicle is not: {0}")]
    NoVehicle(String),
    #[error("{0}")]
    Adapter(#[from] ElmError),
    #[error("Storage problem: {0}")]
    Store(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Blocked(String),
}

impl From<opendiag_store::StoreError> for EngineError {
    fn from(e: opendiag_store::StoreError) -> Self {
        EngineError::Store(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, EngineError>;

pub(crate) struct Conn {
    pub elm: Elm327,
    pub sim: Option<SimControl>,
    pub session_id: i64,
    pub vehicle_error: Option<String>,
    pub ecus: Vec<String>,
    pub supported: BTreeMap<String, BTreeSet<u8>>,
    pub vin: Option<String>,
    pub vehicle_id: Option<i64>,
    pub calids: Vec<String>,
    pub baro: Option<f64>,
    pub last_request: Instant,
}

impl Conn {
    pub fn simulated(&self) -> bool {
        self.elm.link_info().simulated
    }

    /// Sends a read-only request, keeping a minimum gap between requests.
    pub fn req(&mut self, payload: &[u8]) -> std::result::Result<Vec<EcuMessage>, ElmError> {
        let since = self.last_request.elapsed();
        if since < MIN_REQUEST_GAP {
            std::thread::sleep(MIN_REQUEST_GAP - since);
        }
        let r = self.elm.request(payload, None);
        self.last_request = Instant::now();
        r
    }

    pub fn ecu_supporting(&self, pid: u8) -> Option<String> {
        self.ecus.iter().find(|e| self.supported.get(*e).map(|s| s.contains(&pid)).unwrap_or(false)).cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcuSummary {
    pub address: String,
    pub label: String,
    pub supported_pids: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionStatus {
    pub id: String,
    pub name: String,
    pub risk_level: u8,
    pub status: SupportStatus,
    pub status_label: String,
    pub note: String,
    /// Result of the last time this function ran with this adapter on this OS.
    pub local_result: Option<TestResult>,
    pub best_verification: Option<Verification>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatStatus {
    pub simulated: bool,
    pub platform: String,
    pub adapter: String,
    pub adapter_known_records: usize,
    pub vehicle_known_records: usize,
    pub summary: String,
    pub functions: Vec<FunctionStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSummary {
    pub simulated: bool,
    pub link: LinkInfo,
    pub adapter: AdapterCapabilities,
    pub protocol: Option<ObdProtocol>,
    pub vehicle_connected: bool,
    pub vehicle_error: Option<String>,
    pub ecus: Vec<EcuSummary>,
    pub adapter_voltage: Option<f64>,
    pub session_id: i64,
    pub compatibility: CompatStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSample {
    pub key: String,
    pub name: String,
    pub unit: String,
    pub value: Option<f64>,
    pub ecu: Option<String>,
    pub source: String,
    pub error: Option<String>,
    pub at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveParam {
    pub key: String,
    pub name: String,
    pub unit: String,
    pub min: f64,
    pub max: f64,
    pub category: String,
    pub common: bool,
    pub source: String,
}

pub struct Engine {
    pub(crate) store: Store,
    pub(crate) library: DtcLibrary,
    procedures: Vec<guided::Procedure>,
    definitions: Vec<Definition>,
    pub(crate) policy: PolicyEngine,
    pub(crate) platform: String,
    pub(crate) conn: Option<Conn>,
    pub(crate) clear_plan: Option<clear::PendingPlan>,
}

impl Engine {
    pub fn new(store: Store, platform: &str) -> Self {
        Self {
            store,
            library: DtcLibrary::bundled(),
            procedures: guided::bundled(),
            definitions: opendiag_definitions::bundled(),
            policy: PolicyEngine::new(),
            platform: platform.to_string(),
            conn: None,
            clear_plan: None,
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn platform(&self) -> &str {
        &self.platform
    }

    pub fn library(&self) -> &DtcLibrary {
        &self.library
    }

    // ── connection ──────────────────────────────────────────────────────────

    /// Opens the adapter on `link`, probes it, and tries to reach the vehicle.
    /// A vehicle that does not answer (ignition off…) leaves the adapter connected.
    pub fn connect(&mut self, link: Box<dyn ByteLink>, sim: Option<SimControl>) -> Result<ConnectionSummary> {
        self.disconnect();
        let elm = Elm327::open(link)?;
        let simulated = elm.link_info().simulated;
        let caps = serde_json::to_value(elm.capabilities()).unwrap_or_default();
        let session_id = self.store.create_session(None, &caps, simulated)?;
        self.store.log_operation(session_id, "connect", &serde_json::json!({ "adapter": elm.capabilities().identity, "link": elm.link_info() }))?;
        self.conn = Some(Conn {
            elm,
            sim,
            session_id,
            vehicle_error: None,
            ecus: Vec::new(),
            supported: BTreeMap::new(),
            vin: None,
            vehicle_id: None,
            calids: Vec::new(),
            baro: None,
            last_request: Instant::now() - MIN_REQUEST_GAP,
        });
        self.connect_vehicle()
    }

    /// (Re)tries the vehicle on an open adapter, e.g. after switching the ignition on.
    pub fn connect_vehicle(&mut self) -> Result<ConnectionSummary> {
        let c = self.conn.as_mut().ok_or(EngineError::NotConnected)?;
        match c.elm.connect_vehicle(None) {
            Ok((_proto, msgs)) => {
                c.vehicle_error = None;
                c.ecus.clear();
                c.supported.clear();
                for m in &msgs {
                    if m.data.len() >= 6 && m.data[0] == 0x41 && m.data[1] == 0x00 {
                        if !c.ecus.contains(&m.ecu) {
                            c.ecus.push(m.ecu.clone());
                        }
                        c.supported.entry(m.ecu.clone()).or_default().extend(pid::supported_from_bitmap(0, &m.data[2..6]));
                    }
                }
                c.ecus.sort();
                // Further PID ranges, as long as an ECU says it has them.
                let mut base = 0x20u8;
                loop {
                    if !c.supported.values().any(|s| s.contains(&base)) {
                        break;
                    }
                    match c.req(&[0x01, base]) {
                        Ok(msgs) => {
                            for m in msgs {
                                if m.data.len() >= 6 && m.data[0] == 0x41 && m.data[1] == base {
                                    c.supported.entry(m.ecu.clone()).or_default().extend(pid::supported_from_bitmap(base, &m.data[2..6]));
                                }
                            }
                        }
                        Err(_) => break,
                    }
                    match base.checked_add(0x20) {
                        Some(b) => base = b,
                        None => break,
                    }
                }
            }
            Err(e) => c.vehicle_error = Some(e.to_string()),
        }
        self.summary()
    }

    pub fn disconnect(&mut self) {
        if let Some(mut c) = self.conn.take() {
            c.elm.close();
            let _ = self.store.log_operation(c.session_id, "disconnect", &serde_json::json!({}));
        }
        self.clear_plan = None;
    }

    pub fn is_connected(&self) -> bool {
        self.conn.is_some()
    }

    pub fn summary(&mut self) -> Result<ConnectionSummary> {
        let compat = self.compat_status()?;
        let c = self.conn.as_mut().ok_or(EngineError::NotConnected)?;
        let voltage = if c.elm.capabilities().voltage_reading == opendiag_core::Tri::Yes { c.elm.read_voltage() } else { None };
        Ok(ConnectionSummary {
            simulated: c.simulated(),
            link: c.elm.link_info(),
            adapter: c.elm.capabilities().clone(),
            protocol: c.elm.protocol(),
            vehicle_connected: c.vehicle_error.is_none() && !c.ecus.is_empty(),
            vehicle_error: c.vehicle_error.clone(),
            ecus: c
                .ecus
                .iter()
                .map(|e| EcuSummary { address: e.clone(), label: opendiag_obd::ecu_label(e), supported_pids: c.supported.get(e).map(|s| s.len()).unwrap_or(0) })
                .collect(),
            adapter_voltage: voltage,
            session_id: c.session_id,
            compatibility: compat,
        })
    }

    pub(crate) fn vehicle(&mut self) -> Result<&mut Conn> {
        vehicle_of(&mut self.conn)
    }

    pub fn sim_control(&self) -> Option<SimControl> {
        self.conn.as_ref().and_then(|c| c.sim.clone())
    }

    /// The adapter conversation of this session. VIN answers are hidden unless the user turned that off.
    pub fn session_log(&self) -> Vec<LogEntry> {
        self.conn.as_ref().map(|c| c.elm.log().to_vec()).unwrap_or_default()
    }

    // ── live data ───────────────────────────────────────────────────────────

    /// Parameters this vehicle supports (plus adapter voltage and derived values).
    pub fn live_params(&mut self) -> Result<Vec<LiveParam>> {
        let c = self.vehicle()?;
        let mut out: Vec<LiveParam> = pid::PIDS
            .iter()
            .filter(|p| c.ecu_supporting(p.pid).is_some())
            .map(|p| LiveParam {
                key: p.key.into(),
                name: p.name.into(),
                unit: p.unit.into(),
                min: p.min,
                max: p.max,
                category: format!("{:?}", p.category).to_lowercase(),
                common: p.common,
                source: "vehicle".into(),
            })
            .collect();
        if c.ecu_supporting(0x0B).is_some() && c.ecu_supporting(0x33).is_some() {
            out.push(LiveParam {
                key: "boost_derived".into(),
                name: "Boost pressure (manifold minus barometric, calculated)".into(),
                unit: "kPa".into(),
                min: -100.0,
                max: 255.0,
                category: "air".into(),
                common: false,
                source: "derived".into(),
            });
        }
        if c.elm.capabilities().voltage_reading == opendiag_core::Tri::Yes {
            out.push(LiveParam {
                key: "adapter_voltage".into(),
                name: "Battery voltage at the diagnostic socket (measured by the adapter)".into(),
                unit: "V".into(),
                min: 0.0,
                max: 20.0,
                category: "electrical".into(),
                common: true,
                source: "adapter".into(),
            });
        }
        Ok(out)
    }

    /// Reads the given live values once. Call it repeatedly for a live view;
    /// requests are spaced by [`MIN_REQUEST_GAP`] so the bus is not flooded.
    pub fn live_read(&mut self, keys: &[String]) -> Result<Vec<LiveSample>> {
        if keys.len() > MAX_LIVE_KEYS {
            return Err(EngineError::Invalid(format!("At most {MAX_LIVE_KEYS} values at a time.")));
        }
        let c = self.vehicle()?;
        if c.elm.consecutive_errors >= 3 {
            return Err(EngineError::Adapter(ElmError::Bus("several requests in a row failed; check the connection and reconnect".into())));
        }
        let mut out = Vec::new();
        for key in keys {
            let now = chrono::Utc::now().timestamp_millis();
            if key == "adapter_voltage" {
                let v = c.elm.read_voltage();
                out.push(LiveSample {
                    key: key.clone(),
                    name: "Battery voltage (adapter)".into(),
                    unit: "V".into(),
                    value: v,
                    ecu: None,
                    source: "adapter".into(),
                    error: v.is_none().then(|| "The adapter did not report a voltage.".to_string()),
                    at_ms: now,
                });
                continue;
            }
            if key == "boost_derived" {
                let map = read_pid(c, 0x0B);
                if c.baro.is_none() {
                    c.baro = read_pid(c, 0x33).ok().map(|m| m.value);
                }
                let (value, error) = match (map, c.baro) {
                    (Ok(m), Some(b)) => (Some(m.value - b), None),
                    (Err(e), _) => (None, Some(e)),
                    (_, None) => (None, Some("Barometric pressure not available.".into())),
                };
                out.push(LiveSample { key: key.clone(), name: "Boost pressure (calculated)".into(), unit: "kPa".into(), value, ecu: None, source: "derived".into(), error, at_ms: now });
                continue;
            }
            let Some(def) = pid::def_by_key(key) else {
                out.push(LiveSample { key: key.clone(), name: key.clone(), unit: String::new(), value: None, ecu: None, source: "vehicle".into(), error: Some("Unknown parameter.".into()), at_ms: now });
                continue;
            };
            match read_pid(c, def.pid) {
                Ok(m) => out.push(LiveSample { key: key.clone(), name: m.name, unit: m.unit, value: Some(m.value), ecu: Some(m.ecu), source: "vehicle".into(), error: None, at_ms: now }),
                Err(e) => out.push(LiveSample { key: key.clone(), name: def.name.into(), unit: def.unit.into(), value: None, ecu: None, source: "vehicle".into(), error: Some(e), at_ms: now }),
            }
        }
        Ok(out)
    }

    // ── expert console ──────────────────────────────────────────────────────

    /// Sends a raw request from the expert console. Only read-only services are
    /// accepted, whatever mode is on: expert mode does not bypass the safety policy.
    pub fn raw_request(&mut self, hex: &str) -> Result<Vec<EcuMessage>> {
        if self.setting_or("mode", "beginner") != "expert" {
            return Err(EngineError::Blocked("Turn on expert mode in Settings to use the console.".into()));
        }
        let bytes = opendiag_core::parse_hex(hex).ok_or_else(|| EngineError::Invalid("Enter the request as hex bytes, e.g. 01 0C.".into()))?;
        let class = opendiag_safety::classify(&bytes);
        if !class.raw_allowed {
            return Err(EngineError::Blocked(format!(
                "\"{}\" (risk level {}) cannot be sent from the console. Only read-only requests are allowed here.",
                class.what, class.level as u8
            )));
        }
        let c = self.vehicle()?;
        let msgs = c.req(&bytes)?;
        let session = c.session_id;
        self.store.log_operation(session, "raw_request", &serde_json::json!({ "request": opendiag_core::hex(&bytes), "what": class.what }))?;
        Ok(msgs)
    }

    // ── settings ────────────────────────────────────────────────────────────

    pub fn setting_or(&self, key: &str, default: &str) -> String {
        self.store.setting(key).ok().flatten().unwrap_or_else(|| default.to_string())
    }

    pub fn settings(&self) -> serde_json::Value {
        serde_json::json!({
            "mode": self.setting_or("mode", "beginner"),
            "units": self.setting_or("units", "metric"),
            "theme": self.setting_or("theme", "system"),
            "language": self.setting_or("language", "en"),
            "terms_accepted": self.terms_accepted(),
            "terms_version": TERMS_VERSION,
            "configuration_enabled": self.setting_or("configuration_enabled", "false") == "true",
            "show_vin_in_log": self.setting_or("show_vin_in_log", "false") == "true",
            "platform": self.platform,
        })
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<()> {
        let allowed: &[(&str, &[&str])] = &[
            ("mode", &["beginner", "expert"]),
            ("units", &["metric", "imperial"]),
            ("theme", &["system", "light", "dark"]),
            ("language", &["en"]),
            ("configuration_enabled", &["true", "false"]),
            ("show_vin_in_log", &["true", "false"]),
        ];
        let Some((_, values)) = allowed.iter().find(|(k, _)| *k == key) else {
            return Err(EngineError::Invalid(format!("Unknown setting {key}.")));
        };
        if !values.contains(&value) {
            return Err(EngineError::Invalid(format!("{key} cannot be {value}.")));
        }
        self.store.set_setting(key, value)?;
        if key == "show_vin_in_log" {
            if let Some(c) = self.conn.as_mut() {
                c.elm.redact_vin = value != "true";
            }
        }
        Ok(())
    }

    pub fn terms_accepted(&self) -> bool {
        self.setting_or("terms_accepted_version", "") == TERMS_VERSION
    }

    pub fn accept_terms(&mut self, accept: bool) -> Result<()> {
        self.store.set_setting("terms_accepted_version", if accept { TERMS_VERSION } else { "" })?;
        self.store.set_setting("terms_accepted_at", &opendiag_store::now())?;
        Ok(())
    }

    // ── reports and garage ──────────────────────────────────────────────────

    pub fn report(&self, id: i64) -> Result<report::ScanReport> {
        let v = self.store.report(id)?.ok_or_else(|| EngineError::Invalid("No such report.".into()))?;
        serde_json::from_value(v).map_err(|e| EngineError::Store(e.to_string()))
    }

    pub fn set_report_notes(&self, id: i64, notes: &str) -> Result<()> {
        let mut r = self.report(id)?;
        r.notes = notes.to_string();
        self.store.update_report(id, &serde_json::to_value(&r).unwrap_or_default())?;
        Ok(())
    }

    /// Exports a report. `include_vin = false` masks the VIN.
    /// Returns (file name, bytes).
    pub fn export_report(&self, id: i64, format: &str, include_vin: bool) -> Result<(String, Vec<u8>)> {
        let r = self.report(id)?;
        let r = if include_vin { r } else { r.redacted() };
        let base = format!("opendiag-report-{id}{}", if r.simulated { "-SIMULATED" } else { "" });
        Ok(match format {
            "json" => (format!("{base}.json"), serde_json::to_vec_pretty(&r).unwrap_or_default()),
            "csv" => (format!("{base}.csv"), report::to_csv(&r).into_bytes()),
            "html" => (format!("{base}.html"), report::to_html(&r).into_bytes()),
            "pdf" => (format!("{base}.pdf"), pdf::render(&report::to_text_lines(&r))),
            _ => return Err(EngineError::Invalid("Format must be pdf, json, csv or html.".into())),
        })
    }

    pub fn compare_reports(&self, older: i64, newer: i64) -> Result<report::Comparison> {
        Ok(report::compare(&self.report(older)?, &self.report(newer)?))
    }

    // ── knowledge ───────────────────────────────────────────────────────────

    pub fn procedures(&self) -> &[guided::Procedure] {
        &self.procedures
    }

    pub fn guided(&self, id: &str, answers: &[guided::Answer]) -> Result<guided::GuidedState> {
        let p = self.procedures.iter().find(|p| p.id == id).ok_or_else(|| EngineError::Invalid("No such procedure.".into()))?;
        Ok(guided::evaluate(p, answers))
    }

    pub fn definitions(&self) -> &[Definition] {
        &self.definitions
    }

    // ── compatibility ───────────────────────────────────────────────────────

    pub fn compat_status(&self) -> Result<CompatStatus> {
        let (simulated, adapter, firmware, vin_make) = match &self.conn {
            Some(c) => (
                c.simulated(),
                c.elm.capabilities().identity.clone(),
                c.elm.capabilities().stn_firmware.clone().unwrap_or_default(),
                c.vin.as_deref().map(opendiag_obd::vin::decode).and_then(|v| v.manufacturer),
            ),
            None => (false, String::new(), String::new(), None),
        };
        let mine = self.store.compat_records(&CompatFilter { adapter: Some(adapter.clone()).filter(|a| !a.is_empty()), os: Some(self.platform.clone()), ..Default::default() })?;
        let mine: Vec<_> = mine.into_iter().filter(|r| r.adapter_firmware == firmware).collect();
        let vehicle_records = match &vin_make {
            Some(m) => self.store.compat_records(&CompatFilter { make: Some(m.clone()), ..Default::default() })?.len(),
            None => 0,
        };
        let functions = catalog::FUNCTIONS
            .iter()
            .map(|f| {
                let status = if simulated && f.risk_level <= 1 && catalog::status_for(f, &self.platform) != SupportStatus::Planned && catalog::status_for(f, &self.platform) != SupportStatus::Unsupported && !f.id.starts_with("connect_") {
                    SupportStatus::SimulatedOnly
                } else {
                    catalog::status_for(f, &self.platform)
                };
                let local: Vec<&CompatRecord> = mine.iter().filter(|r| r.function == f.id).collect();
                FunctionStatus {
                    id: f.id.into(),
                    name: f.name.into(),
                    risk_level: f.risk_level,
                    status,
                    status_label: if simulated && status == SupportStatus::SimulatedOnly { "Simulated vehicle".into() } else { status.label().into() },
                    note: f.note.into(),
                    local_result: local.iter().max_by_key(|r| r.updated_at.clone()).map(|r| r.result),
                    best_verification: local.iter().map(|r| r.verification).max(),
                }
            })
            .collect();
        let summary = if simulated {
            "Simulated vehicle and adapter. Nothing shown here says anything about real hardware.".to_string()
        } else if self.conn.is_none() {
            "Not connected.".to_string()
        } else if mine.is_empty() {
            format!("No test results yet for this adapter on {}. Functions are implemented but not verified with this hardware.", self.platform)
        } else {
            format!("{} earlier result(s) for this adapter on {} (see each function).", mine.len(), self.platform)
        };
        Ok(CompatStatus {
            simulated,
            platform: self.platform.clone(),
            adapter,
            adapter_known_records: mine.len(),
            vehicle_known_records: vehicle_records,
            summary,
            functions,
        })
    }

    /// Records the result of running a function with the current hardware.
    pub(crate) fn record_result(&self, function: &str, result: TestResult, notes: &str) {
        let Some(c) = &self.conn else { return };
        let vin = c.vin.as_deref().map(opendiag_obd::vin::decode);
        let simulated = c.simulated();
        let rec = CompatRecord {
            uuid: String::new(),
            make: if simulated {
                "Simulated".into()
            } else {
                vin.as_ref().and_then(|v| v.manufacturer.clone()).unwrap_or_else(|| "Unknown".into())
            },
            model: if simulated { c.elm.link_info().name.trim_start_matches("Simulator: ").to_string() } else { String::new() },
            year: None,
            ecu_hw: String::new(),
            ecu_sw: c.calids.first().cloned().unwrap_or_default(),
            adapter: c.elm.capabilities().identity.clone(),
            adapter_firmware: c.elm.capabilities().stn_firmware.clone().unwrap_or_default(),
            os: self.platform.clone(),
            protocol: c.elm.protocol().map(|p| format!("{p:?}").to_lowercase()).unwrap_or_default(),
            function: function.into(),
            result,
            verification: if simulated { Verification::Simulated } else { Verification::UnverifiedLocal },
            app_version: opendiag_core::APP_VERSION.into(),
            evidence: vec![format!("session {}", c.session_id)],
            notes: notes.into(),
            source: "local".into(),
            version: 0,
            updated_at: String::new(),
        };
        let _ = self.store.record_compat(&rec, "recorded by the app");
    }
}

/// The connection, if the vehicle answers. A free function so callers can still
/// borrow the engine's other fields (store, library, policy) at the same time.
pub(crate) fn vehicle_of(conn: &mut Option<Conn>) -> Result<&mut Conn> {
    let c = conn.as_mut().ok_or(EngineError::NotConnected)?;
    if let Some(e) = &c.vehicle_error {
        return Err(EngineError::NoVehicle(e.clone()));
    }
    if c.ecus.is_empty() {
        return Err(EngineError::NoVehicle("no ECU answered".into()));
    }
    Ok(c)
}

/// Reads one PID from the first ECU that supports it.
pub(crate) fn read_pid(c: &mut Conn, p: u8) -> std::result::Result<Measurement, String> {
    let def = pid::def(p).ok_or("unknown PID")?;
    let ecu = c.ecu_supporting(p).ok_or_else(|| "Not supported by this vehicle.".to_string())?;
    let msgs = c.req(&[0x01, p]).map_err(|e| e.to_string())?;
    let m = msgs
        .iter()
        .find(|m| m.ecu == ecu && m.data.len() >= 2 && m.data[0] == 0x41 && m.data[1] == p)
        .or_else(|| msgs.iter().find(|m| m.data.len() >= 2 && m.data[0] == 0x41 && m.data[1] == p))
        .ok_or_else(|| "No valid answer.".to_string())?;
    let v = def.decode(&m.data[2..]).ok_or_else(|| "Answer too short.".to_string())?;
    Ok(Measurement { key: def.key.into(), name: def.name.into(), value: v, unit: def.unit.into(), ecu: m.ecu.clone(), source: "vehicle".into() })
}

/// Devices the wizard can offer: serial ports, the simulator profiles and a Wi-Fi entry.
pub fn devices() -> Vec<opendiag_transport::DeviceCandidate> {
    use opendiag_transport::DeviceCandidate;
    let mut out = opendiag_transport::discover_serial();
    out.push(DeviceCandidate {
        id: "tcp".into(),
        kind: opendiag_core::LinkKind::Wifi,
        name: "Wi-Fi or Ethernet adapter".into(),
        detail: format!("Enter the address (often {})", opendiag_transport::tcp::DEFAULT_WIFI_ADDRESS),
        address: format!("tcp:{}", opendiag_transport::tcp::DEFAULT_WIFI_ADDRESS),
        simulated: false,
    });
    for p in opendiag_sim::profiles::all() {
        out.push(DeviceCandidate {
            id: format!("sim:{}", p.id),
            kind: opendiag_core::LinkKind::Simulated,
            name: p.name.clone(),
            detail: p.description.clone(),
            address: format!("sim:{}", p.id),
            simulated: true,
        });
    }
    out
}

#[cfg(test)]
mod tests;
