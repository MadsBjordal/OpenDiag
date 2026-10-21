//! Offline diagnostic knowledge bundled with the app.
//!
//! * [`DtcLibrary`]: explanations of generic fault codes (written for OpenDiag),
//!   standard titles of all generic codes and manufacturer-specific titles by
//!   make (both from the MIT-licensed dtc-database, see `scripts/import_dtc_titles.py`).
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
    /// Standard titles of generic codes (no explanation written yet).
    titles: HashMap<String, String>,
    titles_source: String,
    /// Manufacturer-specific titles: dataset make (e.g. "VOLKSWAGEN") → code → title.
    makes: HashMap<String, HashMap<String, String>>,
    makes_source: String,
}

#[derive(Deserialize)]
struct TitleRow {
    code: String,
    title: String,
}

#[derive(Deserialize)]
struct TitlesFile {
    format: String,
    source: String,
    codes: Vec<TitleRow>,
}

#[derive(Deserialize)]
struct MakesFile {
    format: String,
    source: String,
    makes: HashMap<String, Vec<TitleRow>>,
}

const BUNDLED: &str = include_str!("../data/dtc-generic.json");
const TITLES: &str = include_str!("../data/dtc-titles.json");
const MANUFACTURER: &str = include_str!("../data/dtc-manufacturer.json");

/// The dataset's makes to look in for a vehicle make, most specific first.
/// Brands of one group share their manufacturer codes (e.g. Škoda uses VW's).
pub fn dataset_makes(make: &str) -> Vec<&'static str> {
    let m: String = make
        .to_uppercase()
        .chars()
        .map(|c| match c {
            'Š' => 'S',
            'Ë' | 'É' => 'E',
            'Ö' => 'O',
            'Ü' => 'U',
            'Ä' => 'A',
            c => c,
        })
        .collect();
    let has = |k: &str| {
        m.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| w == k)
    };
    let pick = |own: &'static str, group: &'static str| {
        if own == group {
            vec![group]
        } else {
            vec![own, group]
        }
    };
    for (names, own, group) in [
        (&["AUDI"][..], "AUDI", "VOLKSWAGEN"),
        (
            &["VOLKSWAGEN", "VW", "SKODA", "SEAT", "CUPRA"][..],
            "VOLKSWAGEN",
            "AUDI",
        ),
        (&["FORD"][..], "FORD", "FORD"),
        (&["LINCOLN"][..], "LINCOLN", "FORD"),
        (&["MERCURY"][..], "MERCURY", "FORD"),
        (&["CHEVROLET", "CHEVY"][..], "CHEVY", "GM"),
        (&["GMC"][..], "GMC", "GM"),
        (&["BUICK"][..], "BUICK", "GM"),
        (&["CADILLAC"][..], "CADILLAC", "GM"),
        (&["PONTIAC"][..], "PONTIAC", "GM"),
        (&["OLDSMOBILE"][..], "OLDSMOBILE", "GM"),
        (&["SATURN"][..], "SATURN", "GM"),
        (&["GEO"][..], "GEO", "GM"),
        (&["GM", "GENERAL"][..], "GM", "GM"),
        (&["CHRYSLER"][..], "CHRYSLER", "CHRYSLER"),
        (&["DODGE", "RAM"][..], "DODGE", "CHRYSLER"),
        (&["JEEP"][..], "JEEP", "CHRYSLER"),
        (&["PLYMOUTH"][..], "PLYMOUTH", "CHRYSLER"),
        (&["LEXUS"][..], "LEXUS", "TOYOTA"),
        (&["TOYOTA", "SCION"][..], "TOYOTA", "TOYOTA"),
        (&["ACURA"][..], "ACURA", "HONDA"),
        (&["HONDA"][..], "HONDA", "HONDA"),
        (&["NISSAN", "INFINITI", "DATSUN"][..], "NISSAN", "NISSAN"),
        (&["MAZDA"][..], "MAZDA", "MAZDA"),
        (&["SUBARU"][..], "SUBARU", "SUBARU"),
        (&["KIA"][..], "KIA", "KIA"),
        (&["BMW", "MINI"][..], "BMW", "BMW"),
        (&["MERCEDES", "DAIMLER"][..], "MERCEDES", "MERCEDES"),
        (&["MITSUBISHI"][..], "MITSUBISHI", "MITSUBISHI"),
        (&["SUZUKI"][..], "SUZUKI", "SUZUKI"),
        (&["JAGUAR"][..], "JAGUAR", "JAGUAR"),
    ] {
        if names.iter().any(|n| has(n)) || (names.len() > 1 && m.contains(names[0])) {
            return pick(own, group);
        }
    }
    vec![]
}

