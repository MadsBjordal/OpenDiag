//! The safety policy engine.
//!
//! Every byte sequence that goes to a vehicle is classified here. Read-only
//! requests pass. Anything that can change the vehicle (even clearing codes)
//! needs an [`Authorization`], and an `Authorization` can only be created by
//! [`PolicyEngine`] after its checks pass: its fields are private, so drivers,
//! plugins, expert mode and the raw console cannot make one themselves.
//!
//! Level 4 (programming, security access, immobiliser…) is never authorized.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Level 0: reading information.
    ReadOnly = 0,
    /// Level 1: clearing codes, documented service resets.
    Maintenance = 1,
    /// Level 2: verified low-risk personalization.
    Personalization = 2,
    /// Level 3: calibrations, adaptations, actuator tests, ECU reset.
    AdvancedService = 3,
    /// Level 4: programming, security access, immobiliser. Never available.
    HighRisk = 4,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Classification {
    pub level: RiskLevel,
    pub what: &'static str,
    /// May be sent from the expert console without any workflow.
    pub raw_allowed: bool,
}

const fn c(level: RiskLevel, what: &'static str, raw_allowed: bool) -> Classification {
    Classification {
        level,
        what,
        raw_allowed,
    }
}

/// Classifies a request by its service id (OBD-II modes 01–0A, UDS ISO 14229 services).
/// Unknown services are treated as high risk.
pub fn classify(payload: &[u8]) -> Classification {
    use RiskLevel::*;
    let Some(&sid) = payload.first() else {
        return c(HighRisk, "Empty request", false);
    };
    match sid {
        0x01 => c(ReadOnly, "OBD: current data", true),
        0x02 => c(ReadOnly, "OBD: freeze frame data", true),
        0x03 => c(ReadOnly, "OBD: stored trouble codes", true),
        0x04 => c(
            Maintenance,
            "OBD: clear trouble codes and emissions data",
            false,
        ),
        0x05 => c(ReadOnly, "OBD: oxygen sensor test results", true),
        0x06 => c(ReadOnly, "OBD: on-board monitoring test results", true),
        0x07 => c(ReadOnly, "OBD: pending trouble codes", true),
        0x08 => c(
            AdvancedService,
            "OBD: control of on-board system (actuation)",
            false,
        ),
        0x09 => c(ReadOnly, "OBD: vehicle information", true),
        0x0A => c(ReadOnly, "OBD: permanent trouble codes", true),
        0x10 => match payload.get(1).map(|b| b & 0x7F) {
            Some(0x01) => c(ReadOnly, "UDS: default session", true),
            Some(0x03) => c(Maintenance, "UDS: extended diagnostic session", false),
            Some(0x02) => c(HighRisk, "UDS: programming session", false),
            _ => c(HighRisk, "UDS: unknown session", false),
        },
        0x11 => c(AdvancedService, "UDS: ECU reset", false),
        0x14 => c(Maintenance, "UDS: clear diagnostic information", false),
        0x19 => c(ReadOnly, "UDS: read DTC information", true),
        0x22 => c(ReadOnly, "UDS: read data by identifier", true),
        0x23 => c(HighRisk, "UDS: read memory by address", false),
        0x24 => c(ReadOnly, "UDS: read scaling data", true),
        0x27 => c(HighRisk, "UDS: security access", false),
        0x28 => c(AdvancedService, "UDS: communication control", false),
        0x29 => c(HighRisk, "UDS: authentication", false),
        0x2A => c(ReadOnly, "UDS: read data periodically", false),
        0x2C => c(
            Maintenance,
            "UDS: dynamically define data identifier",
            false,
        ),
        0x2E => c(Personalization, "UDS: write data by identifier", false),
        0x2F => c(AdvancedService, "UDS: input/output control", false),
        0x31 => c(AdvancedService, "UDS: routine control", false),
        0x34..=0x38 => c(HighRisk, "UDS: data transfer (programming)", false),
        0x3D => c(HighRisk, "UDS: write memory by address", false),
        0x3E => c(ReadOnly, "UDS: tester present", true),
        0x85 => c(AdvancedService, "UDS: control DTC setting", false),
        0x87 => c(AdvancedService, "UDS: link control", false),
        _ => c(HighRisk, "Unknown service", false),
    }
}

/// Proof that the policy engine allowed one specific request.
/// Fields are private: only [`PolicyEngine`] creates these.
#[derive(Debug, Clone)]
pub struct Authorization {
    payload: Vec<u8>,
    level: RiskLevel,
}

impl Authorization {
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    pub fn level(&self) -> RiskLevel {
        self.level
    }
    /// True when this authorization covers exactly `payload`.
    pub fn covers(&self, payload: &[u8]) -> bool {
        self.payload == payload
    }
}

/// Is `payload` allowed to be sent? Drivers call this before every write.
pub fn may_send(payload: &[u8], auth: Option<&Authorization>) -> Result<(), String> {
    let class = classify(payload);
    if class.level == RiskLevel::ReadOnly {
        return Ok(());
    }
    match auth {
        Some(a)
            if a.covers(payload) && a.level == class.level && class.level < RiskLevel::HighRisk =>
        {
            Ok(())
        }
        _ => Err(format!(
            "Blocked by the safety policy: \"{}\" (level {}) needs an approved workflow.",
            class.what, class.level as u8
        )),
    }
}

