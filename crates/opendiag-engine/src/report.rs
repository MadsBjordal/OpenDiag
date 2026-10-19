//! The scan report, and what can be done with it (CSV, HTML, PDF, comparison).

use opendiag_core::{AdapterCapabilities, LinkInfo, ObdProtocol};
use opendiag_knowledge::DtcExplanation;
use opendiag_obd::readiness::MonitorStatus;
use opendiag_obd::vin::VinInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DtcKind {
    /// Confirmed and stored (service 03).
    Stored,
    /// Detected once, not confirmed yet (service 07).
    Pending,
    /// Cannot be cleared with a tool; goes away when the vehicle's own test passes (service 0A).
    Permanent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtcFinding {
    pub code: String,
    pub ecu: String,
    pub ecu_label: String,
    pub kind: DtcKind,
    pub explanation: DtcExplanation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStatus {
    Scanned,
    PartlyScanned,
    NotScanned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleResult {
    pub category: String,
    pub ecu: Option<String>,
    pub label: String,
    pub status: ModuleStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub key: String,
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub ecu: String,
    /// "vehicle" (an ECU reported it), "adapter" (measured by the adapter), "derived" (calculated).
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreezeFrame {
    pub ecu: String,
    pub dtc: Option<String>,
    pub values: Vec<Measurement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleIdentity {
    pub vin: Option<VinInfo>,
    pub calibration_ids: Vec<(String, String)>,
    pub ecu_names: Vec<(String, String)>,
    pub obd_standard: Option<String>,
    pub fuel_type: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    FaultsFound,
    NoFaultsInScannedModules,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub verdict: Verdict,
    pub headline: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub at: String,
    pub what: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub id: String,
    pub created_at: String,
    pub app: String,
    pub app_version: String,
    pub platform: String,
    pub simulated: bool,
    pub link: LinkInfo,
    pub adapter: AdapterCapabilities,
    pub protocol: Option<ObdProtocol>,
    pub vehicle: VehicleIdentity,
    pub modules: Vec<ModuleResult>,
    pub dtcs: Vec<DtcFinding>,
    pub readiness: Vec<(String, MonitorStatus)>,
    pub freeze_frames: Vec<FreezeFrame>,
    pub measurements: Vec<Measurement>,
    pub warnings: Vec<String>,
    pub communication_errors: Vec<String>,
    pub health: Health,
    pub notes: String,
    pub operations: Vec<Operation>,
    pub cautions: Vec<String>,
}

impl ModuleStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Scanned => "Scanned",
            Self::PartlyScanned => "Partly scanned",
            Self::NotScanned => "Not scanned",
        }
    }
}

impl DtcKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stored => "stored",
            Self::Pending => "pending",
            Self::Permanent => "permanent",
        }
    }
}

impl ScanReport {
    pub fn dtc_count(&self) -> usize {
        self.dtcs.len()
    }

    pub fn title(&self) -> String {
        let who = self
            .vehicle
            .vin
            .as_ref()
            .and_then(|v| v.manufacturer.clone())
            .unwrap_or_else(|| {
                if self.simulated {
                    "Simulated vehicle".into()
                } else {
                    "Vehicle".into()
                }
            });
        let sim = if self.simulated { "SIMULATED · " } else { "" };
        format!("{sim}{who} · {} fault code(s)", self.dtc_count())
    }

    /// A copy with the VIN masked (for sharing).
    pub fn redacted(&self) -> ScanReport {
        let mut r = self.clone();
        if let Some(v) = r.vehicle.vin.as_mut() {
            v.vin = mask_vin(&v.vin);
        }
        r
    }
}

pub fn mask_vin(vin: &str) -> String {
    let keep: String = vin.chars().take(3).collect();
    format!("{keep}{}", "*".repeat(vin.len().saturating_sub(3)))
}