impl DtcLibrary {
    pub fn bundled() -> Self {
        let mut lib = Self::from_json(BUNDLED).expect("bundled DTC library is valid");
        lib.add_titles(TITLES, MANUFACTURER)
            .expect("bundled DTC titles are valid");
        lib
    }

    /// Adds generic titles and manufacturer titles (files made by scripts/import_dtc_titles.py).
    pub fn add_titles(&mut self, titles: &str, makes: &str) -> Result<(), String> {
        let t: TitlesFile = serde_json::from_str(titles).map_err(|e| e.to_string())?;
        if t.format != "opendiag-dtc-titles" {
            return Err(format!("unknown format {}", t.format));
        }
        let m: MakesFile = serde_json::from_str(makes).map_err(|e| e.to_string())?;
        if m.format != "opendiag-dtc-manufacturer" {
            return Err(format!("unknown format {}", m.format));
        }
        for r in t.codes {
            let d = Dtc(r.code.clone());
            if d.to_bytes().is_none() || d.ownership() != CodeOwnership::Generic {
                return Err(format!("{} is not a generic code", r.code));
            }
            self.titles.insert(r.code, r.title);
        }
        for (make, rows) in m.makes {
            let codes = self.makes.entry(make).or_default();
            for r in rows {
                let d = Dtc(r.code.clone());
                if d.to_bytes().is_none() || d.ownership() != CodeOwnership::Manufacturer {
                    return Err(format!("{} is not a manufacturer code", r.code));
                }
                codes.insert(r.code, r.title);
            }
        }
        self.titles_source = t.source;
        self.makes_source = m.source;
        Ok(())
    }

