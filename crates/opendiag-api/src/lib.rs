//! The one entry point the user interface talks to.
//!
//! The UI sends `{"cmd": "...", ...}` and gets `{"ok": true, "data": ...}` or
//! `{"ok": false, "error": "...", "kind": "..."}`. The Tauri app (Windows,
//! Linux, macOS, Android) and the development bridge (`opendiag-cli serve`)
//! both call [`Api::call`], so the UI runs the same code path everywhere.

use base64::Engine as _;
use opendiag_core::{ByteLink, LinkError, LinkKind};
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

pub struct Api {
    engine: Mutex<Engine>,
    links: Box<dyn LinkFactory>,
    progress: Mutex<Option<ScanProgress>>,
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
        }
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
        let v = ok(&a, json!({"cmd":"validate_definition","json":"{}"}));
        assert_eq!(v["valid"], false);
    }
}
