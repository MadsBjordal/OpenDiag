# Backlog

Prioritised. Each item has measurable acceptance criteria. ✅ = done in this
repository (automated tests pass), 🔶 = done in code, waiting for hardware
verification, ⬜ = to do.

## Phase 1 — working foundation (Windows + Android)

Phase 1 is complete only when **both Windows and Android** pass all nine
criteria on real hardware (see [hardware-testing.md](hardware-testing.md)).

| # | Criterion | Windows | Android | Simulator |
| --- | --- | --- | --- | --- |
| 1 | Connect to a compatible physical OBD-II adapter | 🔶 serial (USB + paired BT COM), TCP | 🔶 USB host bridge, BT RFCOMM, TCP | ✅ |
| 2 | Identify available generic capabilities | 🔶 | 🔶 | ✅ |
| 3 | Read actual supported fault codes | 🔶 | 🔶 | ✅ |
| 4 | Read supported live measurements | 🔶 | 🔶 | ✅ |
| 5 | Clear applicable codes after explicit confirmation | 🔶 | 🔶 | ✅ |
| 6 | Save and reopen a report | 🔶 (app build not yet run on Windows) | 🔶 | ✅ |
| 7 | Work entirely offline | 🔶 (no network code paths) | 🔶 | ✅ |
| 8 | Same workflows against simulated vehicles | 🔶 | 🔶 | ✅ (4 profiles) |
| 9 | Display adapter/vehicle compatibility status | 🔶 | 🔶 | ✅ |

### Remaining Phase 1 work

| Item | Acceptance criteria |
| --- | --- |
| ⬜ Windows hardware run | All nine tests in hardware-testing.md pass with ≥ 1 USB and ≥ 1 Bluetooth adapter on Windows 10 or 11; results recorded in compatibility.md (Windows table). |
| ⬜ Android hardware run | Same with ≥ 1 USB-C (OTG) and ≥ 1 Bluetooth adapter on Android 10+; recorded in the Android table. |
| ⬜ Windows installer in CI | CI produces an NSIS/MSI installer; it installs and starts on a clean Windows 11 VM. |
| ⬜ Signed Android release APK | Release build signed with the project key; installs on Android 8–15. |
| ⬜ Browser end-to-end tests in CI | Playwright runs the simulator flow on every pull request. |

## Phase 2 — broad connectivity

| Item | Acceptance criteria |
| --- | --- |
| ⬜ BLE adapters | At least two adapter families with *documented* GATT services; discovery, pairing status, signal strength; tests with recorded BLE traffic. |
| ⬜ Wi-Fi discovery | Finds adapters on the current network without typing an address, where the adapter supports it. |
| ⬜ Reconnection | After a dropped link, the app reconnects or explains, within 10 s, without losing the report in progress. |
| ⬜ Adapter capability database | Known adapters (identity + firmware) with tested capabilities shipped as a snapshot; probe results compared against it. |
| 🔶 Raw K-line interface driver (FTDI/KKL cables) | Opens the cable with explicit wake-up control (5-baud, fast init, manufacturer wake-ups); tested with recorded traces. Prerequisite for most pre-Euro 5 motorcycles and older cars (see docs/connectors.md). |
| 🔶 Honda motorcycle K-line (4-pin DLC), wake-up + raw tables | Wake-up message confirmed on at least one real Honda bike (sources disagree); status per model in compatibility.md. |
| ⬜ Honda motorcycle fault codes over K-line | Only from a documented or reproducibly tested source; decoded per model; never guessed. |
| ⬜ Manufacturer K-line protocols (Suzuki SDS, Kawasaki KDS, VAG KW1281, Fiat/Marelli) | One at a time, each from a documented or open-source reference and tested on a real vehicle. |
| 🔶 J1939 read-only (DM1 codes, PGN requests) | Through ELM327 protocol A (done, tested against a simulated J1939 network; see docs/j1939.md) and raw CAN interfaces (not yet); one real truck or machine still to test. 500 kbit/s networks not yet. |
| ⬜ SocketCAN (Linux) and J2534 (Windows) drivers | Same engine workflows pass against a CAN interface + simulated ECU on a test bench. |
| ⬜ Multi-platform CI | Windows, Linux, macOS builds and tests on every pull request. |

## Phase 3 — extended diagnostics (read only)

| Item | Acceptance criteria |
| --- | --- |
| ⬜ ISO-TP over raw CAN | Flow control, block size, STmin, timeouts per ISO 15765-2; tests with recorded traces. |
| ⬜ UDS read services | 0x22, 0x19 (sub-functions 0x01, 0x02, 0x04, 0x06), 0x3E, default session only; response-pending handling; definitions describe DIDs with sources. |
| ⬜ Manufacturer modules | Modules from verified definitions appear in the scan; everything else stays "not scanned". |
| ⬜ Connector-aware vehicle definitions | Each definition names its connector and cable; the wizard warns before connecting when the connector needs a cable or another interface. |
| ⬜ Legacy simulator profiles | KWP2000 and J1850 profiles; parser tests with recorded traces from real vehicles. |
| ⬜ More vehicle profiles | Each with provenance; no fabricated data. |

## Phase 4 — community ecosystem

| Item | Acceptance criteria |
| --- | --- |
| ⬜ Plugin SDK (WebAssembly) | Sandboxed plugins with declared permissions; no filesystem/network/credentials/vehicle writes by default; signed; template + docs. |
| ⬜ Definition authoring tool | Edit, validate and test a definition against the simulator in the app. |
| ⬜ Community server (optional, opt-in) | Submissions, moderation, signed snapshots; the app works fully without it. |
| ⬜ Signed snapshots | Maintainer signatures; only signed snapshots may carry `maintainer_verified` / `hil_verified`. |

## Phase 5 — guided maintenance and customisation

Only documented, validated operations, each with exact ECU checks, a verified
rollback and hardware-in-the-loop evidence before release.

## Phase 6 — advanced capabilities

Investigation only (DoIP, commercial vehicles / J1939, manufacturer-authorised
workflows such as SERMI). Investigated is not the same as suitable for release.
