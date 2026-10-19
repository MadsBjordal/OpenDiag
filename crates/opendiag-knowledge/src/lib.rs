//! Offline diagnostic knowledge bundled with the app.
//!
//! * [`DtcLibrary`]: explanations of generic fault codes.
//! * [`guided`]: data-driven troubleshooting procedures and their evaluation.
//!
//! Everything is data (JSON in `data/`), so contributors add knowledge without
//! touching code. Every entry carries its source; causes are always presented
//! as possibilities, never as conclusions.

pub mod guided;

use opendiag_obd::dtc::{CodeOwnership, Dtc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Moderate,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtcEntry {
    pub code: String,
    pub title: String,
    pub plain: String,
    pub severity: Severity,
    pub possible_causes: Vec<String>,
    pub suggested_tests: Vec<String>,
    pub related_measurements: Vec<String>,
    pub driving_advice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LibraryFile {
    format: String,
    version: u32,
    source: String,
    codes: Vec<DtcEntry>,
}

/// What the app tells the user about one code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtcExplanation {
    pub code: String,
    pub system: String,
    pub ownership: CodeOwnership,
    pub known: bool,
    pub title: String,
    pub plain: String,
    pub technical: String,
    pub severity: Severity,
    /// How sure the severity is.
    pub severity_note: String,
    pub possible_causes: Vec<String>,
    pub suggested_tests: Vec<String>,
    pub related_measurements: Vec<String>,
    pub driving_advice: Option<String>,
    pub source: String,
}

pub struct DtcLibrary {
    source: String,
    by_code: HashMap<String, DtcEntry>,
}

const BUNDLED: &str = include_str!("../data/dtc-generic.json");

impl DtcLibrary {
    pub fn bundled() -> Self {
        Self::from_json(BUNDLED).expect("bundled DTC library is valid")
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let f: LibraryFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if f.format != "opendiag-dtc-library" {
            return Err(format!("unknown format {}", f.format));
        }
        let mut by_code = HashMap::new();
        for e in f.codes {
            let d = Dtc(e.code.clone());
            if d.to_bytes().is_none() {
                return Err(format!("invalid code {}", e.code));
            }
            if by_code.insert(e.code.clone(), e).is_some() {
                return Err("duplicate code".into());
            }
        }
        let _ = f.version;
        Ok(Self { source: f.source, by_code })
    }

    pub fn len(&self) -> usize {
        self.by_code.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_code.is_empty()
    }

    pub fn search(&self, query: &str) -> Vec<DtcExplanation> {
        let q = query.trim().to_ascii_lowercase();
        // "p04", "U0100": a code (or its start) only matches codes.
        let code_like = q.len() >= 2
            && matches!(q.as_bytes()[0], b'p' | b'c' | b'b' | b'u')
            && q[1..].chars().all(|c| c.is_ascii_hexdigit());
        let mut out: Vec<DtcExplanation> = self
            .by_code
            .values()
            .filter(|e| {
                q.is_empty()
                    || e.code.to_ascii_lowercase().starts_with(&q)
                    || (!code_like
                        && (e.title.to_ascii_lowercase().contains(&q) || e.plain.to_ascii_lowercase().contains(&q)))
            })
            .map(|e| self.explain(&e.code))
            .collect();
        out.sort_by(|a, b| a.code.cmp(&b.code));
        out
    }

    pub fn explain(&self, code: &str) -> DtcExplanation {
        let code = code.trim().to_ascii_uppercase();
        let dtc = Dtc(code.clone());
        let ownership = dtc.ownership();
        match self.by_code.get(&code) {
            Some(e) => DtcExplanation {
                code: code.clone(),
                system: dtc.system().into(),
                ownership,
                known: true,
                title: e.title.clone(),
                plain: e.plain.clone(),
                technical: format!("{code}: {} (generic SAE J2012 definition).", e.title),
                severity: e.severity,
                severity_note: "Typical severity for this code. The real urgency depends on the vehicle and the cause.".into(),
                possible_causes: e.possible_causes.clone(),
                suggested_tests: e.suggested_tests.clone(),
                related_measurements: e.related_measurements.clone(),
                driving_advice: e.driving_advice.clone(),
                source: self.source.clone(),
            },
            None => {
                let (plain, technical) = match ownership {
                    CodeOwnership::Manufacturer => (
                        "This is a manufacturer-specific code. Its meaning depends on the make and model, and OpenDiag has no verified description for it yet.".to_string(),
                        format!("{code}: manufacturer-defined {} code (SAE J2012 numbering).", dtc.system().to_lowercase()),
                    ),
                    _ => (
                        "OpenDiag has no description for this code yet.".to_string(),
                        format!("{code}: {} code not in the offline library.", dtc.system().to_lowercase()),
                    ),
                };
                DtcExplanation {
                    code,
                    system: dtc.system().into(),
                    ownership,
                    known: false,
                    title: "No description available".into(),
                    plain,
                    technical,
                    severity: Severity::Moderate,
                    severity_note: "Unknown: treat it as needing attention until you know more.".into(),
                    possible_causes: vec![],
                    suggested_tests: vec!["Look the code up in the vehicle's service information.".into()],
                    related_measurements: vec![],
                    driving_advice: None,
                    source: "Code structure only (SAE J2012).".into(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_library_loads() {
        let lib = DtcLibrary::bundled();
        assert!(lib.len() > 90);
        let e = lib.explain("p0301");
        assert!(e.known);
        assert_eq!(e.system, "Powertrain");
        assert!(e.title.contains("Cylinder 1"));
        assert!(!e.possible_causes.is_empty());
    }

    #[test]
    fn related_measurements_exist() {
        let lib = DtcLibrary::bundled();
        for e in lib.by_code.values() {
            for k in &e.related_measurements {
                assert!(opendiag_obd::pid::def_by_key(k).is_some(), "{}: unknown measurement {k}", e.code);
            }
        }
    }

    #[test]
    fn unknown_codes_are_honest() {
        let lib = DtcLibrary::bundled();
        let e = lib.explain("P1234");
        assert!(!e.known);
        assert_eq!(e.ownership, CodeOwnership::Manufacturer);
        assert!(e.plain.contains("manufacturer-specific"));
        assert!(e.possible_causes.is_empty());
    }

    #[test]
    fn search() {
        let lib = DtcLibrary::bundled();
        assert!(lib.search("misfire").len() >= 9);
        assert_eq!(lib.search("P0420").len(), 1);
    }

    #[test]
    fn rejects_bad_files() {
        assert!(DtcLibrary::from_json("{}").is_err());
        let bad = r#"{"format":"opendiag-dtc-library","version":1,"source":"x","codes":[{"code":"X123","title":"","plain":"","severity":"low","possible_causes":[],"suggested_tests":[],"related_measurements":[],"driving_advice":null}]}"#;
        assert!(DtcLibrary::from_json(bad).is_err());
    }
}