    /// How many codes have at least a title (explained, generic title or manufacturer title).
    pub fn titled(&self) -> usize {
        let mut all: std::collections::HashSet<&String> =
            self.by_code.keys().chain(self.titles.keys()).collect();
        for m in self.makes.values() {
            all.extend(m.keys());
        }
        all.len()
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
        Ok(Self {
            source: f.source,
            by_code,
            titles: HashMap::new(),
            titles_source: String::new(),
            makes: HashMap::new(),
            makes_source: String::new(),
        })
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
                        && (e.title.to_ascii_lowercase().contains(&q)
                            || e.plain.to_ascii_lowercase().contains(&q)))
            })
            .map(|e| self.explain(&e.code))
            .collect();
        // Generic codes that only have a title (empty query: explained codes only).
        if !q.is_empty() {
            out.extend(
                self.titles
                    .iter()
                    .filter(|(code, _)| !self.by_code.contains_key(*code))
                    .filter(|(code, title)| {
                        code.to_ascii_lowercase().starts_with(&q)
                            || (!code_like && title.to_ascii_lowercase().contains(&q))
                    })
                    .map(|(code, _)| self.explain(code)),
            );
        }
        out.sort_by(|a, b| a.code.cmp(&b.code));
        out
    }

    pub fn explain(&self, code: &str) -> DtcExplanation {
        self.explain_for(code, None)
    }

    /// Like [`Self::explain`], using the vehicle's make for manufacturer-specific codes.
    pub fn explain_for(&self, code: &str, make: Option<&str>) -> DtcExplanation {
        let code = code.trim().to_ascii_uppercase();
        let dtc = Dtc(code.clone());
        let ownership = dtc.ownership();
        if !self.by_code.contains_key(&code) {
            let base = |title: &str, plain: String, technical: String, source: String| {
                DtcExplanation {
                code: code.clone(),
                system: dtc.system().into(),
                ownership,
                known: true,
                title: title.to_string(),
                plain,
                technical,
                severity: Severity::Moderate,
                severity_note: "Severity not assessed for this code yet: treat it as needing attention until you know more.".into(),
                possible_causes: vec![],
                suggested_tests: vec!["Look up causes and tests for this code in the vehicle's service information.".into()],
                related_measurements: vec![],
                driving_advice: None,
                source,
            }
            };
            if let Some(t) = self.titles.get(&code) {
                return base(
                    t,
                    "This is the standard meaning of the code. OpenDiag has no detailed explanation or list of possible causes for it yet.".into(),
                    format!("{code}: {t} (generic SAE J2012 definition)."),
                    self.titles_source.clone(),
                );
            }
            if ownership == CodeOwnership::Manufacturer {
                if let Some(make) = make.filter(|m| !m.trim().is_empty()) {
                    for ds in dataset_makes(make) {
                        if let Some(t) = self.makes.get(ds).and_then(|m| m.get(&code)) {
                            return base(
                                t,
                                format!("This is a manufacturer-specific code. Its meaning for {make} comes from a community dataset and is not verified: check it against the vehicle's service information."),
                                format!("{code}: {t} ({ds}-specific meaning, community dataset, not verified)."),
                                format!("{} Not verified against manufacturer documentation.", self.makes_source),
                            );
                        }
                    }
                }
            }
        }
        self.explain_curated(&code, make.is_none())
    }

    fn explain_curated(&self, code: &str, make_unknown: bool) -> DtcExplanation {
        let code = code.to_string();
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
                        if make_unknown {
                            "This is a manufacturer-specific code. Its meaning depends on the make: enter the make in My Garage so OpenDiag can look it up.".to_string()
                        } else {
                            "This is a manufacturer-specific code. Its meaning depends on the make and model, and OpenDiag has no description for it yet.".to_string()
                        },
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
    fn titles_and_manufacturer_codes() {
        let lib = DtcLibrary::bundled();
        assert!(lib.titled() > 9_500, "{}", lib.titled());
        // Explained codes keep their explanation.
        assert!(!lib.explain("P0301").possible_causes.is_empty());
        // A generic code with only a title.
        let e = lib.explain("P2187");
        assert!(e.known && e.title.to_lowercase().contains("lean"), "{e:?}");
        assert!(e.source.contains("dtc-database"));
        // Manufacturer codes need the make; Skoda uses the VW group's list.
        assert!(!lib.explain("P1101").known);
        assert!(lib.explain("P1101").plain.contains("My Garage"));
        let vw = lib.explain_for("P1101", Some("ŠKODA"));
        assert!(vw.known && vw.plain.contains("not verified"), "{vw:?}");
        assert_ne!(vw.title, lib.explain_for("P1101", Some("Ford")).title);
        assert!(!lib.explain_for("P1101", Some("Unknownmake")).known);
        // Searching a code finds titled codes too.
        assert_eq!(lib.search("P2187").len(), 1);
    }

    #[test]
    fn makes_map_to_their_group() {
        assert_eq!(dataset_makes("Skoda"), vec!["VOLKSWAGEN", "AUDI"]);
        assert_eq!(dataset_makes("AUDI AG"), vec!["AUDI", "VOLKSWAGEN"]);
        assert_eq!(dataset_makes("Chevrolet"), vec!["CHEVY", "GM"]);
        assert_eq!(dataset_makes("Infiniti"), vec!["NISSAN"]);
        assert!(dataset_makes("Husqvarna").is_empty());
    }

    #[test]
    fn related_measurements_exist() {
        let lib = DtcLibrary::bundled();
        for e in lib.by_code.values() {
            for k in &e.related_measurements {
                assert!(
                    opendiag_obd::pid::def_by_key(k).is_some(),
                    "{}: unknown measurement {k}",
                    e.code
                );
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