/// What a clear-codes workflow knows when the user presses Confirm.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClearContext {
    pub terms_accepted: bool,
    /// The codes, freeze frame and readiness were read and saved before clearing.
    pub snapshot_saved: bool,
    /// The user confirmed this exact plan (and it has not expired).
    pub user_confirmed: bool,
    pub plan_expired: bool,
    /// Control module / adapter voltage, if known.
    pub battery_voltage: Option<f64>,
    /// Engine speed, if known. Clearing is done with the ignition on and the engine off.
    pub engine_rpm: Option<f64>,
    /// No communication errors in the last requests.
    pub link_healthy: bool,
}

/// Configuration changes (levels 2–4).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigContext {
    pub terms_accepted: bool,
    /// The user turned on the vehicle-configuration subsystem in Settings.
    pub configuration_enabled: bool,
    /// The definition for this exact ECU hardware/software is verified (not community-only).
    pub definition_verified: bool,
    pub ecu_identity_matches: bool,
    pub original_value_saved: bool,
    pub rollback_verified: bool,
    pub user_confirmed: bool,
    pub battery_voltage: Option<f64>,
    pub link_healthy: bool,
    pub simulated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub allowed: bool,
    /// Why it is blocked (every reason, so the user can fix them all at once).
    pub blockers: Vec<String>,
    /// Allowed, but the user should know.
    pub warnings: Vec<String>,
}

pub const MIN_VOLTAGE_CLEAR: f64 = 11.0;
pub const MIN_VOLTAGE_CONFIG: f64 = 12.2;

