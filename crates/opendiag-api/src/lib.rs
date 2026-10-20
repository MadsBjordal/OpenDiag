//! The one entry point the user interface talks to.
//!
//! The UI sends `{"cmd": "...", ...}` and gets `{"ok": true, "data": ...}` or
//! `{"ok": false, "error": "...", "kind": "..."}`. The Tauri app (Windows,
//! Linux, macOS, Android) and the development bridge (`opendiag-cli serve`)
//! both call [`Api::call`], so the UI runs the same code path everywhere.

use base64::Engine as _;
use opendiag_core::{ByteLink, KLinePort, LinkError, LinkKind};
use opendiag_engine::{Engine, EngineError, ScanProgress};
use opendiag_knowledge::guided::Answer;
use opendiag_sim::{SimAdapter, SimSettings};
use opendiag_store::compat::{CompatFilter, CompatRecord};
use opendiag_transport::DeviceCandidate;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::Duration;

/// Platform-specific links (Android USB host and Bluetooth). Desktop builds use [`NoExtraLinks`].
pub trait LinkFactory: Send + Sync {
    fn devices(&self) -> Vec<DeviceCandidate>;
    /// `None` when the address is not one of this factory's.
    fn open(&self, address: &str) -> Option<Result<Box<dyn ByteLink>, LinkError>>;
    /// A K-line cable at this address, when the platform has one (`None` = not mine).
    fn open_kline(&self, _address: &str) -> Option<Result<Box<dyn KLinePort>, LinkError>> {
        None
    }
}

pub struct NoExtraLinks;
impl LinkFactory for NoExtraLinks {
    fn devices(&self) -> Vec<DeviceCandidate> {
        Vec::new()
    }
    fn open(&self, _: &str) -> Option<Result<Box<dyn ByteLink>, LinkError>> {
        None
    }
}

/// A K-line port, its simulator control (if simulated) and the requested wake-up.
type OpenedKLine = (
    Box<dyn KLinePort>,
    Option<opendiag_sim::SimControl>,
    opendiag_kline::InitMode,
);

pub struct Api {
    engine: Mutex<Engine>,
    links: Box<dyn LinkFactory>,
    progress: Mutex<Option<ScanProgress>>,
    honda: Mutex<Option<opendiag_kline::honda::HondaKLine>>,
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Cmd {
    Info,
    Devices,
    Connect {
        address: String,
    },
    ConnectVehicle,
    Disconnect,
    Summary,
    Scan,
    Progress,
    LiveParams,
    LiveRead {
        keys: Vec<String>,
    },
    PrepareClear,
    ExecuteClear {
        plan_id: String,
        confirmed: bool,
    },
    Raw {
        hex: String,
    },
    Log,
    Settings,
    SetSetting {
        key: String,
        value: String,
    },
    AcceptTerms {
        accept: bool,
    },
    Reports {
        vehicle_id: Option<i64>,
    },
    Report {
        id: i64,
    },
    ReportNotes {
        id: i64,
        notes: String,
    },
    DeleteReport {
        id: i64,
    },
    ExportReport {
        id: i64,
        format: String,
        include_vin: bool,
    },
    Compare {
        older: i64,
        newer: i64,
    },
    Vehicles,
    UpdateVehicle {
        vehicle: opendiag_store::Vehicle,
    },
    DeleteVehicle {
        id: i64,
    },
    DeleteAllData,
    DtcExplain {
        code: String,
    },
    DtcSearch {
        q: String,
    },
    Procedures,
    Guided {
        id: String,
        answers: Vec<Answer>,
    },
    Definitions,
    Connectors {
        category: Option<opendiag_definitions::connectors::Category>,
        make: Option<String>,
    },
    BlinkTableGet {
        key: String,
    },
    BlinkTableSet {
        key: String,
        entries: Vec<BlinkEntry>,
    },
    ValidateDefinition {
        json: String,
    },
    Functions,
    CompatStatus,
    CompatQuery {
        filter: CompatFilter,
    },
    CompatHistory {
        uuid: String,
    },
    CompatSubmit {
        record: CompatRecord,
    },
    CompatSubmissions {
        status: Option<String>,
    },
    CompatModerate {
        id: i64,
        decision: String,
        note: String,
    },
    CompatExport,
    CompatImport {
        json: String,
        source: Option<String>,
    },
    SimGet,
    SimSet {
        settings: SimSettings,
    },
    SimProfiles,
    /// Experimental read-only Honda motorcycle K-line mode (expert mode only).
    HondaConnect {
        address: String,
    },
    HondaWake {
        variant: opendiag_kline::honda::Wake,
    },
    HondaReadTable {
        table: u8,
    },
    HondaLog,
    HondaDisconnect,
    /// A hardware-test result for one function with the current connection.
    RecordTest {
        function: String,
        result: opendiag_store::compat::TestResult,
        notes: String,
    },
}

/// One row of a blink-code table the user copied from their own service manual.
#[derive(serde::Serialize, Deserialize)]
struct BlinkEntry {
    code: String,
    meaning: String,
}

fn blink_key(key: &str) -> Result<String, EngineError> {
    let ok = !key.is_empty()
        && key.len() <= 80
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(format!("blink_table:{key}"))
    } else {
        Err(EngineError::Invalid("Bad table name.".into()))
    }
}

