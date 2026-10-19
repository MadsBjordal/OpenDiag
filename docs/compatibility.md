# Compatibility

**Status words** (used everywhere in the app and the docs):

* **Implemented and tested**: automated tests pass *and* it was tested on real hardware on that platform.
* **Implemented but not hardware-verified**: automated tests (simulator) pass; nobody has confirmed it on real hardware yet.
* **Partially implemented**, **Simulated only**, **Planned**, **Unsupported**.

Nothing is listed as "tested" because a protocol is "theoretically supported".

## Functions per platform (app version 0.1.0)

| Function | Windows | Android | Linux |
| --- | --- | --- | --- |
| USB / USB-C serial adapter (ELM327-compatible) | Not hardware-verified | Not hardware-verified (USB host bridge) | Not hardware-verified |
| Bluetooth Classic adapter | Not hardware-verified (paired COM port) | Not hardware-verified (RFCOMM bridge) | Partially (bind with `rfcomm` first) |
| Bluetooth Low Energy adapter | Planned | Planned | Planned |
| Wi-Fi / Ethernet (TCP) adapter | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| SAE J2534 PassThru | Planned | Unsupported | Unsupported |
| Adapter identification and capability probe | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Generic OBD-II: VIN, calibration IDs, ECU names | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Stored, pending, permanent fault codes | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Freeze frame | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Readiness monitors, MIL | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Live data (standard PIDs) | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Clear emissions codes (with safety checks) | Not hardware-verified | Not hardware-verified | Not hardware-verified |
| Reports, history, compatibility database, guided diagnostics (no vehicle hardware involved) | Not verified: the Windows app has not been run yet | Not verified: the APK builds but has not been run on a phone yet | Implemented and tested (desktop app driven by WebDriver, browser tests) |
| Manufacturer modules (UDS, read only) | Planned | Planned | Planned |
| Vehicle configuration (levels 2–3) | Planned (no verified operations) | Planned | Planned |
| ECU programming, security access, immobiliser (level 4) | Unsupported | Unsupported | Unsupported |

All of the "not hardware-verified" functions pass their automated tests against
the simulator on every supported vehicle protocol (CAN 11/29-bit, ISO 9141-2
K-line framing; J1850/KWP framing is parsed by the same code but has no
simulator profile yet).

## Verified hardware results

Kept **separately per operating system**. Add a row only after running the
[hardware test protocol](hardware-testing.md); attach the exported log.

### Windows

| Date | Adapter (+ firmware) | Link | Vehicle (make/model/year, ECU ids) | Protocol | Functions passed | Tester | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| – | *none yet* | | | | | | |

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
