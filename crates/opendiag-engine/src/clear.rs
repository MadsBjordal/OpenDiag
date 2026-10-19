//! Clearing emissions-related fault codes (OBD service 04), the only vehicle-
//! changing function in Phase 1.
//!
//! 1. [`Engine::prepare_clear`] reads everything that clearing would erase,
//!    saves it, and returns a plan that says exactly what will happen.
//! 2. The user confirms that plan.
//! 3. [`Engine::execute_clear`] re-checks the conditions through the safety
//!    policy, sends service 04 only with the policy's authorization, then reads
//!    the codes again to show what really happened.

use crate::report::{DtcFinding, DtcKind, FreezeFrame};
use crate::scan::{read_all_dtcs, read_freeze_frames, read_readiness};
use crate::{read_pid, Engine, EngineError, Result};
use opendiag_obd::{ecu_label, response::NegativeResponse};
use opendiag_safety::{ClearContext, Decision};
use opendiag_store::compat::TestResult;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const PLAN_LIFETIME: Duration = Duration::from_secs(300);

pub(crate) struct PendingPlan {
    pub id: String,
    pub created: Instant,
    pub snapshot_id: i64,
    pub session_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearPlan {
    pub id: String,
    pub simulated: bool,
    pub expires_in_seconds: u64,
    /// Codes that will be erased.
    pub codes: Vec<DtcFinding>,
    /// Permanent codes: these stay until the vehicle's own self-test passes.
    pub permanent: Vec<DtcFinding>,
    /// Modules the clear request reaches (every emissions-related module that answers).
    pub affected_modules: Vec<(String, String)>,
    pub freeze_frames: Vec<FreezeFrame>,
    pub consequences: Vec<String>,
    /// What the safety policy would say right now if you confirmed.
    pub preview: Decision,
    pub snapshot_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcuClearResult {
    pub ecu: String,
    pub label: String,
    pub accepted: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearResult {
    pub performed: bool,
    pub decision: Decision,
    pub modules: Vec<EcuClearResult>,
    /// Read back after clearing.
    pub remaining: Vec<DtcFinding>,
    pub summary: String,
}

const CONSEQUENCES: [&str; 6] = [
    "Clearing codes does not repair anything. If the fault is still there, the code and warning light will come back.",
    "The freeze frame data (the conditions when the fault was stored) will be erased. OpenDiag has saved a copy.",
    "Readiness monitors will reset to \"not complete\". The vehicle may fail an emissions inspection until it has been driven enough for them to complete again.",
    "Fuel trim and other learned values may be reset on some vehicles, so the engine can run a little differently for a while.",
    "Permanent codes are not erased by a scan tool; they clear themselves after the vehicle's own test passes.",
    "The request goes to every emissions-related module that answers (listed above), not only the one you are looking at.",
];

impl Engine {
    pub fn prepare_clear(&mut self) -> Result<ClearPlan> {
        let terms = self.terms_accepted();
        let c = crate::vehicle_of(&mut self.conn)?;
        let simulated = c.simulated();
        let dtcs = read_all_dtcs(c);
        if !dtcs.errors.is_empty() {
            return Err(EngineError::Blocked(format!(
                "The current codes could not be read completely ({}), so they cannot be saved before clearing.",
                dtcs.errors.join("; ")
            )));
        }
        let mut errs = Vec::new();
        let readiness = read_readiness(c, &mut errs);
        let frames = read_freeze_frames(c, &mut errs);
        let rpm = read_pid(c, 0x0C).ok().map(|m| m.value);
        let voltage = c
            .elm
            .read_voltage()
            .or_else(|| read_pid(c, 0x42).ok().map(|m| m.value));
        let healthy = c.elm.consecutive_errors == 0;
        let session = c.session_id;
        let affected: Vec<(String, String)> = dtcs
            .answered
            .iter()
            .map(|e| (e.clone(), ecu_label(e)))
            .collect();
        let lib = &self.library;
        let to_finding = |(code, kind, ecu): &(String, DtcKind, String)| DtcFinding {
            code: code.clone(),
            ecu: ecu.clone(),
            ecu_label: ecu_label(ecu),
            kind: *kind,
            explanation: lib.explain(code),
        };
        let codes: Vec<DtcFinding> = dtcs
            .findings
            .iter()
            .filter(|f| f.1 != DtcKind::Permanent)
            .map(to_finding)
            .collect();
        let permanent: Vec<DtcFinding> = dtcs
            .findings
            .iter()
            .filter(|f| f.1 == DtcKind::Permanent)
            .map(to_finding)
            .collect();
        let snapshot = serde_json::json!({
            "codes": codes, "permanent": permanent, "freeze_frames": frames, "readiness": readiness,
            "engine_rpm": rpm, "voltage": voltage, "simulated": simulated,
        });
        let snapshot_id = self.store.save_clear_snapshot(session, &snapshot)?;
        let preview = self.policy.evaluate_clear(&ClearContext {
            terms_accepted: terms,
            snapshot_saved: true,
            user_confirmed: true,
            plan_expired: false,
            battery_voltage: voltage,
            engine_rpm: rpm,
            link_healthy: healthy,
        });
        let id = uuid::Uuid::new_v4().to_string();
        self.clear_plan = Some(PendingPlan {
            id: id.clone(),
            created: Instant::now(),
            snapshot_id,
            session_id: session,
        });
        Ok(ClearPlan {
            id,
            simulated,
            expires_in_seconds: PLAN_LIFETIME.as_secs(),
            codes,
            permanent,
            affected_modules: affected,
            freeze_frames: frames,
            consequences: CONSEQUENCES.iter().map(|s| s.to_string()).collect(),
            preview,
            snapshot_id,
        })
    }

    /// Runs a confirmed plan. Nothing is sent unless the safety policy authorizes it.
    pub fn execute_clear(&mut self, plan_id: &str, confirmed: bool) -> Result<ClearResult> {
        let plan = match self.clear_plan.take() {
            Some(p) if p.id == plan_id => p,
            other => {
                self.clear_plan = other;
                return Err(EngineError::Invalid(
                    "This clear plan is not current. Read the codes again.".into(),
                ));
            }
        };
        let terms = self.terms_accepted();
        let snapshot_saved = self.store.clear_snapshot(plan.snapshot_id)?.is_some();
        let c = crate::vehicle_of(&mut self.conn)?;
        if c.session_id != plan.session_id {
            return Err(EngineError::Invalid(
                "The connection changed since the plan was made. Read the codes again.".into(),
            ));
        }
        // Conditions are read again now: the engine may have been started since.
        let rpm = read_pid(c, 0x0C).ok().map(|m| m.value);
        let voltage = c
            .elm
            .read_voltage()
            .or_else(|| read_pid(c, 0x42).ok().map(|m| m.value));
        let ctx = ClearContext {
            terms_accepted: terms,
            snapshot_saved,
            user_confirmed: confirmed,
            plan_expired: plan.created.elapsed() > PLAN_LIFETIME,
            battery_voltage: voltage,
            engine_rpm: rpm,
            link_healthy: c.elm.consecutive_errors == 0,
        };
        let auth = match self.policy.authorize_clear(&ctx) {
            Ok(a) => a,
            Err(decision) => {
                let session = c.session_id;
                self.store.log_operation(
                    session,
                    "clear_blocked",
                    &serde_json::json!({ "blockers": decision.blockers }),
                )?;
                return Ok(ClearResult {
                    performed: false,
                    decision,
                    modules: vec![],
                    remaining: vec![],
                    summary: "Nothing was sent to the vehicle.".into(),
                });
            }
        };
        let decision = self.policy.evaluate_clear(&ctx);
        let c = crate::vehicle_of(&mut self.conn)?;
        let answers = c.elm.request(auth.payload(), Some(&auth));
        let mut modules = Vec::new();
        match &answers {
            Ok(msgs) => {
                for m in msgs {
                    let (accepted, message) = if m.data.first() == Some(&0x44) {
                        (true, "Codes cleared.".to_string())
                    } else if let Some(n) = NegativeResponse::parse(&m.data) {
                        (
                            false,
                            format!("Refused: {} (code {:#04X}).", n.meaning(), n.code),
                        )
                    } else {
                        (
                            false,
                            format!("Unexpected answer {}.", opendiag_core::hex(&m.data)),
                        )
                    };
                    modules.push(EcuClearResult {
                        ecu: m.ecu.clone(),
                        label: ecu_label(&m.ecu),
                        accepted,
                        message,
                    });
                }
            }
            Err(e) => modules.push(EcuClearResult {
                ecu: "-".into(),
                label: "All modules".into(),
                accepted: false,
                message: format!("No confirmation: {e}"),
            }),
        }
        // Read back what is there now.
        let after = read_all_dtcs(c);
        let lib = &self.library;
        let remaining: Vec<DtcFinding> = after
            .findings
            .iter()
            .map(|(code, kind, ecu)| DtcFinding {
                code: code.clone(),
                ecu: ecu.clone(),
                ecu_label: ecu_label(ecu),
                kind: *kind,
                explanation: lib.explain(code),
            })
            .collect();
        let accepted = modules.iter().filter(|m| m.accepted).count();
        let left_stored = remaining
            .iter()
            .filter(|r| r.kind != DtcKind::Permanent)
            .count();
        let summary = if accepted == 0 {
            "No module confirmed the clear.".to_string()
        } else if left_stored == 0 {
            format!("{accepted} module(s) cleared their codes. Permanent codes that remain clear themselves after the vehicle's own tests pass.")
        } else {
            format!("{accepted} module(s) confirmed, but {left_stored} code(s) are still reported: the fault may still be present.")
        };
        let session = c.session_id;
        self.store.log_operation(
            session,
            "clear_dtcs",
            &serde_json::json!({ "summary": summary, "modules": modules, "remaining": remaining.len(), "snapshot_id": plan.snapshot_id }),
        )?;
        self.record_result(
            "clear_emission_dtcs",
            if accepted > 0 {
                TestResult::Works
            } else {
                TestResult::Fails
            },
            &summary,
        );
        Ok(ClearResult {
            performed: true,
            decision,
            modules,
            remaining,
            summary,
        })
    }
}