fn err_kind(e: &EngineError) -> &'static str {
    match e {
        EngineError::NotConnected => "not_connected",
        EngineError::NoVehicle(_) => "no_vehicle",
        EngineError::Adapter(_) => "adapter",
        EngineError::Store(_) => "store",
        EngineError::Invalid(_) => "invalid",
        EngineError::Blocked(_) => "blocked",
    }
}

fn file(name: String, bytes: Vec<u8>) -> Value {
    json!({ "filename": name, "base64": base64::engine::general_purpose::STANDARD.encode(bytes) })
}

impl Api {
    pub fn new(engine: Engine, links: Box<dyn LinkFactory>) -> Self {
        Self {
            engine: Mutex::new(engine),
            links,
            progress: Mutex::new(None),
            honda: Mutex::new(None),
        }
    }

    /// Opens a K-line cable. Addresses: `kline:<serial port or android-usb:…>[#auto|fast|5baud]`
    /// and `kline-sim:<profile>:<iso9141|kwp5baud|kwpfast|honda>[#…]` (simulator).
    fn open_kline(&self, address: &str) -> Result<OpenedKLine, EngineError> {
        let bad = |e: LinkError| EngineError::Invalid(e.to_string());
        let (addr, init) = match address.split_once('#') {
            Some((a, i)) => (
                a,
                opendiag_kline::InitMode::parse(i)
                    .ok_or_else(|| EngineError::Invalid(format!("Unknown K-line init \"{i}\".")))?,
            ),
            None => (address, opendiag_kline::InitMode::Auto),
        };
        if let Some(rest) = addr.strip_prefix("kline-sim:") {
            use opendiag_sim::kline::{SimKLineMode, SimKLinePort};
            let (profile, mode) = rest.split_once(':').unwrap_or((rest, "iso9141"));
            let profile = opendiag_sim::profiles::by_id(profile)
                .ok_or_else(|| EngineError::Invalid(format!("No simulator profile {profile}.")))?;
            let mode = match mode {
                "iso9141" => SimKLineMode::Iso9141,
                "kwp5baud" => SimKLineMode::Kwp5Baud,
                "kwpfast" => SimKLineMode::KwpFast,
                "honda" => SimKLineMode::Honda,
                m => {
                    return Err(EngineError::Invalid(format!(
                        "Unknown simulated K-line mode {m}."
                    )))
                }
            };
            let (port, ctl) = SimKLinePort::new(profile, SimSettings::default(), mode);
            return Ok((Box::new(port), Some(ctl), init));
        }
        let inner = addr
            .strip_prefix("kline:")
            .ok_or_else(|| EngineError::Invalid(format!("Not a K-line address: {address}")))?;
        if let Some(r) = self.links.open_kline(inner) {
            return r.map(|p| (p, None, init)).map_err(bad);
        }
        #[cfg(feature = "serial")]
        {
            let port = opendiag_transport::serial::SerialKLine::open(inner).map_err(bad)?;
            return Ok((Box::new(port), None, init));
        }
        #[allow(unreachable_code)]
        Err(EngineError::Invalid(
            "K-line cables are not available on this platform.".into(),
        ))
    }

    fn require_expert(&self) -> Result<(), EngineError> {
        if self.engine().setting_or("mode", "beginner") != "expert" {
            return Err(EngineError::Blocked(
                "Turn on expert mode in Settings to use the experimental Honda mode.".into(),
            ));
        }
        Ok(())
    }

