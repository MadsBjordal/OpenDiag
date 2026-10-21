# Compatibility

**Status words** (used everywhere in the app and the docs):

* **Implemented and tested**: automated tests pass *and* it was tested on real hardware on that platform.
* **Implemented but not hardware-verified**: automated tests (simulator) pass; nobody has confirmed it on real hardware yet.
* **Partially implemented**, **Simulated only**, **Planned**, **Unsupported**.

Nothing is listed as "tested" because a protocol is "theoretically supported".

## Functions per platform (app version 0.1.0)

| Function | Windows | Android | Linux |
| --- | --- | --- | --- |
| USB / USB-C serial adapter (ELM327-compatible) | Implemented and tested (vLinker FS USB, 2026-10-21) | Not hardware-verified (USB host bridge) | Not hardware-verified |
| Bluetooth Classic adapter | Not hardware-verified (paired COM port) | Not hardware-verified (RFCOMM bridge) | Partially (bind with `rfcomm` first) |
| Bluetooth Low Energy adapter | Planned | Planned | Planned |
| Raw K-line cable (KKL / FTDI), generic OBD-II, fast and 5-baud init | Not hardware-verified | Not hardware-verified (USB OTG) | Not hardware-verified |
| Honda motorcycle K-line, experimental (wake-up, raw table reads) | Partially implemented | Partially implemented | Partially implemented |
| Trucks, buses, machines (SAE J1939, 250 kbit/s): DM1/DM2 codes, read only | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Trucks, buses, machines (SAE J1939): engine values, hours, distance, VIN | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Wi-Fi / Ethernet (TCP) adapter | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| SAE J2534 PassThru | Planned | Unsupported | Unsupported |
| Adapter identification and capability probe | Implemented and tested (STN1170, 2026-10-21) | Not hardware-verified | Not hardware-verified |
| Generic OBD-II: VIN, calibration IDs, ECU names | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Stored, pending, permanent fault codes | Implemented and tested: stored codes on ISO 9141-2 (pending: none present; permanent: not supported by that car) | Not hardware-verified | Not hardware-verified |
| Freeze frame | Implemented and tested (ISO 9141-2) | Not hardware-verified | Not hardware-verified |
| Readiness monitors, MIL | Implemented and tested (ISO 9141-2) | Not hardware-verified | Not hardware-verified |
| Live data (standard PIDs) | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Clear emissions codes (with safety checks) | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Reports, history, compatibility database, guided diagnostics (no vehicle hardware involved) | Reports: implemented and tested (real scan saved and exported as JSON, 2026-10-21); the rest not verified yet | Not verified: the APK builds but has not been run on a phone yet | Implemented and tested (desktop app driven by WebDriver, browser tests) |
| Manufacturer modules (UDS, read only) | Planned | Planned | Planned |
| Vehicle configuration (levels 2–3) | Planned (no verified operations) | Planned | Planned |
| ECU programming, security access, immobiliser (level 4) | Unsupported | Unsupported | Unsupported |

All of the "not hardware-verified" functions pass their automated tests against
the simulator: CAN 11/29-bit and K-line framing through the ELM327 driver, and
ISO 9141-2 (5-baud), ISO 14230-4 (fast and 5-baud) and the Honda wake-up through
the raw K-line driver. J1850 framing is parsed but has no simulator profile yet.

## Verified hardware results

Kept **separately per operating system**. Add a row only after running the
[hardware test protocol](hardware-testing.md); attach the exported log.

### Windows

| Date | Adapter (+ firmware) | Link | Vehicle (make/model/year, ECU ids) | Protocol | Functions passed | Tester | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-10-21 | Vgate vLinker FS USB (STN1170 v4.3.2, reports "ELM327 v2.3"; FTDI 0403:6015) | USB, COM10 @ 115200 | Škoda, petrol, model/year not recorded; engine ECU 036906034KE 7450; no VIN reported | ISO 9141-2 (K-line) | Connect, adapter probe, stored codes (P0322, P0343), freeze frame, readiness (7 monitors), PIDs during scan, report export. *Partial: scan only; the hardware test protocol is still to be run.* | Owner | Report JSON (opendiag-report-1.json), Test build 7 |

### Android

| Date | Phone + Android version | Adapter (+ firmware) | Link | Vehicle | Protocol | Functions passed | Tester | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| – | *none yet* | | | | | | | |

## Adapter families

| Family | How detected | Notes |
| --- | --- | --- |
| Genuine ELM327 (v1.0–v2.x) | `ATI` contains "ELM327" | Documented command set. |
| STN chips (OBDLink and others) | `STI` answers `STN…` | ELM327-compatible plus ST commands (not used yet). |
| ELM327 clones | "v1.5" (never released by ELM Electronics), failing `ATH1`/`ATS0`/`ATRV` | Supported as far as they behave; missing features are shown, not assumed. |

OpenDiag only sends documented adapter commands (`ADAPTER_COMMANDS` in
`crates/opendiag-elm327/src/lib.rs`) and never stores settings in the adapter.

## USB chips seen in OBD adapters

FTDI (0403), WCH CH340/CH341 (1A86), Silicon Labs CP210x (10C4), Prolific
PL2303 (067B). On Windows the chip maker's driver may be needed; on Android
these are handled by `usb-serial-for-android`.
