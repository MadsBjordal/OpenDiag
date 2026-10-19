//! The compatibility database: which function worked with which vehicle, ECU,
//! adapter, operating system, protocol and app version.
//!
//! Rules that keep it honest:
//! * Records made on this device are `simulated` or `unverified_local`.
//! * Community submissions go through moderation; approval makes them at most
//!   `community_tested`.
//! * Imported snapshots are capped at `community_tested` until signed snapshots
//!   exist (planned): nothing becomes "verified" by being copied around.

use crate::{now, Result, Store, StoreError};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    /// Run against the OpenDiag simulator only.
    Simulated,
    /// Seen on this device with real hardware, not reviewed.
    UnverifiedLocal,
    /// Reported by a community member, not reviewed.
    CommunityReported,
    /// Reviewed by moderators with evidence.
    CommunityTested,
    /// Reproduced by a maintainer.
    MaintainerVerified,
    /// Verified on a hardware-in-the-loop test bench.
    HilVerified,
}

impl Verification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Simulated => "simulated",
            Self::UnverifiedLocal => "unverified_local",
            Self::CommunityReported => "community_reported",
            Self::CommunityTested => "community_tested",
            Self::MaintainerVerified => "maintainer_verified",
            Self::HilVerified => "hil_verified",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(s.into())).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestResult {
    Works,
    Partial,
    Fails,
    NotSupported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompatRecord {
    #[serde(default)]
    pub uuid: String,
    pub make: String,
    pub model: String,
    pub year: Option<u16>,
    #[serde(default)]
    pub ecu_hw: String,
    #[serde(default)]
    pub ecu_sw: String,
    pub adapter: String,
    #[serde(default)]
    pub adapter_firmware: String,
    /// "windows", "android", "linux", "macos" (+ version when known).
    pub os: String,
    pub protocol: String,
    pub function: String,
    pub result: TestResult,
    pub verification: Verification,
    pub app_version: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub notes: String,
    /// "local", "submission", "snapshot:<id>".
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub updated_at: String,
}

impl CompatRecord {
    /// Records are unique per combination.
    pub fn combo_key(&self) -> String {
        [
            self.make.to_lowercase(),
            self.model.to_lowercase(),
            self.year.map(|y| y.to_string()).unwrap_or_default(),
            self.ecu_hw.to_lowercase(),
            self.ecu_sw.to_lowercase(),
            self.adapter.to_lowercase(),
            self.adapter_firmware.to_lowercase(),
            self.os.to_lowercase(),
            self.protocol.to_lowercase(),
            self.function.to_lowercase(),
            self.app_version.clone(),
        ]
        .join("|")
    }

    fn check(&self) -> Result<()> {
        for (name, v) in [("make", &self.make), ("adapter", &self.adapter), ("os", &self.os), ("function", &self.function), ("app_version", &self.app_version)] {
            if v.trim().is_empty() {
                return Err(StoreError::Invalid(format!("{name} is required")));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompatFilter {
    pub make: Option<String>,
    pub model: Option<String>,
    pub adapter: Option<String>,
    pub os: Option<String>,
    pub function: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    pub id: i64,
    pub record: CompatRecord,
    pub status: String,
    pub moderator_note: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub format: String,
    pub snapshot_id: String,
    pub created_at: String,
    pub records: Vec<CompatRecord>,
}

pub const SNAPSHOT_FORMAT: &str = "opendiag-compat-snapshot/1";

fn row_to_record(r: &rusqlite::Row) -> rusqlite::Result<CompatRecord> {
    let ev: String = r.get("evidence_json")?;
    let res: String = r.get("result")?;
    let ver: String = r.get("verification")?;
    Ok(CompatRecord {
        uuid: r.get("uuid")?,
        make: r.get("make")?,
        model: r.get("model")?,
        year: r.get("year")?,
        ecu_hw: r.get("ecu_hw")?,
        ecu_sw: r.get("ecu_sw")?,
        adapter: r.get("adapter")?,
        adapter_firmware: r.get("adapter_firmware")?,
        os: r.get("os")?,
        protocol: r.get("protocol")?,
        function: r.get("function")?,
        result: serde_json::from_value(serde_json::Value::String(res)).unwrap_or(TestResult::Partial),
        verification: Verification::parse(&ver).unwrap_or(Verification::UnverifiedLocal),
        app_version: r.get("app_version")?,
        evidence: serde_json::from_str(&ev).unwrap_or_default(),
        notes: r.get("notes")?,
        source: r.get("source")?,
        version: r.get("version")?,
        updated_at: r.get("updated_at")?,
    })
}

impl Store {
    /// Adds or updates the record for this combination. The previous version is kept in the history.
    pub fn record_compat(&self, rec: &CompatRecord, change_note: &str) -> Result<CompatRecord> {
        rec.check()?;
        let key = rec.combo_key();
        let existing: Option<CompatRecord> = self
            .conn
            .query_row("SELECT * FROM compat_records WHERE combo_key = ?1", [&key], row_to_record)
            .optional()?;
        let mut new = rec.clone();
        new.updated_at = now();
        match existing {
            Some(old) => {
                new.uuid = old.uuid.clone();
                new.version = old.version + 1;
                // Never downgrade a stronger verification by re-recording a weaker one.
                if old.verification > new.verification {
                    new.verification = old.verification;
                }
            }
            None => {
                new.uuid = uuid::Uuid::new_v4().to_string();
                new.version = 1;
            }
        }
        self.conn.execute(
            "INSERT INTO compat_records (uuid, combo_key, make, model, year, ecu_hw, ecu_sw, adapter, adapter_firmware, os,
               protocol, function, result, verification, app_version, evidence_json, notes, source, version, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)
             ON CONFLICT(combo_key) DO UPDATE SET result=excluded.result, verification=excluded.verification,
               evidence_json=excluded.evidence_json, notes=excluded.notes, source=excluded.source,
               version=excluded.version, updated_at=excluded.updated_at",
            params![
                new.uuid,
                key,
                new.make,
                new.model,
                new.year,
                new.ecu_hw,
                new.ecu_sw,
                new.adapter,
                new.adapter_firmware,
                new.os,
                new.protocol,
                new.function,
                serde_json::to_value(new.result)?.as_str().unwrap_or("partial"),
                new.verification.as_str(),
                new.app_version,
                serde_json::to_string(&new.evidence)?,
                new.notes,
                new.source,
                new.version,
                new.updated_at,
            ],
        )?;
        self.conn.execute(
            "INSERT INTO compat_history (uuid, version, data_json, changed_at, change_note) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new.uuid, new.version, serde_json::to_string(&new)?, new.updated_at, change_note],
        )?;
        Ok(new)
    }

    pub fn compat_records(&self, f: &CompatFilter) -> Result<Vec<CompatRecord>> {
        let mut st = self.conn.prepare(
            "SELECT * FROM compat_records
             WHERE (?1 IS NULL OR make LIKE ?1) AND (?2 IS NULL OR model LIKE ?2) AND (?3 IS NULL OR adapter LIKE ?3)
               AND (?4 IS NULL OR os LIKE ?4) AND (?5 IS NULL OR function = ?5)
             ORDER BY make, model, year, function",
        )?;
        let like = |v: &Option<String>| v.as_ref().filter(|s| !s.is_empty()).map(|s| format!("%{s}%"));
        let rows = st.query_map(
            params![like(&f.make), like(&f.model), like(&f.adapter), like(&f.os), f.function.as_ref().filter(|s| !s.is_empty())],
            row_to_record,
        )?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn compat_history(&self, uuid: &str) -> Result<Vec<(i64, String, String, CompatRecord)>> {
        let mut st = self
            .conn
            .prepare("SELECT version, changed_at, change_note, data_json FROM compat_history WHERE uuid = ?1 ORDER BY version")?;
        let rows = st.query_map([uuid], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (v, at, note, json) = row?;
            out.push((v, at, note, serde_json::from_str(&json)?));
        }
        Ok(out)
    }

    // ── community submissions and moderation ────────────────────────────────

    /// Queues a report for moderation. It does not count until approved.
    pub fn submit_compat(&self, rec: &CompatRecord) -> Result<i64> {
        rec.check()?;
        if rec.evidence.is_empty() {
            return Err(StoreError::Invalid("a submission needs evidence (log excerpt, photo reference, test notes)".into()));
        }
        let mut r = rec.clone();
        r.verification = Verification::CommunityReported;
        r.source = "submission".into();
        self.conn.execute(
            "INSERT INTO compat_submissions (record_json, status, created_at) VALUES (?1, 'pending', ?2)",
            params![serde_json::to_string(&r)?, now()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn submissions(&self, status: Option<&str>) -> Result<Vec<Submission>> {
        let mut st = self.conn.prepare(
            "SELECT id, record_json, status, moderator_note, created_at FROM compat_submissions
             WHERE ?1 IS NULL OR status = ?1 ORDER BY id DESC",
        )?;
        let rows = st.query_map([status], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, json, status, note, at) = row?;
            out.push(Submission { id, record: serde_json::from_str(&json)?, status, moderator_note: note, created_at: at });
        }
        Ok(out)
    }

    /// Moderation decision: "approve", "reject" or "needs_evidence".
    /// Approving records it as `community_tested`, never higher.
    pub fn moderate(&self, id: i64, decision: &str, note: &str) -> Result<Option<CompatRecord>> {
        let sub = self
            .submissions(None)?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| StoreError::Invalid("no such submission".into()))?;
        if sub.status != "pending" && sub.status != "needs_evidence" {
            return Err(StoreError::Invalid(format!("submission already {}", sub.status)));
        }
        let status = match decision {
            "approve" => "approved",
            "reject" => "rejected",
            "needs_evidence" => "needs_evidence",
            _ => return Err(StoreError::Invalid("decision must be approve, reject or needs_evidence".into())),
        };
        if status != "approved" && note.trim().is_empty() {
            return Err(StoreError::Invalid("say why in the note".into()));
        }
        self.conn.execute(
            "UPDATE compat_submissions SET status = ?2, moderator_note = ?3, decided_at = ?4 WHERE id = ?1",
            params![id, status, note, now()],
        )?;
        if status == "approved" {
            let mut r = sub.record;
            r.verification = Verification::CommunityTested;
            r.source = format!("submission:{id}");
            return Ok(Some(self.record_compat(&r, &format!("approved submission {id}: {note}"))?));
        }
        Ok(None)
    }

    // ── offline snapshots ───────────────────────────────────────────────────

    pub fn export_snapshot(&self) -> Result<String> {
        let snap = Snapshot {
            format: SNAPSHOT_FORMAT.into(),
            snapshot_id: uuid::Uuid::new_v4().to_string(),
            created_at: now(),
            records: self.compat_records(&CompatFilter::default())?,
        };
        Ok(serde_json::to_string_pretty(&snap)?)
    }

    /// Imports a snapshot file. Returns how many records were added or updated.
    pub fn import_snapshot(&self, json: &str, source: &str) -> Result<usize> {
        let snap: Snapshot = serde_json::from_str(json)?;
        if snap.format != SNAPSHOT_FORMAT {
            return Err(StoreError::Invalid(format!("unknown snapshot format {}", snap.format)));
        }
        let sha = hex::encode(Sha256::digest(json.as_bytes()));
        if self
            .conn
            .query_row("SELECT 1 FROM compat_snapshots WHERE sha256 = ?1", [&sha], |_| Ok(()))
            .optional()?
            .is_some()
        {
            return Ok(0);
        }
        let tx = self.conn.unchecked_transaction()?;
        let mut n = 0;
        for mut r in snap.records {
            if r.check().is_err() {
                continue;
            }
            // Until snapshots are signed, nothing imported is more than community-tested.
            if r.verification > Verification::CommunityTested {
                r.notes = format!("{} [imported as {}; capped until signed snapshots exist]", r.notes, r.verification.as_str()).trim().into();
                r.verification = Verification::CommunityTested;
            }
            r.source = format!("snapshot:{}", snap.snapshot_id);
            self.record_compat(&r, &format!("imported from snapshot {}", snap.snapshot_id))?;
            n += 1;
        }
        tx.execute(
            "INSERT INTO compat_snapshots (snapshot_id, created_at, imported_at, source, sha256, records) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![snap.snapshot_id, snap.created_at, now(), source, sha, n as i64],
        )?;
        tx.commit()?;
        Ok(n)
    }

    pub fn snapshots(&self) -> Result<Vec<serde_json::Value>> {
        let mut st = self
            .conn
            .prepare("SELECT snapshot_id, created_at, imported_at, source, records FROM compat_snapshots ORDER BY id DESC")?;
        let rows = st.query_map([], |r| {
            Ok(serde_json::json!({
                "snapshot_id": r.get::<_, String>(0)?,
                "created_at": r.get::<_, String>(1)?,
                "imported_at": r.get::<_, String>(2)?,
                "source": r.get::<_, String>(3)?,
                "records": r.get::<_, i64>(4)?,
            }))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

mod hex {
    pub fn encode(b: impl AsRef<[u8]>) -> String {
        b.as_ref().iter().map(|x| format!("{x:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec() -> CompatRecord {
        CompatRecord {
            uuid: String::new(),
            make: "Simulated".into(),
            model: "Generic car".into(),
            year: Some(2016),
            ecu_hw: String::new(),
            ecu_sw: "SIMCAL-CAR-0001".into(),
            adapter: "ELM327 v1.4b".into(),
            adapter_firmware: String::new(),
            os: "linux".into(),
            protocol: "can11_500".into(),
            function: "read_stored_dtcs".into(),
            result: TestResult::Works,
            verification: Verification::Simulated,
            app_version: "0.1.0".into(),
            evidence: vec![],
            notes: String::new(),
            source: "local".into(),
            version: 0,
            updated_at: String::new(),
        }
    }

    #[test]
    fn records_and_history() {
        let s = Store::open_in_memory().unwrap();
        let a = s.record_compat(&rec(), "first").unwrap();
        assert_eq!(a.version, 1);
        let mut r2 = rec();
        r2.result = TestResult::Partial;
        let b = s.record_compat(&r2, "second").unwrap();
        assert_eq!(b.uuid, a.uuid);
        assert_eq!(b.version, 2);
        assert_eq!(s.compat_history(&a.uuid).unwrap().len(), 2);
        // Another OS is another combination.
        let mut win = rec();
        win.os = "windows".into();
        s.record_compat(&win, "win").unwrap();
        assert_eq!(s.compat_records(&CompatFilter::default()).unwrap().len(), 2);
        assert_eq!(s.compat_records(&CompatFilter { os: Some("windows".into()), ..Default::default() }).unwrap().len(), 1);
    }

    #[test]
    fn submissions_need_evidence_and_moderation() {
        let s = Store::open_in_memory().unwrap();
        let mut r = rec();
        r.verification = Verification::HilVerified; // claims are ignored
        assert!(s.submit_compat(&r).is_err());
        r.evidence = vec!["log excerpt".into()];
        let id = s.submit_compat(&r).unwrap();
        assert!(s.compat_records(&CompatFilter::default()).unwrap().is_empty(), "nothing counts before approval");
        assert!(s.moderate(id, "reject", "").is_err(), "a reason is needed");
        let approved = s.moderate(id, "approve", "log looks right").unwrap().unwrap();
        assert_eq!(approved.verification, Verification::CommunityTested);
        assert!(s.moderate(id, "approve", "again").is_err());
    }

    #[test]
    fn snapshots_round_trip_and_cap_verification() {
        let a = Store::open_in_memory().unwrap();
        let mut r = rec();
        r.verification = Verification::UnverifiedLocal;
        a.record_compat(&r, "x").unwrap();
        let mut snap: serde_json::Value = serde_json::from_str(&a.export_snapshot().unwrap()).unwrap();
        snap["records"][0]["verification"] = serde_json::json!("hil_verified");
        let json = snap.to_string();
        let b = Store::open_in_memory().unwrap();
        assert_eq!(b.import_snapshot(&json, "file").unwrap(), 1);
        assert_eq!(b.import_snapshot(&json, "file").unwrap(), 0, "same file twice");
        let got = &b.compat_records(&CompatFilter::default()).unwrap()[0];
        assert_eq!(got.verification, Verification::CommunityTested);
        assert!(got.notes.contains("capped"));
        assert_eq!(b.snapshots().unwrap().len(), 1);
        assert!(b.import_snapshot(r#"{"format":"x","snapshot_id":"1","created_at":"","records":[]}"#, "f").is_err());
    }
}