    /// Runs one command. `emit` receives events (e.g. "scan_progress") while it runs.
    pub fn call(&self, request: Value, emit: &dyn Fn(&str, Value)) -> Value {
        let cmd: Cmd = match serde_json::from_value(request) {
            Ok(c) => c,
            Err(e) => {
                return json!({ "ok": false, "kind": "invalid", "error": format!("Bad request: {e}") })
            }
        };
        match self.run(cmd, emit) {
            Ok(v) => json!({ "ok": true, "data": v }),
            Err(e) => json!({ "ok": false, "kind": err_kind(&e), "error": e.to_string() }),
        }
    }

    fn engine(&self) -> std::sync::MutexGuard<'_, Engine> {
        // A panic in one command must not lock the app forever.
        self.engine.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn open_link(
        &self,
        address: &str,
    ) -> Result<(Box<dyn ByteLink>, Option<opendiag_sim::SimControl>), EngineError> {
        let bad = |e: LinkError| EngineError::Invalid(e.to_string());
        if let Some(id) = address.strip_prefix("sim:") {
            let profile = opendiag_sim::profiles::by_id(id)
                .ok_or_else(|| EngineError::Invalid(format!("No simulator profile {id}.")))?;
            let (link, ctl) = SimAdapter::new(profile, SimSettings::default());
            return Ok((Box::new(link), Some(ctl)));
        }
        if let Some(r) = self.links.open(address) {
            return r.map(|l| (l, None)).map_err(bad);
        }
        if let Some(rest) = address
            .strip_prefix("tcp:")
            .map(|r| (r, LinkKind::Wifi))
            .or_else(|| {
                address
                    .strip_prefix("eth:")
                    .map(|r| (r, LinkKind::Ethernet))
            })
        {
            let link =
                opendiag_transport::tcp::TcpLink::connect(rest.0, rest.1, Duration::from_secs(5))
                    .map_err(bad)?;
            return Ok((Box::new(link), None));
        }
        #[cfg(feature = "serial")]
        if let Some(rest) = address.strip_prefix("serial:") {
            use opendiag_transport::serial::SerialLink;
            let (path, baud) = match rest.rsplit_once('@') {
                Some((p, b)) => (p, b.parse::<u32>().ok()),
                None => (rest, None),
            };
            let kind = opendiag_transport::discover_serial()
                .into_iter()
                .find(|d| d.name == path)
                .map(|d| d.kind)
                .unwrap_or(LinkKind::UsbSerial);
            let link = match baud {
                Some(b) => SerialLink::open(path, b, kind).map_err(bad)?,
                None => SerialLink::open_auto(path, kind).map_err(bad)?.0,
            };
            return Ok((Box::new(link), None));
        }
        Err(EngineError::Invalid(format!(
            "Unknown connection address \"{address}\"."
        )))
    }

    /// Records the Honda wake-up result in the compatibility database (this OS, this cable).
    fn record_honda(&self, awake: bool, simulated: bool, notes: &str) {
        use opendiag_store::compat::{CompatRecord, TestResult, Verification};
        let e = self.engine();
        let rec = CompatRecord {
            uuid: String::new(),
            make: if simulated {
                "Simulated".into()
            } else {
                "Honda".into()
            },
            model: String::new(),
            year: None,
            ecu_hw: String::new(),
            ecu_sw: String::new(),
            adapter: "Raw K-line cable".into(),
            adapter_firmware: String::new(),
            os: e.platform().into(),
            protocol: "honda_kline".into(),
            function: "honda_kline_wake".into(),
            result: if awake {
                TestResult::Works
            } else {
                TestResult::Fails
            },
            verification: if simulated {
                Verification::Simulated
            } else {
                Verification::UnverifiedLocal
            },
            app_version: opendiag_core::APP_VERSION.into(),
            evidence: vec![notes.into()],
            notes: String::new(),
            source: "local".into(),
            version: 0,
            updated_at: String::new(),
        };
        let _ = e.store().record_compat(&rec, "recorded by the app");
    }