pub const STANDARD_CAUTIONS: [&str; 3] = [
    "Fault codes show what a control module detected. They do not by themselves prove that a part is faulty.",
    "Possible causes are hypotheses to check, listed from general knowledge. Confirm with tests before replacing parts.",
    "Generic OBD-II only reaches emissions-related modules. Systems marked \"not scanned\" were not checked.",
];

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn to_csv(r: &ScanReport) -> String {
    let mut out = String::from("section,ecu,item,value,unit,status,description\n");
    let mut row = |cols: [&str; 7]| {
        out.push_str(
            &cols
                .iter()
                .map(|c| csv_field(c))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    };
    row([
        "report",
        "",
        "created_at",
        &r.created_at,
        "",
        if r.simulated { "SIMULATED" } else { "" },
        &r.health.headline,
    ]);
    if let Some(v) = &r.vehicle.vin {
        row([
            "vehicle",
            "",
            "vin",
            &v.vin,
            "",
            "",
            v.manufacturer.as_deref().unwrap_or(""),
        ]);
    }
    for m in &r.modules {
        row([
            "module",
            m.ecu.as_deref().unwrap_or(""),
            &m.label,
            "",
            "",
            m.status.label(),
            &m.detail,
        ]);
    }
    for d in &r.dtcs {
        row([
            "dtc",
            &d.ecu,
            &d.code,
            "",
            "",
            d.kind.label(),
            &d.explanation.title,
        ]);
    }
    for f in &r.freeze_frames {
        for m in &f.values {
            row([
                "freeze_frame",
                &f.ecu,
                &m.name,
                &format!("{:.2}", m.value),
                &m.unit,
                f.dtc.as_deref().unwrap_or(""),
                "",
            ]);
        }
    }
    for m in &r.measurements {
        row([
            "measurement",
            &m.ecu,
            &m.name,
            &format!("{:.2}", m.value),
            &m.unit,
            &m.source,
            "",
        ]);
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A stand-alone printable HTML page.
pub fn to_html(r: &ScanReport) -> String {
    let mut h = String::new();
    h.push_str("<!doctype html><html><head><meta charset=\"utf-8\"><title>");
    h.push_str(&esc(&r.title()));
    h.push_str("</title><style>body{font:14px system-ui,sans-serif;max-width:860px;margin:24px auto;color:#111}h1{font-size:22px}h2{font-size:17px;margin-top:28px;border-bottom:1px solid #ccc}table{border-collapse:collapse;width:100%}td,th{border:1px solid #ddd;padding:4px 6px;text-align:left;vertical-align:top}.sim{background:#7c3aed;color:#fff;padding:6px 10px;font-weight:700}.muted{color:#555}</style></head><body>");
    if r.simulated {
        h.push_str("<div class=\"sim\">SIMULATED VEHICLE: not a real diagnostic result</div>");
    }
    h.push_str(&format!(
        "<h1>{}</h1><p class=\"muted\">{} · {} {} · {}</p>",
        esc(&r.title()),
        esc(&r.created_at),
        esc(&r.app),
        esc(&r.app_version),
        esc(&r.platform)
    ));
    h.push_str(&format!(
        "<p><b>{}</b><br>{}</p>",
        esc(&r.health.headline),
        esc(&r.health.detail)
    ));
    h.push_str("<h2>Vehicle and adapter</h2><table>");
    let vin = r
        .vehicle
        .vin
        .as_ref()
        .map(|v| v.vin.clone())
        .unwrap_or_else(|| "not reported".into());
    for (k, v) in [
        ("VIN", vin),
        (
            "Manufacturer (from VIN)",
            r.vehicle
                .vin
                .as_ref()
                .and_then(|v| v.manufacturer.clone())
                .unwrap_or_else(|| "unknown".into()),
        ),
        (
            "OBD standard",
            r.vehicle
                .obd_standard
                .clone()
                .unwrap_or_else(|| "not reported".into()),
        ),
        (
            "Fuel type",
            r.vehicle
                .fuel_type
                .clone()
                .unwrap_or_else(|| "not reported".into()),
        ),
        ("Adapter", r.adapter.identity.clone()),
        ("Connection", r.link.name.clone()),
        (
            "Protocol",
            r.protocol
                .map(|p| p.label().to_string())
                .unwrap_or_else(|| "unknown".into()),
        ),
    ] {
        h.push_str(&format!("<tr><th>{}</th><td>{}</td></tr>", esc(k), esc(&v)));
    }
    h.push_str("</table>");
    h.push_str("<h2>Modules</h2><table><tr><th>System</th><th>Status</th><th>Detail</th></tr>");
    for m in &r.modules {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&m.label),
            m.status.label(),
            esc(&m.detail)
        ));
    }
    h.push_str("</table><h2>Fault codes</h2>");
    if r.dtcs.is_empty() {
        h.push_str("<p>No fault codes reported by the modules that were scanned.</p>");
    } else {
        h.push_str("<table><tr><th>Code</th><th>Type</th><th>Module</th><th>Meaning</th><th>Possible causes (to check)</th></tr>");
        for d in &r.dtcs {
            h.push_str(&format!(
                "<tr><td><b>{}</b></td><td>{}</td><td>{}</td><td>{}<br><span class=\"muted\">{}</span></td><td>{}</td></tr>",
                esc(&d.code),
                d.kind.label(),
                esc(&d.ecu_label),
                esc(&d.explanation.title),
                esc(&d.explanation.plain),
                esc(&d.explanation.possible_causes.join("; "))
            ));
        }
        h.push_str("</table>");
    }
    if !r.freeze_frames.is_empty() {
        h.push_str("<h2>Freeze frame (values when the fault was stored)</h2>");
        for f in &r.freeze_frames {
            h.push_str(&format!(
                "<p>{} · {}</p><table>",
                esc(&f.ecu),
                esc(f.dtc.as_deref().unwrap_or("-"))
            ));
            for m in &f.values {
                h.push_str(&format!(
                    "<tr><td>{}</td><td>{:.1} {}</td></tr>",
                    esc(&m.name),
                    m.value,
                    esc(&m.unit)
                ));
            }
            h.push_str("</table>");
        }
    }
    if !r.measurements.is_empty() {
        h.push_str("<h2>Measurements at scan time</h2><table>");
        for m in &r.measurements {
            h.push_str(&format!(
                "<tr><td>{}</td><td>{:.2} {}</td><td class=\"muted\">{}</td></tr>",
                esc(&m.name),
                m.value,
                esc(&m.unit),
                esc(&m.source)
            ));
        }
        h.push_str("</table>");
    }
    if !r.notes.is_empty() {
        h.push_str(&format!("<h2>Notes</h2><p>{}</p>", esc(&r.notes)));
    }
    if !r.operations.is_empty() {
        h.push_str("<h2>Operations performed</h2><ul>");
        for o in &r.operations {
            h.push_str(&format!(
                "<li>{} · {}: {}</li>",
                esc(&o.at),
                esc(&o.what),
                esc(&o.detail)
            ));
        }
        h.push_str("</ul>");
    }
    h.push_str("<h2>Important</h2><ul>");
    for c in &r.cautions {
        h.push_str(&format!("<li>{}</li>", esc(c)));
    }
    h.push_str("</ul></body></html>");
    h
}

