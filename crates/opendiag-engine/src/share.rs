//! Anonymous result sharing (opt-in). Builds the exact data a user can choose
//! to send so others learn which vehicles and adapters work and which fault
//! codes occur. Never contains the full VIN, names, notes, location or times
//! more precise than the app version. Sending happens in the app shell, after
//! the user has seen this data; the engine itself never goes online.

use crate::report::{DtcKind, ModuleStatus, ScanReport};
use crate::{Engine, EngineError, Result};
use opendiag_store::compat::TestResult;
use serde::{Deserialize, Serialize};

pub const SHARE_SCHEMA: &str = "opendiag-share/1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareVehicle {
    pub make: String,
    /// As typed in My Garage (empty when unknown).
    pub model: String,
    pub year: Option<u16>,
    /// The manufacturer code: the first 3 characters of the VIN (not the vehicle's identity).
    pub wmi: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareAdapter {
    pub identity: String,
    pub firmware: Option<String>,
    pub family: String,
    pub clone_suspected: bool,
    pub link: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareEcu {
    pub label: String,
    pub calibration_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareResult {
    pub function: String,
    pub result: TestResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareDtc {
    pub code: String,
    pub kind: DtcKind,
    pub ecu: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareValue {
    pub key: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareFreezeFrame {
    pub dtc: Option<String>,
    pub values: Vec<ShareValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareMonitor {
    pub name: String,
    pub complete: bool,
}

/// One shared result. The receiving service accepts exactly these fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Share {
    pub schema: String,
    pub app_version: String,
    pub os: String,
    pub vehicle: ShareVehicle,
    pub adapter: ShareAdapter,
    pub protocol: Option<String>,
    pub ecus: Vec<ShareEcu>,
    pub results: Vec<ShareResult>,
    pub dtcs: Vec<ShareDtc>,
    pub freeze_frames: Vec<ShareFreezeFrame>,
    pub mil_on: Option<bool>,
    pub monitors: Vec<ShareMonitor>,
    /// The user said the vehicle is modified (tune, replacement ECU, engine
    /// changes): kept apart from standard vehicles in the community data.
    pub modified: bool,
}

fn enum_str<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn ok(b: bool) -> TestResult {
    if b {
        TestResult::Works
    } else {
        TestResult::Fails
    }
}

/// What the scan itself shows about each function.
fn results_from(r: &ScanReport) -> Vec<ShareResult> {
    let mut out = vec![];
    let connect = match enum_str(&r.link.kind).as_str() {
        "usb_serial" | "android_usb" => Some("connect_usb_serial"),
        "bluetooth_classic" => Some("connect_bluetooth_classic"),
        "wifi" | "ethernet" => Some("connect_wifi"),
        "k_line_cable" => Some("connect_kline_raw"),
        _ => None,
    };
    if let Some(f) = connect {
        out.push(ShareResult {
            function: f.into(),
            result: TestResult::Works,
        });
    }
    out.push(ShareResult {
        function: "adapter_probe".into(),
        result: TestResult::Works,
    });
    let scanned = r
        .modules
        .iter()
        .any(|m| m.status != ModuleStatus::NotScanned);
    out.push(ShareResult {
        function: "read_dtcs".into(),
        result: ok(scanned),
    });
    out.push(ShareResult {
        function: "read_readiness".into(),
        result: if r.readiness.is_empty() {
            TestResult::NotSupported
        } else {
            TestResult::Works
        },
    });
    if !r.dtcs.is_empty() {
        out.push(ShareResult {
            function: "read_freeze_frame".into(),
            result: if r.freeze_frames.is_empty() {
                TestResult::Partial
            } else {
                TestResult::Works
            },
        });
    }
    out.push(ShareResult {
        function: "read_vehicle_info".into(),
        result: ok(r.vehicle.vin.is_some()),
    });
    if r.measurements.iter().any(|m| m.source == "vehicle") {
        out.push(ShareResult {
            function: "live_data".into(),
            result: TestResult::Works,
        });
    }
    out
}

impl Engine {
    /// The data that would be shared for a saved scan report. `tests` are
    /// results from the hardware test page; they replace what the scan shows
    /// for the same function. `modified` is the user's answer to "is this
    /// vehicle modified?". Simulated reports cannot be shared.
    pub fn share_preview(
        &self,
        report_id: i64,
        tests: &[ShareResult],
        modified: bool,
    ) -> Result<Share> {
        let r = self.report(report_id)?;
        if r.simulated {
            return Err(EngineError::Invalid(
                "Results from the simulator are not shared: they say nothing about real vehicles."
                    .into(),
            ));
        }
        let garage = self
            .store
            .reports(None)?
            .into_iter()
            .find(|s| s.id == report_id)
            .and_then(|s| s.vehicle_id)
            .and_then(|id| self.store.vehicles().ok()?.into_iter().find(|v| v.id == id));
        let vin = r.vehicle.vin.as_ref();
        let make = garage
            .as_ref()
            .and_then(|g| g.make.clone())
            .filter(|m| !m.trim().is_empty())
            .or_else(|| vin.and_then(|v| v.manufacturer.clone()))
            .unwrap_or_default();
        let year = garage.as_ref().and_then(|g| g.year).or_else(|| {
            match vin.map(|v| v.model_years.as_slice()) {
                Some([y]) => Some(*y),
                _ => None,
            }
        });
        let vehicle = ShareVehicle {
            make: make.trim().chars().take(40).collect(),
            model: garage
                .as_ref()
                .and_then(|g| g.model.clone())
                .unwrap_or_default()
                .trim()
                .chars()
                .take(40)
                .collect(),
            year,
            wmi: vin.map(|v| v.wmi.clone()).filter(|w| w.len() == 3),
            kind: garage.as_ref().and_then(|g| g.kind.clone()),
        };
        let mut results = results_from(&r);
        for t in tests {
            results.retain(|x| x.function != t.function);
            results.push(t.clone());
        }
        let label = |ecu: &str| opendiag_obd::ecu_label(ecu);
        Ok(Share {
            schema: SHARE_SCHEMA.into(),
            app_version: r.app_version.clone(),
            os: r.platform.clone(),
            vehicle,
            adapter: ShareAdapter {
                identity: r.adapter.identity.chars().take(60).collect(),
                firmware: r.adapter.stn_firmware.clone(),
                family: enum_str(&r.adapter.family),
                clone_suspected: r.adapter.clone_suspected,
                link: enum_str(&r.link.kind),
            },
            protocol: r.protocol.map(|p| p.label().to_string()),
            ecus: r
                .vehicle
                .calibration_ids
                .iter()
                .map(|(ecu, cal)| ShareEcu {
                    label: label(ecu),
                    calibration_id: Some(cal.chars().take(20).collect()),
                })
                .collect(),
            results,
            dtcs: r
                .dtcs
                .iter()
                .map(|d| ShareDtc {
                    code: d.code.clone(),
                    kind: d.kind,
                    ecu: d.ecu_label.clone(),
                })
                .collect(),
            freeze_frames: r
                .freeze_frames
                .iter()
                .map(|f| ShareFreezeFrame {
                    dtc: f.dtc.clone(),
                    values: f
                        .values
                        .iter()
                        .filter(|m| m.source == "vehicle")
                        .map(|m| ShareValue {
                            key: m.key.clone(),
                            value: m.value,
                            unit: m.unit.clone(),
                        })
                        .collect(),
                })
                .collect(),
            mil_on: r.readiness.first().map(|(_, s)| s.mil_on),
            monitors: r
                .readiness
                .first()
                .map(|(_, s)| {
                    s.monitors
                        .iter()
                        .filter(|m| m.available)
                        .map(|m| ShareMonitor {
                            name: m.name.clone(),
                            complete: m.complete,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            modified,
        })
    }

    /// Remembers a sent share so the user can delete it later (id + delete token, kept on this device only).
    pub fn remember_share(&self, id: &str, delete_token: &str, report_id: i64) -> Result<()> {
        let mut list = self.shares();
        list.push(serde_json::json!({
            "id": id, "delete_token": delete_token, "report_id": report_id, "sent_at": opendiag_store::now(),
        }));
        self.store.set_setting(
            "shared_results",
            &serde_json::to_string(&list).unwrap_or_default(),
        )?;
        Ok(())
    }

    pub fn shares(&self) -> Vec<serde_json::Value> {
        serde_json::from_str(&self.setting_or("shared_results", "[]")).unwrap_or_default()
    }

    pub fn forget_share(&self, id: &str) -> Result<()> {
        let list: Vec<_> = self
            .shares()
            .into_iter()
            .filter(|s| s["id"] != id)
            .collect();
        self.store.set_setting(
            "shared_results",
            &serde_json::to_string(&list).unwrap_or_default(),
        )?;
        Ok(())
    }
}

impl Engine {
    /// Saves downloaded community summaries for offline use: the compatibility
    /// snapshot goes into the compatibility database (as "community reported",
    /// never verified); the fault code counts are kept as they came.
    pub fn import_community(&self, snapshot: &str, codes: &serde_json::Value) -> Result<usize> {
        let n = self.store.import_snapshot(snapshot, "community")?;
        let codes: Vec<serde_json::Value> = codes
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|c| c["code"].is_string() && c["reports"].is_u64())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        self.store.set_setting(
            "community_codes",
            &serde_json::to_string(&codes).unwrap_or_default(),
        )?;
        self.store
            .set_setting("community_updated", &opendiag_store::now())?;
        Ok(n)
    }

    /// How many shared reports mention each code for this make (and model and year when given).
    pub fn community_codes(
        &self,
        make: &str,
        model: Option<&str>,
        year: Option<u16>,
    ) -> Vec<serde_json::Value> {
        let all: Vec<serde_json::Value> =
            serde_json::from_str(&self.setting_or("community_codes", "[]")).unwrap_or_default();
        let eq = |a: &serde_json::Value, b: &str| {
            a.as_str()
                .map(|x| x.trim().eq_ignore_ascii_case(b.trim()))
                .unwrap_or(false)
        };
        all.into_iter()
            // Counts from modified vehicles never describe standard ones.
            .filter(|c| c["modified"] != serde_json::Value::Bool(true))
            .filter(|c| eq(&c["make"], make))
            .filter(|c| {
                model
                    .map(|m| m.is_empty() || eq(&c["model"], m))
                    .unwrap_or(true)
            })
            .filter(|c| {
                year.map(|y| c["year"].as_u64() == Some(y as u64))
                    .unwrap_or(true)
            })
            .collect()
    }

    pub fn community_updated(&self) -> Option<String> {
        Some(self.setting_or("community_updated", "")).filter(|s| !s.is_empty())
    }
}