    fn run(&self, cmd: Cmd, emit: &dyn Fn(&str, Value)) -> Result<Value, EngineError> {
        let to =
            |v: Result<Value, serde_json::Error>| v.map_err(|e| EngineError::Store(e.to_string()));
        Ok(match cmd {
            Cmd::Info => {
                let e = self.engine();
                json!({
                    "app": opendiag_core::APP_NAME,
                    "version": opendiag_core::APP_VERSION,
                    "platform": e.platform(),
                    "connected": e.is_connected(),
                    "settings": e.settings(),
                    "dtc_library_size": e.library().len(),
                })
            }
            Cmd::Devices => {
                let mut d = self.links.devices();
                d.extend(opendiag_engine::devices());
                to(serde_json::to_value(d))?
            }
            Cmd::Connect { address } if address.starts_with("kline") => {
                let (port, sim, init) = self.open_kline(&address)?;
                let drv = opendiag_kline::KLineObd::open(port, init)?;
                to(serde_json::to_value(
                    self.engine().connect_driver(Box::new(drv), sim)?,
                ))?
            }
            Cmd::Connect { address } => {
                let (link, sim) = self.open_link(&address)?;
                to(serde_json::to_value(self.engine().connect(link, sim)?))?
            }
            Cmd::ConnectVehicle => to(serde_json::to_value(self.engine().connect_vehicle()?))?,
            Cmd::Disconnect => {
                self.engine().disconnect();
                Value::Null
            }
            Cmd::Summary => to(serde_json::to_value(self.engine().summary()?))?,
            Cmd::Scan => {
                let mut e = self.engine();
                let progress = &self.progress;
                let r = e.scan(&mut |p| {
                    emit(
                        "scan_progress",
                        serde_json::to_value(&p).unwrap_or_default(),
                    );
                    *progress.lock().unwrap_or_else(|x| x.into_inner()) = Some(p);
                });
                *self.progress.lock().unwrap_or_else(|x| x.into_inner()) = None;
                to(serde_json::to_value(r?))?
            }
            Cmd::Progress => to(serde_json::to_value(
                self.progress
                    .lock()
                    .unwrap_or_else(|x| x.into_inner())
                    .clone(),
            ))?,
            Cmd::LiveParams => to(serde_json::to_value(self.engine().live_params()?))?,
            Cmd::LiveRead { keys } => to(serde_json::to_value(self.engine().live_read(&keys)?))?,
            Cmd::PrepareClear => to(serde_json::to_value(self.engine().prepare_clear()?))?,
            Cmd::ExecuteClear { plan_id, confirmed } => to(serde_json::to_value(
                self.engine().execute_clear(&plan_id, confirmed)?,
            ))?,
            Cmd::Raw { hex } => to(serde_json::to_value(self.engine().raw_request(&hex)?))?,
            Cmd::Log => to(serde_json::to_value(self.engine().session_log()))?,
            Cmd::Settings => self.engine().settings(),
            Cmd::SetSetting { key, value } => {
                let mut e = self.engine();
                e.set_setting(&key, &value)?;
                e.settings()
            }
            Cmd::AcceptTerms { accept } => {
                let mut e = self.engine();
                e.accept_terms(accept)?;
                e.settings()
            }
            Cmd::Reports { vehicle_id } => to(serde_json::to_value(
                self.engine().store().reports(vehicle_id)?,
            ))?,
            Cmd::Report { id } => to(serde_json::to_value(self.engine().report(id)?))?,
            Cmd::ReportNotes { id, notes } => {
                self.engine().set_report_notes(id, &notes)?;
                Value::Null
            }
            Cmd::DeleteReport { id } => {
                self.engine().store().delete_report(id)?;
                Value::Null
            }
            Cmd::ExportReport {
                id,
                format,
                include_vin,
            } => {
                let (name, bytes) = self.engine().export_report(id, &format, include_vin)?;
                file(name, bytes)
            }
            Cmd::Compare { older, newer } => to(serde_json::to_value(
                self.engine().compare_reports(older, newer)?,
            ))?,
            Cmd::Vehicles => to(serde_json::to_value(self.engine().store().vehicles()?))?,
            Cmd::UpdateVehicle { vehicle } => {
                self.engine().store().update_vehicle(&vehicle)?;
                Value::Null
            }
            Cmd::DeleteVehicle { id } => {
                self.engine().store().delete_vehicle(id)?;
                Value::Null
            }
            Cmd::DeleteAllData => {
                self.engine().store().delete_all_vehicle_data()?;
                Value::Null
            }
            Cmd::DtcExplain { code } => {
                to(serde_json::to_value(self.engine().library().explain(&code)))?
            }
            Cmd::DtcSearch { q } => to(serde_json::to_value(self.engine().library().search(&q)))?,
            Cmd::Procedures => to(serde_json::to_value(self.engine().procedures()))?,
            Cmd::Guided { id, answers } => {
                to(serde_json::to_value(self.engine().guided(&id, &answers)?))?
            }
            Cmd::Definitions => to(serde_json::to_value(self.engine().definitions()))?,
            Cmd::Connectors { category, make } => {
                let all = opendiag_definitions::connectors::bundled();
                to(serde_json::to_value(
                    opendiag_definitions::connectors::find(&all, category, make.as_deref()),
                ))?
            }
            Cmd::BlinkTableGet { key } => {
                let k = blink_key(&key)?;
                match self.engine().store().setting(&k)? {
                    Some(s) => serde_json::from_str(&s).unwrap_or(Value::Array(vec![])),
                    None => Value::Array(vec![]),
                }
            }
            Cmd::BlinkTableSet { key, entries } => {
                let k = blink_key(&key)?;
                if entries.len() > 500
                    || entries
                        .iter()
                        .any(|e| e.code.len() > 20 || e.meaning.len() > 500)
                {
                    return Err(EngineError::Invalid("Table too large.".into()));
                }
                self.engine()
                    .store()
                    .set_setting(&k, &serde_json::to_string(&entries).unwrap_or_default())?;
                Value::Null
            }
            Cmd::ValidateDefinition { json } => match opendiag_definitions::parse(&json) {
                Ok(d) => json!({ "valid": true, "id": d.id, "name": d.name, "errors": [] }),
                Err(errs) => json!({ "valid": false, "errors": errs }),
            },
            Cmd::Functions => {
                let e = self.engine();
                let p = e.platform().to_string();
                Value::Array(
                    opendiag_engine::catalog::FUNCTIONS
                        .iter()
                        .map(|f| {
                            let s = opendiag_engine::catalog::status_for(f, &p);
                            json!({ "id": f.id, "name": f.name, "risk_level": f.risk_level, "status": s, "status_label": s.label(), "windows": f.windows, "android": f.android, "linux": f.linux, "note": f.note })
                        })
                        .collect(),
                )
            }
            Cmd::CompatStatus => to(serde_json::to_value(self.engine().compat_status()?))?,
            Cmd::CompatQuery { filter } => to(serde_json::to_value(
                self.engine().store().compat_records(&filter)?,
            ))?,
            Cmd::CompatHistory { uuid } => to(serde_json::to_value(
                self.engine().store().compat_history(&uuid)?,
            ))?,
            Cmd::CompatSubmit { record } => {
                json!({ "id": self.engine().store().submit_compat(&record)? })
            }
            Cmd::CompatSubmissions { status } => to(serde_json::to_value(
                self.engine().store().submissions(status.as_deref())?,
            ))?,
            Cmd::CompatModerate { id, decision, note } => to(serde_json::to_value(
                self.engine().store().moderate(id, &decision, &note)?,
            ))?,
            Cmd::CompatExport => {
                let json = self.engine().store().export_snapshot()?;
                file("opendiag-compat-snapshot.json".into(), json.into_bytes())
            }
            Cmd::CompatImport { json, source } => {
                json!({ "imported": self.engine().store().import_snapshot(&json, source.as_deref().unwrap_or("file"))? })
            }
            Cmd::SimGet => match self.engine().sim_control() {
                Some(c) => to(serde_json::to_value(c.settings()))?,
                None => Value::Null,
            },
            Cmd::SimSet { settings } => match self.engine().sim_control() {
                Some(c) => {
                    c.set(settings);
                    to(serde_json::to_value(c.settings()))?
                }
                None => {
                    return Err(EngineError::Invalid(
                        "Not connected to the simulator.".into(),
                    ))
                }
            },
            Cmd::SimProfiles => to(serde_json::to_value(opendiag_sim::profiles::all()))?,
            Cmd::HondaConnect { address } => {
                self.require_expert()?;
                let (port, _, _) = self.open_kline(&address)?;
                let h = opendiag_kline::honda::HondaKLine::open(port)?;
                let info = h.info();
                *self.honda.lock().unwrap_or_else(|p| p.into_inner()) = Some(h);
                to(serde_json::to_value(info))?
            }
            Cmd::HondaWake { variant } => {
                self.require_expert()?;
                let mut g = self.honda.lock().unwrap_or_else(|p| p.into_inner());
                let h = g.as_mut().ok_or(EngineError::NotConnected)?;
                let r = h.wake(variant)?;
                let simulated = h.info().simulated;
                drop(g);
                self.record_honda(
                    r.awake,
                    simulated,
                    &format!("wake {variant:?}: {}", r.init_reply),
                );
                to(serde_json::to_value(r))?
            }
            Cmd::HondaReadTable { table } => {
                self.require_expert()?;
                let mut g = self.honda.lock().unwrap_or_else(|p| p.into_inner());
                let h = g.as_mut().ok_or(EngineError::NotConnected)?;
                let data = h.read_table(table)?;
                json!({ "table": table, "hex": opendiag_core::hex(&data), "bytes": data, "simulated": h.info().simulated })
            }
            Cmd::HondaLog => match self
                .honda
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
            {
                Some(h) => to(serde_json::to_value(h.log()))?,
                None => Value::Array(vec![]),
            },
            Cmd::RecordTest {
                function,
                result,
                notes,
            } => {
                if !opendiag_engine::catalog::FUNCTIONS
                    .iter()
                    .any(|f| f.id == function)
                {
                    return Err(EngineError::Invalid(format!(
                        "Unknown function {function}."
                    )));
                }
                let e = self.engine();
                if !e.is_connected() {
                    return Err(EngineError::NotConnected);
                }
                e.record_result(
                    &function,
                    result,
                    &notes.chars().take(2000).collect::<String>(),
                );
                to(serde_json::to_value(e.compat_status()?))?
            }
            Cmd::HondaDisconnect => {
                if let Some(mut h) = self.honda.lock().unwrap_or_else(|p| p.into_inner()).take() {
                    h.close();
                }
                Value::Null
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api() -> Api {
        Api::new(
            Engine::new(opendiag_store::Store::open_in_memory().unwrap(), "linux"),
            Box::new(NoExtraLinks),
        )
    }

    fn ok(a: &Api, v: Value) -> Value {
        let r = a.call(v.clone(), &|_, _| {});
        assert_eq!(r["ok"], true, "{v} → {r}");
        r["data"].clone()
    }

    #[test]
    fn whole_workflow_through_json() {
        let a = api();
        assert_eq!(ok(&a, json!({"cmd":"info"}))["app"], "OpenDiag");
        assert!(ok(&a, json!({"cmd":"devices"}))
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["address"] == "sim:sim-car"));
        let r = a.call(json!({"cmd":"scan"}), &|_, _| {});
        assert_eq!(r["kind"], "not_connected");
        let s = ok(&a, json!({"cmd":"connect","address":"sim:sim-car"}));
        assert_eq!(s["simulated"], true);
        let events = std::cell::RefCell::new(0);
        let r = a.call(json!({"cmd":"scan"}), &|name, _| {
            assert_eq!(name, "scan_progress");
            *events.borrow_mut() += 1;
        });
        assert_eq!(r["ok"], true);
        assert_eq!(*events.borrow(), 8);
        let id = r["data"]["id"].as_str().unwrap().parse::<i64>().unwrap();
        let f = ok(
            &a,
            json!({"cmd":"export_report","id":id,"format":"pdf","include_vin":false}),
        );
        assert!(f["filename"].as_str().unwrap().ends_with(".pdf"));
        assert!(ok(&a, json!({"cmd":"reports"})).as_array().unwrap().len() == 1);
        let live = ok(&a, json!({"cmd":"live_read","keys":["rpm"]}));
        assert!(live[0]["value"].as_f64().unwrap() > 0.0);
        let r = a.call(json!({"cmd":"raw","hex":"04"}), &|_, _| {});
        assert_eq!(r["kind"], "blocked");
        let plan = ok(&a, json!({"cmd":"prepare_clear"}));
        let res = ok(
            &a,
            json!({"cmd":"execute_clear","plan_id":plan["id"],"confirmed":true}),
        );
        assert_eq!(res["performed"], false);
        let mut sim = ok(&a, json!({"cmd":"sim_get"}));
        sim["engine_running"] = json!(false);
        ok(&a, json!({"cmd":"sim_set","settings":sim}));
        ok(&a, json!({"cmd":"accept_terms","accept":true}));
        let plan = ok(&a, json!({"cmd":"prepare_clear"}));
        let res = ok(
            &a,
            json!({"cmd":"execute_clear","plan_id":plan["id"],"confirmed":true}),
        );
        assert_eq!(res["performed"], true);
        assert!(
            ok(&a, json!({"cmd":"compat_query","filter":{}}))
                .as_array()
                .unwrap()
                .len()
                >= 3
        );
        ok(&a, json!({"cmd":"disconnect"}));
    }

    #[test]
    fn k_line_and_honda_through_json() {
        let a = api();
        let s = ok(
            &a,
            json!({"cmd":"connect","address":"kline-sim:sim-legacy:kwpfast#fast"}),
        );
        assert_eq!(s["vehicle_connected"], true);
        assert_eq!(s["protocol"], "iso14230_fast");
        assert_eq!(ok(&a, json!({"cmd":"scan"}))["dtcs"][0]["code"], "P0420");
        ok(&a, json!({"cmd":"disconnect"}));
        // Honda mode is expert-only.
        let r = a.call(
            json!({"cmd":"honda_connect","address":"kline-sim:sim-motorcycle:honda"}),
            &|_, _| {},
        );
        assert_eq!(r["kind"], "blocked");
        ok(
            &a,
            json!({"cmd":"set_setting","key":"mode","value":"expert"}),
        );
        ok(
            &a,
            json!({"cmd":"honda_connect","address":"kline-sim:sim-motorcycle:honda"}),
        );
        assert_eq!(
            ok(&a, json!({"cmd":"honda_wake","variant":"ping72"}))["awake"],
            true
        );
        let t = ok(&a, json!({"cmd":"honda_read_table","table":17}));
        assert_eq!(t["bytes"].as_array().unwrap().len(), 16);
        assert!(ok(&a, json!({"cmd":"honda_log"})).as_array().unwrap().len() > 3);
        ok(&a, json!({"cmd":"honda_disconnect"}));
        let recs = ok(
            &a,
            json!({"cmd":"compat_query","filter":{"function":"honda_kline_wake"}}),
        );
        assert_eq!(recs[0]["verification"], "simulated");
        assert_eq!(
            a.call(
                json!({"cmd":"connect","address":"kline-sim:sim-legacy:nope"}),
                &|_, _| {}
            )["kind"],
            "invalid"
        );
    }

    #[test]
    fn hardware_test_results_are_recorded_per_os() {
        let a = api();
        assert_eq!(
            a.call(
                json!({"cmd":"record_test","function":"read_dtcs","result":"works","notes":""}),
                &|_, _| {}
            )["kind"],
            "not_connected"
        );
        ok(&a, json!({"cmd":"connect","address":"sim:sim-car"}));
        let s = ok(
            &a,
            json!({"cmd":"record_test","function":"live_data","result":"partial","notes":"rpm froze once"}),
        );
        let f = s["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["id"] == "live_data")
            .unwrap()
            .clone();
        assert_eq!(f["local_result"], "partial");
        // Simulated sessions can never produce a hardware verification.
        assert_eq!(f["best_verification"], "simulated");
        assert_eq!(
            a.call(
                json!({"cmd":"record_test","function":"fly","result":"works","notes":""}),
                &|_, _| {}
            )["kind"],
            "invalid"
        );
    }

    #[test]
    fn bad_requests() {
        let a = api();
        assert_eq!(a.call(json!({"cmd":"fly"}), &|_, _| {})["kind"], "invalid");
        assert_eq!(
            a.call(json!({"cmd":"connect","address":"floppy:A"}), &|_, _| {})["kind"],
            "invalid"
        );
        assert_eq!(
            a.call(
                json!({"cmd":"set_setting","key":"mode","value":"x"}),
                &|_, _| {}
            )["kind"],
            "invalid"
        );
        let c = ok(
            &a,
            json!({"cmd":"connectors","category":"car","make":"Honda"}),
        );
        assert!(!c.as_array().unwrap().is_empty());
        ok(
            &a,
            json!({"cmd":"blink_table_set","key":"honda-test","entries":[{"code":"12","meaning":"from my manual"}]}),
        );
        assert_eq!(
            ok(&a, json!({"cmd":"blink_table_get","key":"honda-test"}))[0]["code"],
            "12"
        );
        assert_eq!(
            a.call(json!({"cmd":"blink_table_get","key":"../x"}), &|_, _| {})["kind"],
            "invalid"
        );
        let v = ok(&a, json!({"cmd":"validate_definition","json":"{}"}));
        assert_eq!(v["valid"], false);
    }
}
