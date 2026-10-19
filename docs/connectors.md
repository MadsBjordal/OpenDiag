# Diagnostic connectors

OpenDiag's connector catalog is `definitions/connectors.json` (54 entries). The
app shows it under **Find my connector**, linked from the connection wizard.

## How the catalog was built (2026-10-19)

Web research, in four parts: cars (old and new), Japanese motorcycles,
European/US motorcycles, trucks and other vehicles. ISO and SAE standards are
paywalled, so standard pin tables come from secondary summaries. Most
manufacturer pinouts come from data-logger makers (AiM), open-source projects,
adapter vendors and forums.

Rules the validator enforces (`crates/opendiag-definitions/src/connectors.rs`):

* Every entry has at least one source URL and a confidence level.
* **Low confidence = no pin table.** Unverified community pinouts are mentioned
  in a note ("sources disagree"), never listed as pins.
* Conflicts between sources are written into the pin text or notes.
* Blink-code and dashboard-test entries must say how to read the codes, and no
  adapter is involved.
* No entry may claim "implemented and tested" until hardware evidence exists.
* **OpenDiag ships no blink-code meanings.** They differ per model; users copy
  them from their own service manual into the reader (saved on the device only).

## What the research showed

| Group | Main finding |
| --- | --- |
| Cars from 1996 (US) / 2001 (EU petrol) / 2004 (EU diesel) | The 16-pin SAE J1962 socket with standard OBD-II: works with an ELM327. |
| Older cars | Every manufacturer had its own socket: blink codes (Toyota, Ford EEC-IV, Mazda, Honda, Volvo, older Mercedes) or proprietary data links (VAG KW1281, BMW, Mercedes 38-pin, Nissan Consult, Renault, PSA, Fiat). An ELM327 cannot talk to the data links. |
| Euro 5 motorcycles | The regulation (Reg. (EU) 44/2014 Annex XII) requires an ISO 19689 6-pin or ISO 15031-3 16-pin connector and a standard protocol. A 6-pin to 16-pin cable + ELM327 should work. |
| Older motorcycles | Brand-specific: Honda 4-pin red DLC (Honda K-line + SCS blink codes), Yamaha 3/4-pin and dashboard tests, Suzuki SDS 6-pin, Kawasaki KDS 4/6-pin, BMW 10-pin round, Ducati 4-pin, Triumph 16-pin (K-line or CAN), Harley 4-pin J1850 / 6-pin CAN, Piaggio/Guzzi Fiat-style 3-pin. |
| Carburettor bikes | No electronic diagnostics at all, e.g. the original Africa Twin XRV650/750. |
| Trucks | J1939 9-pin Deutsch (Type I black / Type II green), older J1708 6-pin, 16-pin Type B (24 V) and manufacturer plugs (MAN, Scania, Mercedes). |
| Agricultural, powersports, marine, e-bikes | ISOBUS, Caterpillar/John Deere layouts, Polaris, BRP, Sea-Doo, NMEA 2000, Mercury SmartCraft, Yamaha marine, EnergyBus. Mostly proprietary. |

## The Honda Africa Twin, as an example

| Model | Connector | What OpenDiag can do today |
| --- | --- | --- |
| XRV650/750 (1988–2003) | None: carburettors and CDI ignition | Guided electrical checks only. |
| CRF1000L (2016–2019) | DLC under the seat (Honda 4-pin type; not confirmed for this model) | Blink codes with the SCS jumper and your manual's table. Honda K-line data: planned. |
| CRF1100L (2020+) | Euro 5 6-pin | Generic OBD-II with a 6-pin cable: implemented, not tested on a real bike. |

## What hardware the gaps need

Most of the unsupported entries need one of three things besides an ELM327:

1. **A raw K-line interface** (an FTDI USB-serial chip with a K-line
   transceiver, often sold as a "KKL" cable). It is needed for Honda, Suzuki,
   Kawasaki, Yamaha, VAG KW1281, Fiat/Piaggio and Triumph K-line. OpenDiag now
   drives such cables (generic OBD-II, plus an experimental Honda mode; see
   [kline.md](kline.md)); the manufacturer protocols still need adding one by one.
2. **A raw CAN interface** (SocketCAN, J2534, or an STN/ELM in monitor mode) for
   J1939, ISOBUS, NMEA 2000 and proprietary CAN.
3. **Manufacturer-specific hardware** for Nissan Consult, GM ALDL, J1708 and
   Ethernet (DoIP).

## Gaps (nothing reliable found)

Toyota DLC2, Honda car 3-pin, PSA 30-pin pinout, DAF trucks, snowmobiles,
Suzuki outboards, Husqvarna, PADS (Piaggio), Polaris 10-pin, Bosch e-bikes,
CAN bit rates and ECU addresses on most motorcycles, and the ISO 13400 DoIP pin
direction. Contributions with documentation or own measurements are welcome
(issue template "Vehicle definition / knowledge contribution").
