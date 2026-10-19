# OpenDiag architecture

This document answers steps 1–3 of the project brief: the practical limits of
"universal" diagnostics, the architecture, and the module boundaries and data
models. Decisions that changed the original brief are in
[decisions.md](decisions.md).

## 1. What "universal" can and cannot mean

**What a generic OBD adapter can reach.** Every petrol car sold in the EU since
2001 (diesel since 2004) and every US car since 1996 must expose
*emissions-related* diagnostics through the 16-pin connector using
SAE J1979 / ISO 15031 services (OBD-II / EOBD). Euro 5 motorcycles must have
on-board diagnostics too, but connector and protocol are often specific to the
manufacturer and need an adapter cable. That covers the engine module
and usually the transmission. It does **not** cover ABS, airbag, body, cluster,
climate, infotainment or battery management, which use manufacturer-specific
services (mostly UDS, ISO 14229) at manufacturer-specific addresses.

Limits that no software can remove:

| Limit | Consequence for OpenDiag |
| --- | --- |
| Manufacturer modules use undocumented data identifiers, routines and addresses. | Each one needs a definition with a legitimate source (public RMI, own research on own vehicle). Nothing is guessed. |
| Security gateways (e.g. after UN R155) block writes and sometimes reads without manufacturer authentication. | OpenDiag only uses legitimate, approved authentication. It never bypasses them. |
| Cheap ELM327 clones implement a subset of commands, often badly. | The driver probes what the adapter actually does, falls back gracefully, and records adapter-level compatibility. |
| ELM327-class adapters cannot do everything: no raw CAN at speed, limited flow control options, no DoIP. | Advanced functions declare required capabilities; professional interfaces (J2534, SocketCAN, DoIP) come later. |
| Many motorcycles have no 16-pin socket or use K-line/manufacturer protocols. | Vehicle definitions carry the connector, cable and protocol profile; generic OBD is attempted only where it exists. |
| A VIN identifies a model, not the exact ECU hardware/software. | Definitions match on ECU identification read from the vehicle; VIN-only matches are labelled as such. |
| Platform restrictions: iOS has no USB serial or Bluetooth SPP; Android needs runtime permissions; Windows exposes Bluetooth SPP as COM ports. | Transports are per platform behind one `ByteLink` trait. |

**So "universal" means:** one engine and app that work with any adapter and any
vehicle *to the extent that combination has been shown to work*, with an
explicit status per function, and an open way for the community to extend and
verify coverage.

## 2. Architecture

```
┌──────────────────────── UI (React + TypeScript) ───────────────────────┐
│  pages, components, i18n — identical on Windows, Linux, macOS, Android │
└───────────────┬─────────────────────────────────────┬──────────────────┘
                │ Tauri IPC (app)                     │ HTTP on 127.0.0.1 (dev bridge)
┌───────────────▼─────────────────────────────────────▼──────────────────┐
│ opendiag-api      JSON commands → engine (one code path for both)       │
├─────────────────────────────────────────────────────────────────────────┤
│ opendiag-engine   connect · scan · live data · clear (via safety) ·     │
│                   reports (PDF/JSON/CSV/HTML) · garage · compatibility  │
├──────────────┬───────────────┬──────────────┬──────────────┬───────────┤
│ opendiag-    │ opendiag-     │ opendiag-    │ opendiag-    │ opendiag- │
│ elm327       │ obd / isotp   │ safety       │ store        │ knowledge │
│ (driver)     │ (protocols)   │ (policy)     │ (SQLite)     │ definitions│
├──────────────┴───────────────┴──────────────┴──────────────┴───────────┤
│ opendiag-core     ByteLink trait · AdapterCapabilities · SupportStatus  │
├─────────────────────────────────────────────────────────────────────────┤
│ Byte links: serial (USB, Windows BT COM) · TCP (Wi-Fi/Ethernet) ·       │
│ Android USB host + Bluetooth (Kotlin plugin) · simulator                │
└─────────────────────────────────────────────────────────────────────────┘
```

**Technology choices**

* **Rust engine.** Memory safety for parsing untrusted bytes from adapters and
  community files, good cross-compilation (Windows MSVC, Android NDK, Apple),
  no runtime. The engine is synchronous: one adapter is a strictly
  request/response serial device, so threads + timeouts are simpler and more
  predictable than async here. The app runs it on a worker thread.
* **Tauri 2 + React/TypeScript.** One UI codebase for Windows and Android
  (Tauri 2 supports Android and iOS), using the system WebView (small installers).
  The alternative considered was Flutter + Rust FFI: it renders identically
  everywhere but adds a second language for UI and an FFI layer. With Tauri the
  hardware code stays in Rust and only the thin Android USB/Bluetooth bridge is Kotlin.
