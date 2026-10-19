//! Local storage in one SQLite file. Nothing here leaves the device unless the
//! user exports it.

pub mod compat;
mod migrations;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub struct Store {
    pub(crate) conn: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Vehicle {
    pub id: i64,
    pub label: String,
    pub vin: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<u16>,
    pub kind: Option<String>,
    pub notes: String,
    pub simulated: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportSummary {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub vehicle_id: Option<i64>,
    pub simulated: bool,
    pub dtc_count: i64,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| StoreError::Invalid(e.to_string()))?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let s = Self { conn };
        migrations::run(&s.conn)?;
        Ok(s)
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?)
    }

    // ── settings ────────────────────────────────────────────────────────────

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ── garage ──────────────────────────────────────────────────────────────

    /// Adds a vehicle, or returns the existing one with the same VIN.
    pub fn upsert_vehicle(&self, v: &Vehicle) -> Result<i64> {
        if let Some(vin) = &v.vin {
            if let Some(id) = self
                .conn
                .query_row("SELECT id FROM vehicles WHERE vin = ?1", [vin], |r| r.get::<_, i64>(0))
                .optional()?
            {
                return Ok(id);
            }
        }
        self.conn.execute(
            "INSERT INTO vehicles (label, vin, make, model, year, kind, notes, simulated, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![v.label, v.vin, v.make, v.model, v.year, v.kind, v.notes, v.simulated, now()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_vehicle(&self, v: &Vehicle) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE vehicles SET label = ?2, make = ?3, model = ?4, year = ?5, kind = ?6, notes = ?7 WHERE id = ?1",
            params![v.id, v.label, v.make, v.model, v.year, v.kind, v.notes],
        )?;
        if n == 0 {
            return Err(StoreError::Invalid("no such vehicle".into()));
        }
        Ok(())
    }

    pub fn vehicles(&self) -> Result<Vec<Vehicle>> {
        let mut st = self.conn.prepare(
            "SELECT id, label, vin, make, model, year, kind, notes, simulated, created_at FROM vehicles ORDER BY label",
        )?;
        let rows = st.query_map([], |r| {
            Ok(Vehicle {
                id: r.get(0)?,
                label: r.get(1)?,
                vin: r.get(2)?,
                make: r.get(3)?,
                model: r.get(4)?,
                year: r.get(5)?,
                kind: r.get(6)?,
                notes: r.get(7)?,
                simulated: r.get(8)?,
                created_at: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Deletes a vehicle with all its sessions, logs and reports.
    pub fn delete_vehicle(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM vehicles WHERE id = ?1", [id])?;
        Ok(())
    }

    // ── sessions and logs ───────────────────────────────────────────────────

    pub fn create_session(&self, vehicle_id: Option<i64>, adapter: &serde_json::Value, simulated: bool) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions (vehicle_id, started_at, adapter_json, simulated) VALUES (?1, ?2, ?3, ?4)",
            params![vehicle_id, now(), adapter.to_string(), simulated],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn set_session_vehicle(&self, session: i64, vehicle_id: i64) -> Result<()> {
        self.conn.execute("UPDATE sessions SET vehicle_id = ?2 WHERE id = ?1", params![session, vehicle_id])?;
        Ok(())
    }

    /// Records something done in a session (scan, clear, export…). Kept with the session.
    pub fn log_operation(&self, session: i64, what: &str, detail: &serde_json::Value) -> Result<()> {
        self.conn.execute(
            "INSERT INTO operations (session_id, at, what, detail_json) VALUES (?1, ?2, ?3, ?4)",
            params![session, now(), what, detail.to_string()],
        )?;
        Ok(())
    }

    pub fn operations(&self, session: i64) -> Result<Vec<(String, String, serde_json::Value)>> {
        let mut st = self.conn.prepare("SELECT at, what, detail_json FROM operations WHERE session_id = ?1 ORDER BY id")?;
        let rows = st.query_map([session], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (a, w, d) = row?;
            out.push((a, w, serde_json::from_str(&d)?));
        }
        Ok(out)
    }

    // ── clear snapshots ─────────────────────────────────────────────────────

    /// Saves what was on the vehicle right before a clear.
    pub fn save_clear_snapshot(&self, session: i64, data: &serde_json::Value) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO clear_snapshots (session_id, created_at, data_json) VALUES (?1, ?2, ?3)",
            params![session, now(), data.to_string()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn clear_snapshot(&self, id: i64) -> Result<Option<serde_json::Value>> {
        let s: Option<String> =
            self.conn.query_row("SELECT data_json FROM clear_snapshots WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        Ok(s.map(|s| serde_json::from_str(&s)).transpose()?)
    }

    // ── reports ─────────────────────────────────────────────────────────────

    pub fn save_report(
        &self,
        session: Option<i64>,
        vehicle_id: Option<i64>,
        title: &str,
        report: &serde_json::Value,
        simulated: bool,
        dtc_count: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO reports (session_id, vehicle_id, title, created_at, report_json, simulated, dtc_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![session, vehicle_id, title, now(), report.to_string(), simulated, dtc_count],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_report(&self, id: i64, report: &serde_json::Value) -> Result<()> {
        self.conn.execute("UPDATE reports SET report_json = ?2 WHERE id = ?1", params![id, report.to_string()])?;
        Ok(())
    }

    pub fn reports(&self, vehicle_id: Option<i64>) -> Result<Vec<ReportSummary>> {
        let mut st = self.conn.prepare(
            "SELECT id, title, created_at, vehicle_id, simulated, dtc_count FROM reports
             WHERE ?1 IS NULL OR vehicle_id = ?1 ORDER BY id DESC",
        )?;
        let rows = st.query_map([vehicle_id], |r| {
            Ok(ReportSummary {
                id: r.get(0)?,
                title: r.get(1)?,
                created_at: r.get(2)?,
                vehicle_id: r.get(3)?,
                simulated: r.get(4)?,
                dtc_count: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn report(&self, id: i64) -> Result<Option<serde_json::Value>> {
        let s: Option<String> =
            self.conn.query_row("SELECT report_json FROM reports WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        Ok(s.map(|s| serde_json::from_str(&s)).transpose()?)
    }

    pub fn delete_report(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM reports WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Deletes every vehicle, session, report and log (compatibility records are kept).
    pub fn delete_all_vehicle_data(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM reports; DELETE FROM clear_snapshots; DELETE FROM operations; DELETE FROM sessions; DELETE FROM vehicles;",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(vin: Option<&str>) -> Vehicle {
        Vehicle {
            id: 0,
            label: "Test car".into(),
            vin: vin.map(Into::into),
            make: Some("Test".into()),
            model: None,
            year: Some(2016),
            kind: Some("car".into()),
            notes: String::new(),
            simulated: true,
            created_at: String::new(),
        }
    }

    #[test]
    fn migrations_are_idempotent() {
        let dir = std::env::temp_dir().join(format!("opendiag-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("db.sqlite");
        let s = Store::open(&path).unwrap();
        let ver = s.schema_version().unwrap();
        assert!(ver >= 1);
        drop(s);
        let s = Store::open(&path).unwrap();
        assert_eq!(s.schema_version().unwrap(), ver);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn garage_reports_and_delete() {
        let s = Store::open_in_memory().unwrap();
        let id = s.upsert_vehicle(&v(Some("SMLTD0CAR00000001"))).unwrap();
        assert_eq!(s.upsert_vehicle(&v(Some("SMLTD0CAR00000001"))).unwrap(), id);
        s.upsert_vehicle(&v(None)).unwrap();
        assert_eq!(s.vehicles().unwrap().len(), 2);
        let sess = s.create_session(Some(id), &serde_json::json!({"identity":"x"}), true).unwrap();
        s.log_operation(sess, "scan", &serde_json::json!({"dtcs": 2})).unwrap();
        let r = s.save_report(Some(sess), Some(id), "Scan", &serde_json::json!({"a": 1}), true, 2).unwrap();
        assert_eq!(s.report(r).unwrap().unwrap()["a"], 1);
        assert_eq!(s.reports(Some(id)).unwrap()[0].dtc_count, 2);
        assert_eq!(s.operations(sess).unwrap().len(), 1);
        s.save_clear_snapshot(sess, &serde_json::json!({"codes": ["P0301"]})).unwrap();
        // Deleting the vehicle removes its sessions and reports.
        s.delete_vehicle(id).unwrap();
        assert!(s.report(r).unwrap().is_none());
        assert!(s.operations(sess).unwrap().is_empty());
    }

    #[test]
    fn settings() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.setting("mode").unwrap(), None);
        s.set_setting("mode", "beginner").unwrap();
        s.set_setting("mode", "expert").unwrap();
        assert_eq!(s.setting("mode").unwrap().as_deref(), Some("expert"));
    }
}
