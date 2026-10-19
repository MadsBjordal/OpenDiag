//! Guided troubleshooting: procedures are data, evaluation is pure.
//!
//! The UI sends every answer given so far; [`evaluate`] replays them and returns
//! the next step plus a history that keeps three things apart:
//! what was **measured**, what the **specification** is (and where it comes
//! from: documented, general guidance, or derived from an earlier measurement),
//! and what that **suggests** (always phrased as a finding or a hypothesis).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliesTo {
    pub vehicle_kinds: Vec<String>,
    pub codes: Vec<String>,
    pub systems: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Question,
    Info,
    Measurement,
    Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecKind {
    /// From the vehicle's documentation (here: typed in by the user from their manual).
    Documented,
    /// Widely used rule of thumb, not a manufacturer value.
    GeneralGuidance,
    /// Worked out from an earlier measurement.
    Derived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    pub kind: SpecKind,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementSpec {
    pub unit: String,
    #[serde(default)]
    pub spec: Option<Range>,
    #[serde(default)]
    pub relative_to: Option<String>,
    #[serde(default)]
    pub max_below: Option<f64>,
    #[serde(default)]
    pub max_above: Option<f64>,
    /// The user types the specification from their own documentation.
    #[serde(default)]
    pub user_spec: bool,
    #[serde(default)]
    pub kind: Option<SpecKind>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub kind: StepKind,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub measurement: Option<MeasurementSpec>,
    /// Live data key that can fill in the value from the vehicle.
    #[serde(default)]
    pub live_pid: Option<String>,
    #[serde(default)]
    pub next: HashMap<String, String>,
    #[serde(default)]
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    /// Follows directly from a measurement against a specification.
    Finding,
    /// A likely explanation that still needs confirming.
    Hypothesis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    pub kind: OutcomeKind,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Procedure {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub applies_to: AppliesTo,
    pub source: String,
    pub verification: String,
    pub start: String,
    pub steps: Vec<Step>,
    pub outcomes: HashMap<String, Outcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueSource {
    MeasuredByUser,
    ReadFromVehicle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub step: String,
    #[serde(default)]
    pub choice: Option<String>,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub value_source: Option<ValueSource>,
    /// For `user_spec` steps: the min/max from the user's documentation.
    #[serde(default)]
    pub spec_min: Option<f64>,
    #[serde(default)]
    pub spec_max: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    InRange,
    Below,
    Above,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluated {
    pub step: String,
    pub text: String,
    pub choice: Option<String>,
    pub measured: Option<f64>,
    pub value_source: Option<ValueSource>,
    pub unit: Option<String>,
    pub spec: Option<Range>,
    pub comparison: Option<Comparison>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidedState {
    pub procedure_id: String,
    pub current: Option<Step>,
    pub history: Vec<Evaluated>,
    pub outcome: Option<Outcome>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct File {
    format: String,
    procedures: Vec<Procedure>,
}

const BUNDLED: &str = include_str!("../data/procedures.json");

pub fn bundled() -> Vec<Procedure> {
    load(BUNDLED).expect("bundled procedures are valid")
}

pub fn load(json: &str) -> Result<Vec<Procedure>, String> {
    let f: File = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if f.format != "opendiag-procedures" {
        return Err(format!("unknown format {}", f.format));
    }
    for p in &f.procedures {
        validate(p).map_err(|e| format!("{}: {}", p.id, e.join("; ")))?;
    }
    Ok(f.procedures)
}

/// Checks a procedure's structure: every link leads somewhere, measurement
/// steps have a specification, outcomes exist, sources are given.
pub fn validate(p: &Procedure) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    let ids: HashMap<&str, &Step> = p.steps.iter().map(|s| (s.id.as_str(), s)).collect();
    if ids.len() != p.steps.len() {
        errs.push("duplicate step ids".into());
    }
    if !ids.contains_key(p.start.as_str()) {
        errs.push(format!("start step {} missing", p.start));
    }
    if p.source.trim().is_empty() {
        errs.push("source is required".into());
    }
    for s in &p.steps {
        let need: &[&str] = match s.kind {
            StepKind::Question => &["yes", "no"],
            StepKind::Info => &["ok"],
            StepKind::Measurement => &["in_range", "below", "above"],
            StepKind::Outcome => &[],
        };
        for k in need {
            match s.next.get(*k) {
                Some(t) if ids.contains_key(t.as_str()) => {}
                Some(t) => errs.push(format!("{}: next.{k} → unknown step {t}", s.id)),
                None => errs.push(format!("{}: next.{k} missing", s.id)),
            }
        }
        if s.kind == StepKind::Measurement {
            match &s.measurement {
                None => errs.push(format!("{}: measurement missing", s.id)),
                Some(m) => {
                    let has =
                        m.spec.is_some() as u8 + m.relative_to.is_some() as u8 + m.user_spec as u8;
                    if has != 1 {
                        errs.push(format!(
                            "{}: exactly one of spec / relative_to / user_spec",
                            s.id
                        ));
                    }
                    if let Some(r) = &m.relative_to {
                        if ids.get(r.as_str()).map(|x| x.kind) != Some(StepKind::Measurement) {
                            errs.push(format!(
                                "{}: relative_to {r} is not a measurement step",
                                s.id
                            ));
                        }
                    }
                    if m.spec.is_none() && m.source.as_deref().unwrap_or("").is_empty() {
                        errs.push(format!("{}: specification source missing", s.id));
                    }
                    if let Some(r) = &m.spec {
                        if r.min > r.max || r.source.is_empty() {
                            errs.push(format!("{}: bad spec", s.id));
                        }
                    }
                }
            }
        }
        if s.kind == StepKind::Outcome
            && !s
                .outcome
                .as_ref()
                .map(|o| p.outcomes.contains_key(o))
                .unwrap_or(false)
        {
            errs.push(format!("{}: outcome missing", s.id));
        }
        if s.kind != StepKind::Outcome && s.text.is_none() {
            errs.push(format!("{}: text missing", s.id));
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

/// Replays the answers and returns where the user is now.
pub fn evaluate(p: &Procedure, answers: &[Answer]) -> GuidedState {
    let steps: HashMap<&str, &Step> = p.steps.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut state = GuidedState {
        procedure_id: p.id.clone(),
        current: None,
        history: Vec::new(),
        outcome: None,
        error: None,
    };
    let mut values: HashMap<String, f64> = HashMap::new();
    let mut cur = steps.get(p.start.as_str()).copied();
    let mut answers = answers.iter();
    // A cycle guard: procedures are small.
    for _ in 0..200 {
        let Some(step) = cur else { break };
        if step.kind == StepKind::Outcome {
            state.outcome = step
                .outcome
                .as_ref()
                .and_then(|o| p.outcomes.get(o))
                .cloned();
            return state;
        }
        let Some(a) = answers.next() else {
            state.current = Some(step.clone());
            return state;
        };
        if a.step != step.id {
            state.error = Some(format!(
                "Expected an answer for \"{}\", got \"{}\".",
                step.id, a.step
            ));
            state.current = Some(step.clone());
            return state;
        }
        let text = step.text.clone().unwrap_or_default();
        let key = match step.kind {
            StepKind::Info => {
                state.history.push(ev(step, text, Some("ok".into())));
                "ok".to_string()
            }
            StepKind::Question => match a.choice.as_deref() {
                Some(c @ ("yes" | "no")) => {
                    state.history.push(ev(step, text, Some(c.into())));
                    c.to_string()
                }
                _ => {
                    state.error = Some("Answer yes or no.".into());
                    state.current = Some(step.clone());
                    return state;
                }
            },
            StepKind::Measurement => {
                let m = step.measurement.as_ref().expect("validated");
                let Some(v) = a.value.filter(|v| v.is_finite()) else {
                    state.error = Some("Enter a number.".into());
                    state.current = Some(step.clone());
                    return state;
                };
                let range = if let Some(r) = &m.spec {
                    r.clone()
                } else if let Some(rel) = &m.relative_to {
                    let Some(base) = values.get(rel) else {
                        state.error = Some(format!("Step {rel} has no measurement yet."));
                        state.current = Some(step.clone());
                        return state;
                    };
                    Range {
                        min: base - m.max_below.unwrap_or(0.0),
                        max: base + m.max_above.unwrap_or(0.0),
                        kind: SpecKind::Derived,
                        source: m.source.clone().unwrap_or_default(),
                    }
                } else {
                    match (a.spec_min, a.spec_max) {
                        (Some(lo), Some(hi)) if lo <= hi => Range {
                            min: lo,
                            max: hi,
                            kind: SpecKind::Documented,
                            source: m.source.clone().unwrap_or_else(|| "Entered by you".into()),
                        },
                        _ => {
                            state.error = Some(
                                "Enter the minimum and maximum from your service manual.".into(),
                            );
                            state.current = Some(step.clone());
                            return state;
                        }
                    }
                };
                let cmp = if v < range.min {
                    Comparison::Below
                } else if v > range.max {
                    Comparison::Above
                } else {
                    Comparison::InRange
                };
                let qualifier = match range.kind {
                    SpecKind::Documented => "your documented specification",
                    SpecKind::GeneralGuidance => {
                        "the general guidance range (not a manufacturer value)"
                    }
                    SpecKind::Derived => "the range worked out from your earlier measurement",
                };
                let explanation = match cmp {
                    Comparison::InRange => format!(
                        "{v} {} is within {qualifier} ({}–{} {}).",
                        m.unit,
                        fmt(range.min),
                        fmt(range.max),
                        m.unit
                    ),
                    Comparison::Below => format!(
                        "{v} {} is below {qualifier} ({}–{} {}).",
                        m.unit,
                        fmt(range.min),
                        fmt(range.max),
                        m.unit
                    ),
                    Comparison::Above => format!(
                        "{v} {} is above {qualifier} ({}–{} {}).",
                        m.unit,
                        fmt(range.min),
                        fmt(range.max),
                        m.unit
                    ),
                };
                values.insert(step.id.clone(), v);
                state.history.push(Evaluated {
                    step: step.id.clone(),
                    text,
                    choice: None,
                    measured: Some(v),
                    value_source: Some(a.value_source.unwrap_or(ValueSource::MeasuredByUser)),
                    unit: Some(m.unit.clone()),
                    spec: Some(range),
                    comparison: Some(cmp),
                    explanation: Some(explanation),
                });
                match cmp {
                    Comparison::InRange => "in_range",
                    Comparison::Below => "below",
                    Comparison::Above => "above",
                }
                .to_string()
            }
            StepKind::Outcome => unreachable!(),
        };
        cur = step
            .next
            .get(&key)
            .and_then(|n| steps.get(n.as_str()).copied());
    }
    state.error = Some("The procedure did not finish (loop or missing step).".into());
    state
}

fn ev(step: &Step, text: String, choice: Option<String>) -> Evaluated {
    Evaluated {
        step: step.id.clone(),
        text,
        choice,
        measured: None,
        value_source: None,
        unit: None,
        spec: None,
        comparison: None,
        explanation: None,
    }
}

fn fmt(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.2}").trim_end_matches('0').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(step: &str, choice: Option<&str>, value: Option<f64>) -> Answer {
        Answer {
            step: step.into(),
            choice: choice.map(Into::into),
            value,
            value_source: None,
            spec_min: None,
            spec_max: None,
        }
    }

    #[test]
    fn bundled_procedures_validate() {
        let ps = bundled();
        assert_eq!(ps.len(), 3);
    }

    #[test]
    fn heated_grip_open_element() {
        let p = bundled()
            .into_iter()
            .find(|p| p.id == "heated-grips-not-working")
            .unwrap();
        let s = evaluate(&p, &[]);
        assert_eq!(s.current.unwrap().id, "confirm");
        let mut answers = vec![
            a("confirm", Some("yes"), None),
            a("warnings", Some("no"), None),
            a("battery", None, Some(14.1)),
            a("fuse", Some("yes"), None),
            a("supply", None, Some(13.6)),
        ];
        let s = evaluate(&p, &answers);
        assert_eq!(s.current.as_ref().unwrap().id, "element");
        let supply = &s.history[4];
        assert_eq!(supply.comparison, Some(Comparison::InRange));
        assert_eq!(supply.spec.as_ref().unwrap().kind, SpecKind::Derived);
        // Element: user enters the manual's spec.
        let mut el = a("element", None, Some(9999.0));
        let s = evaluate(&p, &[answers.clone(), vec![el.clone()]].concat());
        assert!(s.error.unwrap().contains("service manual"));
        el.spec_min = Some(3.0);
        el.spec_max = Some(5.0);
        answers.push(el);
        let s = evaluate(&p, &answers);
        let o = s.outcome.unwrap();
        assert_eq!(o.kind, OutcomeKind::Finding);
        assert!(o.text.contains("broken"));
        assert_eq!(
            s.history.last().unwrap().spec.as_ref().unwrap().kind,
            SpecKind::Documented
        );
    }

    #[test]
    fn low_supply_is_a_hypothesis() {
        let p = bundled()
            .into_iter()
            .find(|p| p.id == "heated-grips-not-working")
            .unwrap();
        let s = evaluate(
            &p,
            &[
                a("confirm", Some("yes"), None),
                a("warnings", Some("no"), None),
                a("battery", None, Some(14.0)),
                a("fuse", Some("yes"), None),
                a("supply", None, Some(4.0)),
            ],
        );
        assert_eq!(s.outcome.unwrap().kind, OutcomeKind::Hypothesis);
        assert!(s.history[2]
            .explanation
            .as_ref()
            .unwrap()
            .contains("general guidance"));
    }

    #[test]
    fn wrong_answers_are_reported() {
        let p = bundled()
            .into_iter()
            .find(|p| p.id == "battery-and-charging")
            .unwrap();
        let s = evaluate(&p, &[a("running", None, Some(1.0))]);
        assert!(s.error.is_some());
        let s = evaluate(&p, &[a("rest", None, None)]);
        assert_eq!(s.error.as_deref(), Some("Enter a number."));
    }

    #[test]
    fn invalid_procedures_are_rejected() {
        let mut p = bundled().remove(0);
        p.steps[0].next.insert("yes".into(), "nowhere".into());
        assert!(validate(&p).is_err());
        let mut p = bundled().remove(1);
        p.source = String::new();
        assert!(validate(&p).is_err());
    }
}
