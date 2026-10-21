# Changelog

What changed in each OpenDiag build, newest first: **Added**, **Changed**,
**Fixed** and **Removed**. The installed app shows the newest section as
"What's new" when it offers an update. Downloads: <https://opendiag.stencilshot.com>.

Statuses such as "not hardware-verified" are explained in
[docs/compatibility.md](docs/compatibility.md).

## Unreleased

### Added
- **Suggest a feature** page: send a feature request or report a problem from inside the app. It becomes a public issue on GitHub (labelled "feature request" or "bug") that you can follow; no GitHub account needed.
- This changelog, and **What's new** in the update prompt.

## Test build 9 (2026-10-21)

### Fixed
- Sharing: the *Share this result…* button no longer does nothing silently when "standard or modified" has not been chosen; it says what to choose. *Send* shows "Sending…".
- Hardware test: a vehicle that reports no VIN is recorded as "not supported by this vehicle" instead of asking whether the VIN matches; a vehicle that stored no freeze frame is noted, not counted as a failure.

### Changed
- The community data service is pinned to its database, so updates of the service can never start an empty one.

## Test build 8 (2026-10-21)

### Added
- Descriptions for about 9,900 fault codes: the standard title of every generic code (9,075) and manufacturer-specific meanings for 31 makes (6,490), from the MIT-licensed dtc-database. Manufacturer meanings use the make from the VIN or My Garage and are marked "community dataset, not verified".
- Full explanations for the crankshaft and camshaft sensor codes (P0320–P0322, P0336–P0339, P0341–P0344), found on the first real scan.

## Test build 7 (2026-10-21)

### Added
- Silent updates on Windows: *Update now* installs in the background and reopens OpenDiag.

### Fixed
- The Vgate vLinker FS USB (and other adapters that answer some identification commands with "?") is accepted.

## Test build 6 (2026-10-21)

### Changed
- When no adapter answers, the error lists what arrived at each connection speed, and the speed check is more patient.

## Test build 5 (2026-10-21)

### Fixed
- USB adapters that do not call themselves "ELM" or "STN" are recognised (Windows and Android); 1 and 2 Mbit/s are tried as well.

## Test build 4 (2026-10-20)

### Added
- Sharing asks whether the vehicle is standard or modified; modified vehicles are counted separately in the community data.

## Test build 3 (2026-10-20)

### Fixed
- ELM327-compatible adapters that name themselves differently are accepted.

## Test build 2 (2026-10-20)

### Added
- Update prompt: the app checks GitHub for a newer build at start (can be switched off in Settings → Updates).
- Opt-in anonymous result sharing, with a preview of exactly what is sent and deletion in Settings → Sharing; community summaries ("Seen by others").

### Changed
- The Android app is signed with the project's own key and is about 23 MB instead of 156 MB. Uninstall Test build 1 once before installing.

## Test build 1 (2026-10-20)

First public test build.

### Added
- Generic OBD-II diagnostics: fault codes (stored, pending, permanent), readiness, freeze frame, live data, vehicle information.
- Clearing emissions codes with safety checks (engine off, codes saved first, explicit confirmation).
- Adapters: USB, Bluetooth Classic and Wi-Fi ELM327/STN adapters; raw K-line (KKL/FTDI) cables with fast and 5-baud wake-up; an experimental Honda motorcycle K-line mode.
- Trucks, buses and machines (SAE J1939): codes and engine values, read only.
- Reports (PDF, JSON, CSV, HTML), My Garage, guided troubleshooting, the compatibility database, the connector finder (54 connector types) and a blink-code reader.
- The hardware test assistant and a simulator for trying everything without a vehicle.
