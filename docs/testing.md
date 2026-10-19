# Automated tests

```sh
cargo test                  # all Rust crates (87 tests): drivers, protocols, simulator, safety, store, engine, API
pnpm --filter @opendiag/ui test        # UI unit tests (units, CSV, i18n)
pnpm --filter @opendiag/ui typecheck
```

| Area | Where | What is covered |
| --- | --- | --- |
| ISO-TP | `opendiag-isotp` | segmentation, reassembly, sequence wrap, out-of-order and malformed frames |
| OBD encoding/decoding | `opendiag-obd` | PID formulas, DTC bytes ↔ text, CAN vs legacy DTC responses, readiness (spark/diesel), VIN check digit and joining |
| ELM327 driver | `opendiag-elm327/src/tests.rs` | probe of genuine/clone adapters, non-ELM device, CAN 11/29-bit with and without spaces, multi-frame VIN, K-line headers + checksum, headers-off multi-frame, NO DATA / CAN ERROR / garbled / timeout, consecutive errors, writes blocked before reaching the adapter, undocumented AT commands refused |
| Simulator | `opendiag-sim` | ELM behaviour (echo, line feeds, spaces, headers, search, protocol numbers), clear while running refused, permanent codes survive, multi-ECU, K-line format, every fault mode, clone mode, profile validity |
| Safety policy | `opendiag-safety` | read-only allowlist, writes blocked, clear checks one by one, authorization bound to exact bytes, level 4 never authorized |
| Definitions | `opendiag-definitions` | unknown fields, provenance, mislabelled risk, level 4, unverified writes, ECU-confirmed vs vehicle-only matching |
| Store | `opendiag-store` | migrations idempotent, garage cascade delete, reports, settings, compatibility versions/history, moderation rules, snapshot import caps verification and ignores duplicates |
| Engine | `opendiag-engine/src/tests.rs` | full scans of all four simulated vehicles, saved/reopened reports, exports and VIN masking, the whole clear flow (notice, engine running, unconfirmed, single-use plan, low voltage, stale plan), comparison after clear, live data limits and sources, expert console read-only, ignition off + retry, truncated/malformed/disconnect faults, bad VIN and clone adapter warnings |
| API | `opendiag-api` | the whole workflow through JSON, error kinds |
| UI | `apps/ui/src/*.test.ts` | unit conversion, number formatting, CSV, i18n |

End-to-end checks (run during development, not yet in CI):

* `scripts/e2e-browser.mjs`: the UI against the development bridge with
  Playwright — notice → connect simulator → scan → clear plan → blocked clear →
  live data → phone layout → dark mode → compatibility → guided procedure.
* `scripts/e2e-tauri.py`: the **real desktop app** (Linux build) through
  tauri-driver/WebKitWebDriver — Tauri IPC to the engine, simulator connect,
  scan with progress events, PDF export written to disk, report in history.

Build checks done so far: Linux desktop app (debug), Android debug APK
(aarch64, including the Kotlin USB/Bluetooth bridge), Windows cross-compile of
all Rust code with MinGW (the CLI links; the full app installer is built with
MSVC in CI). The Android APK and Windows app have **not been run on a device yet**.

Real hardware: see [hardware-testing.md](hardware-testing.md). Hardware-in-the-loop
benches are planned before any function is marked "implemented and tested".
