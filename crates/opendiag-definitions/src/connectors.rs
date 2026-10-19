//! The connector catalog: every diagnostic socket OpenDiag knows about, what
//! it carries, and what OpenDiag can do with it.
//!
//! Data lives in `definitions/connectors.json`. Every entry names its sources
//! and a confidence level; pins are only listed where a source gives them.
//! A connector being in the catalog does not mean OpenDiag supports it: see
//! `support` and `adapter`.

use opendiag_core::SupportStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Car,
    Motorcycle,
    Truck,
    Agricultural,
    Powersports,
    Marine,
}

/// How information comes out of the connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    /// A data link a tool can talk to (K-line, CAN, J1850…).
    DataLink,
    /// No data: bridging pins makes a warning light flash codes.
    BlinkCodes,
    /// Both, depending on model/year.
    DataLinkOrBlinkCodes,
    /// Codes are shown on the dashboard through a button/key sequence.
    DashboardSelfTest,
}

/// Can an ELM327-class adapter (what most people own) be used?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Elm327 {
    /// Plugs straight in.
    Yes,
    /// With a passive adapter cable to a 16-pin socket.
    WithCable,
    /// Partly (e.g. only some models, or unreliable timing).
    Limited,
    /// No: needs a different interface or a manufacturer tool.
    No,
    /// Not applicable (blink codes, dashboard test).
    NotNeeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub pin: String,
    pub signal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub url: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connector {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub makes: Vec<String>,
    /// Approximate years / models, as the sources state them.
    pub applies_to: String,
    /// Number of pins, `null` when sources disagree or do not say.
    pub pins: Option<u8>,
    pub description: String,
    /// "12 V", "24 V", "12/24 V", or empty when unknown.
    #[serde(default)]
    pub voltage: String,
    pub access: Access,
    pub protocols: Vec<String>,
    /// Pin functions, only those a source gives.
    #[serde(default)]
    pub pinout: Vec<Pin>,
    pub elm327: Elm327,
    /// What hardware is needed, in plain words.
    pub adapter: String,
    /// What OpenDiag can do with this connector today.
    pub support: SupportStatus,
    pub support_note: String,
    /// How to read codes when `access` is blink codes or a dashboard test.
    #[serde(default)]
    pub how_to_read: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub confidence: Confidence,
    pub sources: Vec<Source>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: String,
    version: u32,
    connectors: Vec<Connector>,
}

const BUNDLED: &str = include_str!("../../../definitions/connectors.json");

pub fn bundled() -> Vec<Connector> {
    load(BUNDLED).expect("bundled connector catalog is valid")
}

pub fn load(json: &str) -> Result<Vec<Connector>, Vec<String>> {
    let f: File = serde_json::from_str(json).map_err(|e| vec![e.to_string()])?;
    let mut errs = Vec::new();
    if f.format != "opendiag-connectors" || f.version != 1 {
        errs.push("format must be opendiag-connectors version 1".to_string());
    }
    let mut ids = std::collections::HashSet::new();
    for c in &f.connectors {
        let id = &c.id;
        if !ids.insert(id.clone()) {
            errs.push(format!("{id}: duplicate id"));
        }
        if !id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        {
            errs.push(format!(
                "{id}: id must be lower-case letters, digits and dashes"
            ));
        }
        if c.sources.is_empty()
            || c.sources
                .iter()
                .any(|s| !s.url.starts_with("https://") && !s.url.starts_with("http://"))
        {
            errs.push(format!("{id}: at least one source URL is required"));
        }
        if c.confidence == Confidence::Low && !c.pinout.is_empty() {
            errs.push(format!("{id}: do not list pins when confidence is low"));
        }
        if let Some(n) = c.pins {
            for p in &c.pinout {
                if let Ok(num) = p.pin.parse::<u8>() {
                    if num == 0 || num > n {
                        errs.push(format!("{id}: pin {num} outside 1–{n}"));
                    }
                }
            }
        }
        let blink = matches!(c.access, Access::BlinkCodes | Access::DashboardSelfTest);
        if blink && c.how_to_read.is_empty() {
            errs.push(format!(
                "{id}: blink-code / dashboard connectors need how_to_read steps"
            ));
        }
        if blink && c.elm327 != Elm327::NotNeeded {
            errs.push(format!(
                "{id}: no adapter is involved in reading blink codes"
            ));
        }
        if c.support == SupportStatus::ImplementedTested {
            errs.push(format!(
                "{id}: 'implemented_tested' needs hardware evidence in docs/compatibility.md first"
            ));
        }
        if c.adapter.trim().is_empty()
            || c.support_note.trim().is_empty()
            || c.description.trim().is_empty()
        {
            errs.push(format!(
                "{id}: description, adapter and support_note are required"
            ));
        }
    }
    if errs.is_empty() {
        Ok(f.connectors)
    } else {
        Err(errs)
    }
}

/// Connectors for a vehicle category and (optionally) make, best matches first.
pub fn find<'a>(
    all: &'a [Connector],
    category: Option<Category>,
    make: Option<&str>,
) -> Vec<&'a Connector> {
    let make = make
        .map(|m| m.trim().to_lowercase())
        .filter(|m| !m.is_empty());
    let mut out: Vec<&Connector> = all
        .iter()
        .filter(|c| category.map(|cat| c.category == cat).unwrap_or(true))
        .filter(|c| match &make {
            None => true,
            Some(m) => c.makes.iter().any(|x| x.to_lowercase() == *m || x == "*"),
        })
        .collect();
    // Exact make first, then generic ("*") entries.
    out.sort_by_key(|c| (c.makes.iter().any(|x| x == "*"), c.name.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_valid() {
        let all = bundled();
        assert!(!all.is_empty());
        assert!(all.iter().any(|c| c.id == "sae-j1962-obd2"));
    }

    #[test]
    fn find_by_make_and_category() {
        let all = bundled();
        let cars = find(&all, Some(Category::Car), Some("volkswagen"));
        assert!(
            cars.iter().any(|c| c.id == "sae-j1962-obd2"),
            "generic entries match every make"
        );
        assert!(find(&all, Some(Category::Motorcycle), Some("nobody"))
            .iter()
            .all(|c| c.makes.contains(&"*".to_string())));
    }

    #[test]
    fn rejects_unsourced_or_overclaimed_entries() {
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        v["connectors"][0]["sources"] = serde_json::json!([]);
        assert!(load(&v.to_string()).is_err());
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        v["connectors"][0]["support"] = serde_json::json!("implemented_tested");
        assert!(load(&v.to_string()).is_err());
        let mut v: serde_json::Value = serde_json::from_str(BUNDLED).unwrap();
        v["connectors"][0]["surprise"] = serde_json::json!(1);
        assert!(load(&v.to_string()).is_err());
    }
}
