# Decisions

| Date | Decision | Why |
| --- | --- | --- |
| 2026-10-19 | Windows and Android are first-class at launch; Linux/macOS/iOS later (owner's confirmed architecture). | Owner's priority. |
| 2026-10-19 | Tauri 2 + React/TypeScript UI, Rust engine. | One UI codebase for Windows and Android; hardware code in Rust; small installers. Flutter + FFI was the alternative (see architecture.md). |
| 2026-10-19 | Synchronous engine with timeouts, run on a worker thread. | ELM327-class adapters are strict request/response devices. Simpler to reason about, test and port. |
| 2026-10-19 | Android USB and Bluetooth Classic through a small Kotlin plugin moving bytes; the ELM327 driver stays in Rust. | Android USB host and Bluetooth APIs are Java-only; keeping protocol logic in one place keeps behaviour identical on both platforms. |
| 2026-10-19 | `usb-serial-for-android` (MIT) for USB chips on Android. | Mature driver set for FTDI, CH34x, CP210x, PL2303, CDC-ACM. |
| 2026-10-19 | BLE adapters: planned, not in Phase 1. | Each adapter family uses its own GATT service; we will only add documented ones. |
| 2026-10-19 | One PID per live-data request, at least 25 ms between requests. | Multi-PID requests are badly supported by clones; spacing protects slow adapters and buses. |
| 2026-10-19 | Clearing codes requires: accepted risk notice, a saved snapshot, an explicit confirmation of a plan less than 5 minutes old, engine off (rpm = 0 when readable), battery ≥ 11.0 V when readable, no recent link errors. | Software safety, not just a disclaimer. |
| 2026-10-19 | Compatibility verification is capped: local = `unverified_local`, approved submission = `community_tested`, imported snapshot ≤ `community_tested` until signed snapshots exist. | Nothing becomes "verified" by being copied. |
| 2026-10-19 | Licence: GPL-3.0-or-later for code; definitions and compatibility data CC0-1.0; contributions under the DCO. | See licensing.md. |
| 2026-10-19 | Reports export as PDF (built-in writer), JSON, CSV and HTML; VIN masked by default. | Offline on every platform without a PDF library; privacy by default. |
| 2026-10-19 | Connector catalog (`definitions/connectors.json`) with sources and confidence; low-confidence entries carry no pin table; no blink-code meanings shipped (the user copies them from their manual). | Many vehicles do not have the 16-pin socket; we must say so honestly without inventing pinouts. |
| 2026-10-19 | Next connectivity priority after Phase 1: a raw K-line interface driver. | It unlocks most pre-Euro 5 motorcycles (Honda, Suzuki, Kawasaki, Yamaha) and older cars. |
| 2026-10-19 | Raw K-line driver behind a shared `Driver` interface; OpenDiag drives fast init and 5-baud init itself through serial break. | The engine's workflows work unchanged over ELM327 and K-line cables; break control works on FTDI cables on Windows, Linux and Android. |
| 2026-10-19 | Honda motorcycle K-line only as an experimental, expert-only, read-only mode (wake-up + raw table reads) with its own allowlist (`may_send_honda`). | Only open-source sources exist and they disagree; no decoding until verified on a real bike. |