#[derive(Debug, Default)]
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate_clear(&self, ctx: &ClearContext) -> Decision {
        let mut blockers = Vec::new();
        let mut warnings = Vec::new();
        if !ctx.terms_accepted {
            blockers.push("Accept the risk notice in Settings before using functions that change the vehicle.".into());
        }
        if !ctx.snapshot_saved {
            blockers.push("The current codes could not be read and saved first.".into());
        }
        if ctx.plan_expired {
            blockers.push("This plan is too old. Read the codes again before clearing.".into());
        }
        if !ctx.user_confirmed {
            blockers.push("You have not confirmed the clear.".into());
        }
        if !ctx.link_healthy {
            blockers.push(
                "The connection to the adapter has had errors. Reconnect and try again.".into(),
            );
        }
        match ctx.engine_rpm {
            Some(rpm) if rpm > 0.0 => blockers.push(
                "The engine is running. Switch the engine off and leave the ignition on, then try again.".into(),
            ),
            None => warnings.push("Engine speed could not be read. Make sure the engine is off and the ignition is on.".into()),
            _ => {}
        }
        match ctx.battery_voltage {
            Some(v) if v < MIN_VOLTAGE_CLEAR => blockers.push(format!(
                "Battery voltage is {v:.1} V, too low for reliable communication (needs at least {MIN_VOLTAGE_CLEAR:.1} V)."
            )),
            None => warnings.push("Battery voltage is unknown.".into()),
            _ => {}
        }
        Decision {
            allowed: blockers.is_empty(),
            blockers,
            warnings,
        }
    }

    /// Authorizes the OBD clear request (service 04) once [`Self::evaluate_clear`] allows it.
    pub fn authorize_clear(&self, ctx: &ClearContext) -> Result<Authorization, Decision> {
        let d = self.evaluate_clear(ctx);
        if d.allowed {
            Ok(Authorization {
                payload: vec![0x04],
                level: RiskLevel::Maintenance,
            })
        } else {
            Err(d)
        }
    }

    pub fn evaluate_config(&self, level: RiskLevel, ctx: &ConfigContext) -> Decision {
        let mut blockers: Vec<String> = Vec::new();
        let warnings = Vec::new();
        if level == RiskLevel::HighRisk {
            blockers.push(
                "Level 4 operations (programming, security access, immobiliser, safety-critical changes) are not available in OpenDiag."
                    .into(),
            );
        }
        if level <= RiskLevel::Maintenance {
            blockers
                .push("Use the dedicated maintenance workflows for level 0–1 operations.".into());
        }
        let checks: [(bool, &str); 7] = [
            (ctx.terms_accepted, "Accept the risk notice in Settings first."),
            (ctx.configuration_enabled, "Vehicle configuration is turned off. Turn it on in Settings → Vehicle configuration."),
            (ctx.definition_verified, "No verified definition exists for this exact ECU hardware and software."),
            (ctx.ecu_identity_matches, "The ECU's identification does not match the definition."),
            (ctx.original_value_saved, "The original setting could not be read and saved."),
            (ctx.user_confirmed, "You have not confirmed this change."),
            (ctx.link_healthy, "The connection is not reliable enough for changes."),
        ];
        for (ok, why) in checks {
            if !ok {
                blockers.push(why.into());
            }
        }
        if level >= RiskLevel::AdvancedService && !ctx.rollback_verified {
            blockers.push("Level 3 operations need a verified way back for this ECU.".into());
        }
        match ctx.battery_voltage {
            Some(v) if v >= MIN_VOLTAGE_CONFIG => {}
            Some(v) => blockers.push(format!(
                "Battery voltage is {v:.1} V; configuration changes need at least {MIN_VOLTAGE_CONFIG:.1} V (connect a battery support unit)."
            )),
            None => blockers.push("Battery voltage must be known before changing the configuration.".into()),
        }
        Decision {
            allowed: blockers.is_empty(),
            blockers,
            warnings,
        }
    }

    /// Authorizes one configuration request. Phase 1 ships no configuration
    /// operations, so in practice this only ever runs in tests.
    pub fn authorize_config(
        &self,
        payload: &[u8],
        ctx: &ConfigContext,
    ) -> Result<Authorization, Decision> {
        let class = classify(payload);
        let d = self.evaluate_config(class.level, ctx);
        if d.allowed {
            Ok(Authorization {
                payload: payload.to_vec(),
                level: class.level,
            })
        } else {
            Err(d)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_clear() -> ClearContext {
        ClearContext {
            terms_accepted: true,
            snapshot_saved: true,
            user_confirmed: true,
            plan_expired: false,
            battery_voltage: Some(12.5),
            engine_rpm: Some(0.0),
            link_healthy: true,
        }
    }

    #[test]
    fn read_only_passes_without_authorization() {
        for p in [
            &[0x01, 0x0C][..],
            &[0x03],
            &[0x07],
            &[0x09, 0x02],
            &[0x0A],
            &[0x22, 0xF1, 0x90],
            &[0x19, 0x02, 0xFF],
        ] {
            assert!(may_send(p, None).is_ok(), "{p:?}");
        }
    }

    #[test]
    fn writes_are_blocked_without_authorization() {
        for p in [
            &[0x04][..],
            &[0x14, 0xFF, 0xFF, 0xFF],
            &[0x2E, 0x01, 0x00, 0x01],
            &[0x27, 0x01],
            &[0x31, 0x01],
            &[0x34],
            &[0x10, 0x02],
            &[0x08],
            &[0x99],
            &[],
        ] {
            assert!(may_send(p, None).is_err(), "{p:?}");
        }
    }

    #[test]
    fn clear_needs_every_check() {
        let p = PolicyEngine::new();
        let auth = p.authorize_clear(&ok_clear()).unwrap();
        assert!(may_send(&[0x04], Some(&auth)).is_ok());
        // The authorization is for service 04 only.
        assert!(may_send(&[0x14, 0xFF, 0xFF, 0xFF], Some(&auth)).is_err());
        assert!(may_send(&[0x2E, 0x01], Some(&auth)).is_err());

        let mut c = ok_clear();
        c.engine_rpm = Some(780.0);
        assert!(p.authorize_clear(&c).is_err());
        let mut c = ok_clear();
        c.snapshot_saved = false;
        c.terms_accepted = false;
        let d = p.authorize_clear(&c).unwrap_err();
        assert_eq!(d.blockers.len(), 2);
        let mut c = ok_clear();
        c.battery_voltage = Some(10.2);
        assert!(p.authorize_clear(&c).is_err());
        let mut c = ok_clear();
        c.battery_voltage = None;
        c.engine_rpm = None;
        let d = p.evaluate_clear(&c);
        assert!(d.allowed);
        assert_eq!(d.warnings.len(), 2);
    }

    #[test]
    fn level_four_is_never_authorized() {
        let p = PolicyEngine::new();
        let ctx = ConfigContext {
            terms_accepted: true,
            configuration_enabled: true,
            definition_verified: true,
            ecu_identity_matches: true,
            original_value_saved: true,
            rollback_verified: true,
            user_confirmed: true,
            battery_voltage: Some(13.5),
            link_healthy: true,
            simulated: false,
        };
        for p4 in [&[0x27, 0x01][..], &[0x34, 0x00], &[0x10, 0x02], &[0x3D]] {
            assert!(p.authorize_config(p4, &ctx).is_err());
        }
        // A verified level 2 change with every check passed is allowed…
        let a = p.authorize_config(&[0x2E, 0x01, 0x02, 0x01], &ctx).unwrap();
        assert!(may_send(&[0x2E, 0x01, 0x02, 0x01], Some(&a)).is_ok());
        assert!(may_send(&[0x2E, 0x01, 0x02, 0x00], Some(&a)).is_err());
        // …but not when the subsystem is off.
        let off = ConfigContext {
            configuration_enabled: false,
            ..ctx.clone()
        };
        assert!(p.authorize_config(&[0x2E, 0x01, 0x02, 0x01], &off).is_err());
        let low = ConfigContext {
            battery_voltage: Some(11.9),
            ..ctx
        };
        assert!(p.authorize_config(&[0x2E, 0x01, 0x02, 0x01], &low).is_err());
    }
}