* **SQLite** (bundled through `rusqlite`), one file per install, versioned migrations.
* **Development bridge.** `opendiag serve` runs the same `Api` over HTTP on
  127.0.0.1 so the UI can be used and tested in a browser.

## 3. Modules and interfaces

| Crate / folder | Responsibility | Depends on |
| --- | --- | --- |
| `opendiag-core` | `ByteLink`, `LinkInfo`, `AdapterCapabilities`, `ObdProtocol`, `SupportStatus` | – |
| `opendiag-isotp` | ISO 15765-2 segmentation and reassembly | – |
| `opendiag-obd` | J1979 services: PID formulas, DTC encoding, readiness, VIN | core |
| `opendiag-safety` | Classifies every request (levels 0–4); issues the only `Authorization` drivers accept | – |
| `opendiag-elm327` | ELM327/STN driver: probe, protocol detection, parsing (CAN 11/29-bit, K-line, J1850, headers off) | core, isotp, obd, safety |
| `opendiag-transport` | Serial (USB, BT COM) and TCP links, device discovery | core |
| `opendiag-sim` | Simulated ELM327 + vehicles + fault injection | core, isotp, obd |
| `opendiag-knowledge` | DTC library, guided procedures (data + evaluation) | obd |
| `opendiag-definitions` | Community vehicle definitions: schema, validation, matching | core, safety |
| `opendiag-store` | SQLite: settings, garage, sessions, operations, reports, clear snapshots, compatibility DB | – |
| `opendiag-engine` | Workflows | all of the above |
| `opendiag-api` | JSON command interface, `LinkFactory` for platform links | engine |
| `opendiag-cli` | `opendiag` command line and development bridge | api |
| `apps/ui` | React UI | – |
| `apps/desktop/src-tauri` | Tauri app (Windows, Linux, macOS, Android) | api |
| `plugins/tauri-plugin-opendiag-transport` | Android USB host + Bluetooth Classic bridge (Kotlin) | api |
| `definitions/` | Community definitions (data only) | – |

### Key interfaces

```rust
// opendiag-core: every adapter connection, whatever the platform.
pub trait ByteLink: Send {
    fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError>;
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, LinkError>;
    fn info(&self) -> LinkInfo;
    fn close(&mut self) {}
}

// opendiag-safety: the only way a non-read request reaches a driver.
pub fn may_send(payload: &[u8], auth: Option<&Authorization>) -> Result<(), String>;
// Authorization has private fields: only PolicyEngine::authorize_* creates one.

// opendiag-api: platform-specific links (Android) plug in here.
pub trait LinkFactory: Send + Sync {
    fn devices(&self) -> Vec<DeviceCandidate>;
    fn open(&self, address: &str) -> Option<Result<Box<dyn ByteLink>, LinkError>>;
}
```

The UI ↔ engine contract is the `Cmd` enum in `crates/opendiag-api/src/lib.rs`
(JSON with `"cmd"`), mirrored by `apps/ui/src/types.ts`.

### Data models

* **ScanReport** (`opendiag-engine/src/report.rs`): vehicle identity (VIN
  decode, calibration IDs, ECU names, OBD standard, fuel), modules (scanned /
  partly / not scanned, with reasons), DTC findings (code, ECU, stored /
  pending / permanent, explanation), readiness per ECU, freeze frames,
  measurements (with source: vehicle / adapter / derived), warnings,
  communication errors, a health verdict that never claims "healthy", notes,
  operations performed, cautions.
* **CompatRecord** (`opendiag-store/src/compat.rs`): one tested *combination*:
  make, model, year, ECU hardware id, ECU software id, adapter + firmware, OS,
  protocol, function, result, verification level, app version, evidence,
  version history.
* **Definition** (`opendiag-definitions`): provenance, applicability (vehicle +
  exact ECU ids), adapter needs, functions (request, risk level, verification,
  evidence, rollback), contributors, semantic version.
* **Procedure** (`opendiag-knowledge/src/guided.rs`): steps (question / info /
  measurement / outcome), specifications tagged documented / general guidance /
  derived, outcomes tagged finding / hypothesis.

### Separation of diagnostics and configuration

Diagnostics (levels 0–1) and vehicle configuration (levels 2–4) are separate
subsystems. Configuration is off by default and has its own settings switch,
page and policy (`PolicyEngine::evaluate_config`). The type-level gate
(`Authorization`) means plugins, expert mode and the raw console cannot send a
changing request without the policy's approval. Level 4 is refused in code.

### Future platforms

Nothing in the engine is Windows- or Android-specific. Linux and macOS build
today (serial + TCP). iOS is possible with Tauri 2, limited to Wi-Fi adapters
and MFi/BLE adapters (no USB serial, no Bluetooth SPP on iOS).
