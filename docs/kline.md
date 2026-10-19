# K-line cables

Status: **implemented and simulator-tested, not yet tested on a real vehicle.**

## What it is

Many cars from roughly 2000–2008 and many motorcycles before Euro 5 talk over a
single wire, the **K-line** (ISO 9141-2, ISO 14230 / KWP2000). An ELM327
adapter handles the K-line for you, but only for standard OBD-II. A **raw
K-line cable** ("KKL", "VAG-COM 409.1", FTDI-based) is just a USB-serial chip
plus a K-line transceiver; OpenDiag then controls the line itself. That is what
manufacturer protocols (Honda, Suzuki, Kawasaki, VAG, Fiat…) need.

## What OpenDiag does with it today

| Mode | What it sends | Status |
| --- | --- | --- |
| Generic OBD-II, **fast init** (ISO 14230-4) | K-line low 25 ms, high 25 ms, `C1 33 F1 81 66` at 10400 baud | Simulator-tested |
| Generic OBD-II, **5-baud init** (ISO 9141-2 / ISO 14230-4) | Address `33` at 5 bit/s, sync `55`, key bytes, inverted KB2, inverted address `CC` | Simulator-tested |
| Automatic | Fast init, then 5-baud | Simulator-tested |
| **Honda motorcycle** (4-pin DLC), experimental, expert mode only | K-line low 70 ms, high 120 ms, `FE 04 FF FF` or `FE 04 72 8C`, then `72 05 00 F0 99`; table reads `72 05 71 <table>` | Simulator-tested; frames from open-source projects; table bytes not decoded; fault codes not implemented |

Once connected with generic OBD-II, the whole app works as usual: scan, live
data, clearing (with the same safety checks), reports, compatibility records.

The driver checks the echo of everything it sends (the K-line is one wire),
works with cables that do not echo, re-wakes the vehicle after 4.5 s of
silence (ISO P3 max is 5 s) and keeps at least 55 ms between requests.

## Hardware

* **Cable:** FTDI FT232R-based KKL / "VAG-COM 409.1" cables are the most
  widely reported to work. Forum reports say CH340-based cables often fail
  with manufacturer K-line tools. **The cable must support a serial break**
  (that is how OpenDiag pulls the line low).
* **Car (16-pin socket):** plug in directly; K-line is pin 7.
* **Motorcycle:** you need an adapter cable from the bike's connector to the
  cable (see Find my connector). For the Honda 4-pin DLC, sources give pin 4 =
  K-line, 1 = ground, 3 = +12 V (one source); check before wiring.
* **Windows:** set the FTDI **latency timer to 1 ms**: Device Manager → Ports
  (COM & LPT) → the cable → Properties → Port Settings → Advanced → Latency
  Timer. The default 16 ms can break K-line timing.
* **Android:** USB-C OTG cable only. Bluetooth adapters cannot drive a K-line.

## Code

* `crates/opendiag-kline`: framing (`frame.rs`), the OBD driver (`lib.rs`), the
  Honda mode (`honda.rs`).
* `crates/opendiag-driver`: the `Driver` interface shared with the ELM327 driver.
* `crates/opendiag-core` `KLinePort`: write, read, `set_break`, `set_baud`,
  `clear_input`. Implemented by `SerialKLine` (Windows/Linux/macOS), the Android
  USB bridge, and `SimKLinePort` (simulator, reacts to the real line timing).
* Safety: generic OBD requests go through `may_send` like every driver; Honda
  frames through `may_send_honda`, which allows only the wake-up, initialise and
  table-read frames.

## Testing on a real vehicle

1. Car first (simplest): a K-line car from the early 2000s, 16-pin socket.
   Connect → **K-line cable** → choose the COM port → Wake-up **Automatic** →
   Connect, then Scan. Note which wake-up worked (the adapter log shows it).
2. Honda bike (expert mode): Find my connector → Honda 4-pin red DLC → Details
   → Experimental Honda panel → Open cable → Wake up (try both variants) →
   Read table. Copy the log (Expert console) into a compatibility report.
3. Report results with the "Compatibility report" issue template.
