use rusqlite::Connection;

/// Ordered schema migrations. Never edit an applied one: add a new entry.
const MIGRATIONS: &[(i64, &str)] = &[(
    1,
    r#"
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE vehicles (
  id INTEGER PRIMARY KEY,
  label TEXT NOT NULL,
  vin TEXT UNIQUE,
  make TEXT, model TEXT, year INTEGER, kind TEXT,
  notes TEXT NOT NULL DEFAULT '',
  simulated INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

CREATE TABLE sessions (
  id INTEGER PRIMARY KEY,
  vehicle_id INTEGER REFERENCES vehicles(id) ON DELETE CASCADE,
  started_at TEXT NOT NULL,
  adapter_json TEXT NOT NULL,
  simulated INTEGER NOT NULL
);

CREATE TABLE operations (
  id INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  at TEXT NOT NULL,
  what TEXT NOT NULL,
  detail_json TEXT NOT NULL
);

CREATE TABLE clear_snapshots (
  id INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  data_json TEXT NOT NULL
);

CREATE TABLE reports (
  id INTEGER PRIMARY KEY,
  session_id INTEGER REFERENCES sessions(id) ON DELETE CASCADE,
  vehicle_id INTEGER REFERENCES vehicles(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  created_at TEXT NOT NULL,
  report_json TEXT NOT NULL,
  simulated INTEGER NOT NULL,
  dtc_count INTEGER NOT NULL
);

-- One row per tested combination (vehicle × ECU × adapter × OS × protocol × function × app version).
CREATE TABLE compat_records (
  uuid TEXT PRIMARY KEY,
  combo_key TEXT NOT NULL UNIQUE,
  make TEXT NOT NULL, model TEXT NOT NULL, year INTEGER,
  ecu_hw TEXT NOT NULL, ecu_sw TEXT NOT NULL,
  adapter TEXT NOT NULL, adapter_firmware TEXT NOT NULL,
  os TEXT NOT NULL, protocol TEXT NOT NULL,
  function TEXT NOT NULL,
  result TEXT NOT NULL,
  verification TEXT NOT NULL,
  app_version TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  notes TEXT NOT NULL,
  source TEXT NOT NULL,
  version INTEGER NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX compat_vehicle ON compat_records (make, model, year);
CREATE INDEX compat_adapter ON compat_records (adapter);

CREATE TABLE compat_history (
  id INTEGER PRIMARY KEY,
  uuid TEXT NOT NULL,
  version INTEGER NOT NULL,
  data_json TEXT NOT NULL,
  changed_at TEXT NOT NULL,
  change_note TEXT NOT NULL
);

CREATE TABLE compat_submissions (
  id INTEGER PRIMARY KEY,
  record_json TEXT NOT NULL,
  status TEXT NOT NULL,
  moderator_note TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  decided_at TEXT
);

CREATE TABLE compat_snapshots (
  id INTEGER PRIMARY KEY,
  snapshot_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  imported_at TEXT NOT NULL,
  source TEXT NOT NULL,
  sha256 TEXT NOT NULL,
  records INTEGER NOT NULL
);
"#,
)];

pub fn run(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL)")?;
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;
    for (version, sql) in MIGRATIONS {
        if *version > current {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![version, crate::now()],
            )?;
            tx.commit()?;
        }
    }
    Ok(())
}
