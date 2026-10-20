# Hardware test protocol

Use this to verify a function on real hardware. Results go into
[compatibility.md](compatibility.md), **separately for Windows and Android**,
and into the app's compatibility database.

Only test vehicles you own or have permission to test, parked, on level
ground, with the parking brake on, in a ventilated place if the engine runs.

## Before you start

1. Note: OS + version (and phone model on Android), OpenDiag version (Settings → About),
   adapter brand/model, how it connects, and what the app shows as adapter identity.
2. Ignition on, engine off. Battery ≥ 12.2 V (measure, or read the voltage in the top bar).

## The easy way: the Hardware test page

Connect to the vehicle, then open **Hardware test** in the menu. It walks through
the tests below one by one: it runs the connection, scan, live-data and report
checks itself and asks you yes/no for the things only you can see (does the VIN
match the plate, do the codes match the warning lights, did unplugging hang the
app). Each result is saved in the compatibility database for your operating
system. At the end, **Copy report** gives a ready-made table to paste into a
"Compatibility report" issue on GitHub. A practice run with the simulator is
recorded as *simulated* and never counts as hardware verification.

## Tests

| # | Test | Pass when |
| --- | --- | --- |
| 1 | **Connect** (Connect → choose type → adapter → Connect) | Step 3 shows the adapter identity; Step 4 shows "Vehicle connected" and at least one module. |
| 2 | **Capabilities** | Step 3 chips match the adapter (headers yes/no, voltage). Clone warning shown if applicable. |
| 3 | **Compatibility status** | Compatibility → This connection lists statuses; nothing claims "tested" before you record it. |
| 4 | **Scan** (Scan Vehicle → Start scan) | Report appears; the VIN matches the vehicle's VIN plate; codes match what another tool or the dashboard shows. Note any communication problems listed. |
| 5 | **Live data** (Live Data → Start) | RPM, coolant temperature and speed change plausibly for 2 minutes; no errors. Rev the engine (if safe) and see RPM follow. |
| 6 | **Clear codes** (only if a code is present and you accept losing it) | Engine off: Clear codes → read the plan → confirm → "Codes cleared"; a new scan shows stored/pending codes gone (permanent may remain). Also check that with the engine **running** the clear is blocked. |
| 7 | **Report** | Reports → open the scan → Export PDF and JSON; reopen the report after restarting the app. |
| 8 | **Offline** | Turn on flight mode (Wi-Fi adapters: keep only the adapter's Wi-Fi). Repeat 4 and 7. |
| 9 | **Disconnect** | Unplug the adapter during live data: the app shows an error and does not hang; reconnect works. |

## Recording the result

1. Expert mode → Expert console → copy the adapter log (VIN hidden by default).
2. Compatibility → Submit a result: one record per function tested, with the
   log excerpt as evidence → Queue for moderation.
3. Compatibility → Offline snapshots → Export snapshot, and attach it plus the
   log to a GitHub issue using the "Compatibility report" template.
4. A maintainer adds the row to compatibility.md for that OS.
