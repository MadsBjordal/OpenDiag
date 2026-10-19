//! Data-only vehicle definitions contributed by the community.
//!
//! A definition says which vehicles and ECU revisions it covers, where its
//! information comes from, what adapter it needs and which functions it offers,
//! each with a risk level and a verification status. Definitions are data, not
//! code: they can never run anything, and the engine still passes every request
//! through the safety policy.
//!
//! Validation rejects malformed or unsafe definitions: unknown fields, missing
//! provenance, level 4 functions, requests whose real risk is higher than
//! declared, and changing functions without verification and evidence.

use opendiag_safety::{classify, RiskLevel};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    /// From a public standard (SAE, ISO).
    Standard,
    /// Information the manufacturer published for independent repairers.
    ManufacturerPublic,
    /// Found by the contributor's own testing on their own vehicle.
    CommunityResearch,
    /// Describes a simulator profile.
    Simulated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub kind: ProvenanceKind,
    pub source: String,
    pub license: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Years {
    pub from: u16,
    pub to: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VehicleScope {
    pub make: String,
    pub models: Vec<String>,
    pub years: Years,
    pub kinds: Vec<String>,
    pub markets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EcuScope {
    pub role: String,
    #[serde(default)]
    pub address: Option<String>,
    pub hw_ids: Vec<String>,
    pub sw_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliesTo {
    pub vehicle: VehicleScope,
    pub ecus: Vec<EcuScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterNeeds {
    pub protocols: Vec<opendiag_core::ObdProtocol>,
    pub headers: bool,
    pub multi_frame: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    pub adapter: AdapterNeeds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    Unverified,
    SimulatorTested,
    CommunityTested,
    /// Defined by a public standard; still needs testing per vehicle.
    StandardDefined,
    MaintainerVerified,
    HardwareInTheLoopVerified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Function {
    pub id: String,
    pub kind: String,
    pub risk_level: u8,
    /// The diagnostic request as hex, e.g. "010C".
    pub request: String,
    pub verification: Verification,
    pub evidence: Vec<String>,
    #[serde(default)]
    pub rollback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contributor {
    pub name: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub provenance: Provenance,
    pub applies_to: AppliesTo,
    pub requires: Requires,
    pub functions: Vec<Function>,
    pub contributors: Vec<Contributor>,
}

fn level(n: u8) -> Option<RiskLevel> {
    Some(match n {
        0 => RiskLevel::ReadOnly,
        1 => RiskLevel::Maintenance,
        2 => RiskLevel::Personalization,
        3 => RiskLevel::AdvancedService,
        4 => RiskLevel::HighRisk,
        _ => return None,
    })
}

/// Parses and validates a definition. Returns every problem found.
pub fn parse(json: &str) -> Result<Definition, Vec<String>> {
    let d: Definition =
        serde_json::from_str(json).map_err(|e| vec![format!("not a valid definition: {e}")])?;
    validate(&d)?;
    Ok(d)
}

pub fn validate(d: &Definition) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    if d.schema_version != SCHEMA_VERSION {
        errs.push(format!("schema_version must be {SCHEMA_VERSION}"));
    }
    if d.id.is_empty()
        || !d
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        errs.push("id must be lower-case letters, digits and dashes".into());
    }
    let parts: Vec<_> = d.version.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
        errs.push("version must look like 1.2.3".into());
    }
    if d.provenance.source.trim().len() < 10 {
        errs.push("provenance.source must say where the information comes from".into());
    }
    if d.provenance.license.trim().is_empty() {
        errs.push("provenance.license is required".into());
    }
    if d.contributors.is_empty() {
        errs.push("at least one contributor is required".into());
    }
    if let Some(to) = d.applies_to.vehicle.years.to {
        if to < d.applies_to.vehicle.years.from {
            errs.push("years.to is before years.from".into());
        }
    }
    let simulated = d.provenance.kind == ProvenanceKind::Simulated;
    if simulated && !d.name.starts_with("SIMULATED") {
        errs.push("simulated definitions must have a name starting with SIMULATED".into());
    }
    let mut ids = std::collections::HashSet::new();
    for f in &d.functions {
        if !ids.insert(&f.id) {
            errs.push(format!("{}: duplicate function id", f.id));
        }
        let Some(declared) = level(f.risk_level) else {
            errs.push(format!("{}: risk_level must be 0–4", f.id));
            continue;
        };
        if declared == RiskLevel::HighRisk {
            errs.push(format!("{}: level 4 functions are not accepted", f.id));
        }
        let Some(bytes) = opendiag_core::parse_hex(&f.request) else {
            errs.push(format!("{}: request is not hex", f.id));
            continue;
        };
        let actual = classify(&bytes);
        if actual.level > declared {
            errs.push(format!(
                "{}: request is \"{}\" (level {}), higher than the declared level {}",
                f.id, actual.what, actual.level as u8, f.risk_level
            ));
        }
        if actual.level == RiskLevel::HighRisk {
            errs.push(format!("{}: \"{}\" is never allowed", f.id, actual.what));
        }
        if declared >= RiskLevel::Personalization {
            if f.verification < Verification::MaintainerVerified {
                errs.push(format!(
                    "{}: changing functions need maintainer or hardware-in-the-loop verification",
                    f.id
                ));
            }
            if f.evidence.is_empty() {
                errs.push(format!("{}: changing functions need test evidence", f.id));
            }
            if f.rollback.as_deref().unwrap_or("").is_empty() {
                errs.push(format!(
                    "{}: changing functions must describe how to restore the original value",
                    f.id
                ));
            }
            if d.applies_to
                .ecus
                .iter()
                .all(|e| e.sw_ids.is_empty() && e.hw_ids.is_empty())
            {
                errs.push(format!("{}: changing functions must name the exact ECU hardware/software they were tested on", f.id));
            }
        }
        if simulated && f.verification > Verification::SimulatorTested {
            errs.push(format!(
                "{}: a simulated definition cannot claim more than simulator testing",
                f.id
            ));
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

/// What is known about the connected vehicle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VehicleContext {
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<u16>,
    pub kind: Option<String>,
    /// Calibration ids / software ids read from the ECUs.
    pub ecu_ids: Vec<String>,
    pub simulated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchQuality {
    /// Matches by vehicle details only (make/model/year): ECUs not confirmed.
    VehicleOnly,
    /// An ECU's hardware or software id matches exactly.
    EcuConfirmed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub id: String,
    pub name: String,
    pub quality: MatchQuality,
    pub note: String,
}

fn eq_or_star(want: &str, have: Option<&str>) -> bool {
    want == "*" || have.map(|h| h.eq_ignore_ascii_case(want)).unwrap_or(false)
}

pub fn matches(defs: &[Definition], v: &VehicleContext) -> Vec<Match> {
    let mut out = Vec::new();
    for d in defs {
        let sim = d.provenance.kind == ProvenanceKind::Simulated;
        if sim != v.simulated && d.applies_to.vehicle.make != "*" {
            continue;
        }
        let s = &d.applies_to.vehicle;
        if !eq_or_star(&s.make, v.make.as_deref()) {
            continue;
        }
        if !s.models.iter().any(|m| eq_or_star(m, v.model.as_deref())) && s.make != "*" && !sim {
            continue;
        }
        if let Some(y) = v.year {
            if y < s.years.from || s.years.to.map(|t| y > t).unwrap_or(false) {
                continue;
            }
        }
        if let (Some(k), false) = (&v.kind, s.kinds.is_empty()) {
            if !s.kinds.iter().any(|x| x == k || x == "*") {
                continue;
            }
        }
        let ecu_hit = d
            .applies_to
            .ecus
            .iter()
            .flat_map(|e| e.hw_ids.iter().chain(&e.sw_ids))
            .any(|id| v.ecu_ids.iter().any(|x| x.eq_ignore_ascii_case(id)));
        let (quality, note) = if ecu_hit {
            (
                MatchQuality::EcuConfirmed,
                "An ECU identification matches this definition.".to_string(),
            )
        } else {
            (
                MatchQuality::VehicleOnly,
                "Matches the vehicle details only. The VIN or model does not prove which ECU hardware/software is fitted.".to_string(),
            )
        };
        out.push(Match {
            id: d.id.clone(),
            name: d.name.clone(),
            quality,
            note,
        });
    }
    out.sort_by_key(|m| std::cmp::Reverse(m.quality));
    out
}

pub fn bundled() -> Vec<Definition> {
    [
        include_str!("../../../definitions/vehicles/generic-obd2.json"),
        include_str!("../../../definitions/vehicles/sim-car.json"),
    ]
    .iter()
    .map(|j| parse(j).expect("bundled definition is valid"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../definitions/vehicles/generic-obd2.json"
        ))
        .unwrap()
    }

    #[test]
    fn bundled_definitions_are_valid() {
        assert_eq!(bundled().len(), 2);
    }

    #[test]
    fn rejects_unknown_fields_and_missing_provenance() {
        let mut v = base();
        v["surprise"] = serde_json::json!(true);
        assert!(parse(&v.to_string()).is_err());
        let mut v = base();
        v["provenance"]["source"] = serde_json::json!("");
        let e = parse(&v.to_string()).unwrap_err();
        assert!(e[0].contains("provenance"));
    }

    #[test]
    fn rejects_mislabelled_risk() {
        let mut v = base();
        v["functions"][0]["request"] = serde_json::json!("2EF19001");
        let e = parse(&v.to_string()).unwrap_err();
        assert!(
            e.iter().any(|x| x.contains("higher than the declared")),
            "{e:?}"
        );
    }

    #[test]
    fn rejects_level_four_and_unverified_writes() {
        let mut v = base();
        v["functions"][0] = serde_json::json!({"id":"flash","kind":"programming","risk_level":4,"request":"3400","verification":"maintainer_verified","evidence":["x"],"rollback":"x"});
        assert!(parse(&v.to_string()).is_err());
        let mut v = base();
        v["functions"][0] = serde_json::json!({"id":"lights","kind":"personalization","risk_level":2,"request":"2E010201","verification":"community_tested","evidence":[]});
        let e = parse(&v.to_string()).unwrap_err();
        assert!(e.len() >= 4, "{e:?}");
    }

    #[test]
    fn matching_is_honest_about_ecus() {
        let defs = bundled();
        let real = VehicleContext {
            make: Some("Volkswagen".into()),
            year: Some(2015),
            kind: Some("car".into()),
            ..Default::default()
        };
        let m = matches(&defs, &real);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, "generic-obd2");
        assert_eq!(m[0].quality, MatchQuality::VehicleOnly);
        let sim = VehicleContext {
            make: Some("Simulated".into()),
            ecu_ids: vec!["SIMCAL-CAR-0001".into()],
            simulated: true,
            ..Default::default()
        };
        let m = matches(&defs, &sim);
        assert_eq!(m[0].id, "sim-car");
        assert_eq!(m[0].quality, MatchQuality::EcuConfirmed);
    }
}