/// Lines used by the PDF writer.
pub fn to_text_lines(r: &ScanReport) -> Vec<(bool, String)> {
    let mut l: Vec<(bool, String)> = Vec::new();
    if r.simulated {
        l.push((
            true,
            "SIMULATED VEHICLE - not a real diagnostic result".into(),
        ));
    }
    l.push((true, r.title()));
    l.push((
        false,
        format!(
            "{} | {} {} | {}",
            r.created_at, r.app, r.app_version, r.platform
        ),
    ));
    l.push((false, String::new()));
    l.push((true, r.health.headline.clone()));
    l.push((false, r.health.detail.clone()));
    l.push((false, String::new()));
    l.push((true, "Vehicle and adapter".into()));
    if let Some(v) = &r.vehicle.vin {
        l.push((
            false,
            format!(
                "VIN: {}  ({})",
                v.vin,
                v.manufacturer.as_deref().unwrap_or("manufacturer unknown")
            ),
        ));
    }
    l.push((
        false,
        format!("Adapter: {} via {}", r.adapter.identity, r.link.name),
    ));
    l.push((
        false,
        format!(
            "Protocol: {}",
            r.protocol.map(|p| p.label()).unwrap_or("unknown")
        ),
    ));
    l.push((false, String::new()));
    l.push((true, "Modules".into()));
    for m in &r.modules {
        l.push((
            false,
            format!("{}: {} - {}", m.status.label(), m.label, m.detail),
        ));
    }
    l.push((false, String::new()));
    l.push((true, "Fault codes".into()));
    if r.dtcs.is_empty() {
        l.push((false, "None reported by the scanned modules.".into()));
    }
    for d in &r.dtcs {
        l.push((
            false,
            format!(
                "{} ({}, {}): {}",
                d.code,
                d.kind.label(),
                d.ecu_label,
                d.explanation.title
            ),
        ));
        l.push((false, format!("    {}", d.explanation.plain)));
        if !d.explanation.possible_causes.is_empty() {
            l.push((
                false,
                format!(
                    "    Possible causes to check: {}",
                    d.explanation.possible_causes.join("; ")
                ),
            ));
        }
    }
    for f in &r.freeze_frames {
        l.push((false, String::new()));
        l.push((
            true,
            format!(
                "Freeze frame {} ({})",
                f.ecu,
                f.dtc.as_deref().unwrap_or("-")
            ),
        ));
        for m in &f.values {
            l.push((false, format!("{}: {:.1} {}", m.name, m.value, m.unit)));
        }
    }
    if !r.measurements.is_empty() {
        l.push((false, String::new()));
        l.push((true, "Measurements at scan time".into()));
        for m in &r.measurements {
            l.push((
                false,
                format!("{}: {:.2} {} ({})", m.name, m.value, m.unit, m.source),
            ));
        }
    }
    if !r.notes.is_empty() {
        l.push((false, String::new()));
        l.push((true, "Notes".into()));
        l.push((false, r.notes.clone()));
    }
    if !r.operations.is_empty() {
        l.push((false, String::new()));
        l.push((true, "Operations performed".into()));
        for o in &r.operations {
            l.push((false, format!("{} {}: {}", o.at, o.what, o.detail)));
        }
    }
    l.push((false, String::new()));
    l.push((true, "Important".into()));
    for c in &r.cautions {
        l.push((false, format!("- {c}")));
    }
    l
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comparison {
    pub older: String,
    pub newer: String,
    /// In the newer report only.
    pub new_codes: Vec<(String, String)>,
    /// In both.
    pub still_present: Vec<(String, String)>,
    /// In the older report only.
    pub gone: Vec<(String, String)>,
}

pub fn compare(older: &ScanReport, newer: &ScanReport) -> Comparison {
    let key = |d: &DtcFinding| (d.code.clone(), d.kind.label().to_string());
    let a: Vec<_> = older.dtcs.iter().map(key).collect();
    let b: Vec<_> = newer.dtcs.iter().map(key).collect();
    let mut c = Comparison {
        older: older.created_at.clone(),
        newer: newer.created_at.clone(),
        new_codes: b.iter().filter(|x| !a.contains(x)).cloned().collect(),
        still_present: b.iter().filter(|x| a.contains(x)).cloned().collect(),
        gone: a.iter().filter(|x| !b.contains(x)).cloned().collect(),
    };
    for v in [&mut c.new_codes, &mut c.still_present, &mut c.gone] {
        v.sort();
        v.dedup();
    }
    c
}
