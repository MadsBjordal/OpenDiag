# OpenDiag

**Free, open-source vehicle diagnostics.** Connect an affordable OBD-II adapter,
click *Connect*, scan the vehicle, understand the fault codes and save a report:
on **Windows** and **Android**, fully offline, without an account.

> **Status: Phase 1, pre-release (0.1.0).** Everything below is implemented and
> passes its automated tests against the built-in simulator. **None of it has been
> verified on real adapters and vehicles yet**: see
> [docs/compatibility.md](docs/compatibility.md). Please help by running the
> [hardware test protocol](docs/hardware-testing.md).

## What it does today

| | Status |
| --- | --- |
| Connect over USB/USB-C serial, Bluetooth Classic, Wi-Fi/Ethernet; simulator | Implemented, not hardware-verified |
| Identify ELM327 / STN (OBDLink) / clone adapters and what they really support | Implemented, not hardware-verified |
| Generic OBD-II scan: VIN, calibration IDs, stored / pending / permanent codes, freeze frame, readiness | Implemented, not hardware-verified |
| Plain-language explanations for ~100 common generic codes (offline) | Implemented |
| Live data with charts, units, refresh rate, CSV export | Implemented, not hardware-verified |
| Clearing codes, behind safety checks and explicit confirmation | Implemented, not hardware-verified |
| Reports: saved automatically, notes, compare two scans, PDF / JSON / CSV / HTML | Implemented |
| Guided troubleshooting (heated grips, battery and charging, lean mixture) | Implemented |
| Compatibility database: combination-level records, submissions, moderation, version history, offline snapshots | Implemented |
| Beginner and expert modes, dark/light theme, metric/imperial | Implemented |
| Connector finder: 54 connector types (cars, motorcycles, trucks, boats, machines, e-bikes) with sources, what each needs, and a blink-code reader | Implemented (see docs/connectors.md) |
| Manufacturer modules (ABS, airbag, body…) | Planned (Phase 3) |
| Bluetooth Low Energy adapters | Planned (Phase 2) |
| Vehicle configuration / personalization | Separate subsystem, off by default; no verified operations exist |
| ECU programming, immobiliser, security bypass | Not offered, by design |

OpenDiag never claims a vehicle is "healthy" when systems were not scanned, and
never marks a function as working just because the protocol could support it.

## Try it without a car

```sh
pnpm install
pnpm build:ui
cargo run -p opendiag-cli -- serve --ui apps/ui/dist     # then open http://127.0.0.1:7878
```

Choose **Connect → Simulator**. Every simulated result is labelled SIMULATED.
Or on the command line: `cargo run -p opendiag-cli -- scan sim:sim-car`.

## Install and build

See [docs/building.md](docs/building.md) for Windows and Android step by step.

## How it is built

A Rust diagnostic engine shared by every platform, a React/TypeScript UI in a
Tauri 2 shell, SQLite for local data, and a small Kotlin bridge for Android's
USB and Bluetooth APIs. See [docs/architecture.md](docs/architecture.md).

```
crates/      engine, drivers, protocols, simulator, safety, store, knowledge, CLI
apps/ui      React UI            apps/desktop  Tauri app (Windows, Linux, macOS, Android)
plugins/     Android USB/Bluetooth bridge     definitions/  community vehicle definitions
docs/        architecture, safety, compatibility, legal, testing, backlog
```

## Safety

Reading is low risk. Anything that changes the vehicle goes through a safety
policy enforced in code: see [docs/safety-policy.md](docs/safety-policy.md).
Use OpenDiag only on vehicles you own or may work on.

## Contributing

Code, translations, fault code explanations, procedures, vehicle definitions
and above all **real-hardware test results** are welcome:
[CONTRIBUTING.md](CONTRIBUTING.md). Security issues: [SECURITY.md](SECURITY.md).

## Licence

GPL-3.0-or-later ([LICENSE](LICENSE)). Definitions and compatibility data are
CC0-1.0. See [docs/licensing.md](docs/licensing.md). No warranty, to the extent
permitted by law.
