#!/usr/bin/env python3
# Generates definitions/connectors.json. Edit the entries here, run it, and commit both files.
import json

NV = "implemented_not_hardware_verified"
PART = "partially_implemented"
PLAN = "planned"
UNS = "unsupported"

BLINK_SUPPORT = "OpenDiag's blink-code reader helps you count the flashes and looks them up in a table you copy from your service manual. OpenDiag does not ship the code meanings."
GENERIC_STEPS = [
    "Find the procedure for your exact model and year in its service manual before touching the connector.",
    "Ignition off. Bridge only the terminals the manual names, with the correct jumper or service tool.",
    "Switch the ignition on (engine off) and watch the warning light; use the reader below to count long and short flashes.",
    "Ignition off and remove the jumper when you are done.",
]
BRIDGE_WARN = "Bridging the wrong terminals can damage the engine control unit. If you are not sure which terminals to use, do not guess."
UNVERIFIED_PINS = "Community sources give pin layouts for this connector but disagree or cannot be checked, so none is listed. Check the wiring diagram for your model."

def S(url, note=""):
    return {"url": url, "note": note}

def C(id, name, category, makes, applies_to, pins, description, access, protocols, elm327, adapter, support, support_note,
      confidence, sources, voltage="12 V", pinout=None, how_to_read=None, warnings=None, notes=None):
    return dict(id=id, name=name, category=category, makes=makes, applies_to=applies_to, pins=pins, description=description,
                voltage=voltage, access=access, protocols=protocols,
                pinout=[{"pin": str(p), "signal": s} for p, s in (pinout or [])],
                elm327=elm327, adapter=adapter, support=support, support_note=support_note,
                how_to_read=how_to_read or [], warnings=warnings or [], confidence=confidence, sources=sources, notes=notes or [])

WIKI_OBD = S("https://en.wikipedia.org/wiki/On-board_diagnostics", "SAE J1962 pin assignment with cited standards")
ELM_DS = S("https://www.elmelectronics.com/wp-content/uploads/2017/01/ELM327DS.pdf", "ELM327 data sheet: supported protocols")
EU_44 = S("https://www.legislation.gov.uk/eur/2014/44/2018-03-20/data.html", "Reg. (EU) 44/2014 Annex XII 3.12–3.13 (UK-retained text, 2018)")
ISO19689 = S("https://www.iso.org/standard/66030.html", "ISO 19689: common motorcycle diagnostic connector")
AIM_EURO5 = S("https://store.aimsports.com/download/ecu/stock/Bike_euro5_std_eng_100.pdf", "AiM Euro 5 connector guide (vendor)")

c = []
# ───────────── cars ─────────────
c.append(C("sae-j1962-obd2", "OBD-II / EOBD 16-pin, Type A (SAE J1962 / ISO 15031-3)", "car", ["*"],
  "Petrol cars sold in the US from 1996 and the EU from 2001, diesels in the EU from 2004; most light vans", 16,
  "Trapezoid 16-pin socket in two rows of 8, usually within reach of the driver under the dashboard.",
  "data_link", ["ISO 15765-4 CAN", "ISO 14230-4 KWP2000", "ISO 9141-2", "SAE J1850 PWM", "SAE J1850 VPW"], "yes",
  "Any ELM327-compatible OBD-II adapter (USB, Bluetooth or Wi-Fi). K-line vehicles also work with a raw K-line cable (KKL / FTDI).", NV,
  "Generic OBD-II scan, live data and clearing are implemented and simulator-tested, not yet verified on real vehicles.", "high",
  [WIKI_OBD, ELM_DS, S("https://ww2.arb.ca.gov/sites/default/files/barcu/regact/2022/accii/2acciifro1962.5.pdf", "CARB 13 CCR 1962.5: electric cars from MY2026 must have a J1962 Type A connector")],
  pinout=[(2, "SAE J1850 bus +"), (4, "Chassis ground"), (5, "Signal ground"), (6, "CAN high (ISO 15765-4)"),
          (7, "K-line (ISO 9141-2 / ISO 14230-4)"), (10, "SAE J1850 bus − (PWM only)"), (14, "CAN low (ISO 15765-4)"),
          (15, "L-line (ISO 9141-2 / ISO 14230-4)"), (16, "Battery positive")],
  warnings=["Pins 1, 3, 8, 9, 11, 12 and 13 belong to the manufacturer. Some carry programming voltage (e.g. Ford pin 13): never connect anything to them without a model-specific definition."],
  notes=["Examples of manufacturer pins (Wikipedia, medium confidence): GM single-wire CAN on 1, Ford/Chrysler buses on 3 and 11, a second K-line on 8 on many BMWs.",
         "Electric cars: California requires a J1962 Type A connector on electric cars from model year 2026 (40%) and 2027 (100%). Older electric cars often have no standard diagnostics."]))
c.append(C("doip-j1962", "Diagnostics over Ethernet (DoIP, ISO 13400) on the 16-pin socket", "car", ["BMW", "*"],
  "Newer vehicles with Ethernet diagnostics (e.g. BMW with ENET cables)", 16,
  "The same 16-pin socket, with an Ethernet pair on manufacturer pins and an activation line on pin 8.",
  "data_link", ["ISO 13400 DoIP"], "no", "An Ethernet (ENET/DoIP) cable; ELM327 adapters cannot do Ethernet.", PLAN,
  "DoIP is planned for a later phase.", "medium",
  [S("https://python-doipclient.readthedocs.io/en/latest/automotive_ethernet.html", "DoIP pin options"), S("https://docs.autopi.io/developer_guides/setting-up-doip/", "DoIP pin options"), WIKI_OBD],
  pinout=[(3, "Ethernet pair A + (option 1; sources disagree on RX/TX direction)"), (11, "Ethernet pair A − (option 1)"),
          (12, "Ethernet pair B +"), (13, "Ethernet pair B −"), (8, "Ethernet activation")],
  warnings=["The direction of the pairs and the activation resistance must be confirmed from ISO 13400 before any implementation."],
  notes=["Option 2 uses pins 1 and 9 instead of 3 and 11 for the first pair (python-doipclient)."]))
c.append(C("vag-2x2", "VAG 2×2 round connectors (VW, Audi, SEAT, Škoda)", "car", ["Volkswagen", "Audi", "SEAT", "Škoda"],
  "Most Audi 1990–1995, VW 1990–1993 (Ross-Tech); pre-1996 in general", 4,
  "Two separate 2-pin round connectors in the engine bay or fuse box.",
  "data_link", ["KW1281 (VAG K-line protocol)"], "no",
  "A KKL (raw K-line) cable with a 2×2 adapter, or a VAG-specific interface.", UNS,
  "KW1281 is not implemented.", "low",
  [S("https://mail.register.ross-tech.com/vag-com/old-interfaces/max-com.html", "Ross-Tech: models with 2×2 connectors"), S("https://www.elektroda.com/rtvforum/topic2848768.html", "forum wiring discussion")],
  notes=[UNVERIFIED_PINS]))
c.append(C("bmw-20pin-round", "BMW 20-pin round connector (engine bay)", "car", ["BMW"], "BMW cars until about 2000–2001", 20,
  "Round 20-pin socket with a screw-on cap, under the bonnet.", "data_link", ["BMW K/L-line (DS2/KWP)", "CAN on later cars (one source)"], "limited",
  "A 20-pin to 16-pin adapter; most BMW systems need BMW-specific protocols, so a generic adapter reaches little or nothing.", UNS,
  "BMW protocols are not implemented; generic OBD-II only where the car offers it.", "low",
  [S("https://pinouts.ru/CarElectronics/BMW_20_pin_to_OBD_adapter.shtml", "community pinout")], notes=[UNVERIFIED_PINS]))
c.append(C("mercedes-38pin", "Mercedes-Benz 38-pin round connector", "car", ["Mercedes-Benz"],
  "Roughly 1990s to 2002 (sources disagree on the start year)", 38, "Round 38-pin socket in the engine bay.",
  "data_link", ["K-line, one line per system"], "no", "A tool with a line-switching 38-pin cable (multiplexer).", UNS,
  "Not supported.", "low",
  [S("https://pinouts.ru/CarElectronics/Mercedes_38_pin_to_OBD-2.shtml", "community pinout"), S("https://www.youcanic.com/mercedes-benz-models-with-38-pin-port/", "models list")],
  notes=[UNVERIFIED_PINS]))
c.append(C("mercedes-x11-old", "Mercedes-Benz X11 8/9/16-pin (older)", "car", ["Mercedes-Benz"], "Roughly 1988 to mid-1990s", None,
  "Older diagnostic sockets with an on/off-ratio output and blink codes through a pulse counter.", "blink_codes",
  ["On/off ratio (duty cycle)", "Blink codes"], "not_needed", "A blink-code reader or LED tester and the service manual.", PART,
  BLINK_SUPPORT, "low",
  [S("https://pinouts.ru/CarElectronics/mercedes_old_9pin.shtml", "community pinout"), S("https://www.peachparts.com/shopforum/tech-help/26298-self-made-blink-code-tool.html", "forum")],
  how_to_read=GENERIC_STEPS, warnings=[BRIDGE_WARN], notes=["Which pin gives the blink codes differs between sources and models."]))
c.append(C("gm-aldl-12pin", "GM ALDL 12-pin", "car", ["Chevrolet", "GMC", "Buick", "Cadillac", "Pontiac", "Oldsmobile", "Opel"],
  "GM vehicles from 1981; 1994–96 'OBD 1.5' cars carry ALDL on pin 9 of a 16-pin socket", 12,
  "Two rows of 6 lettered terminals (A–F, G–M) under the dashboard.", "data_link_or_blink_codes",
  ["ALDL 160 baud (early)", "ALDL 8192 baud (later)", "Blink codes"], "no",
  "An ALDL interface for data; a jumper for blink codes.", PART, BLINK_SUPPORT + " ALDL data is not implemented.", "medium",
  [S("https://en.wikipedia.org/wiki/ALDL", "ALDL overview"), WIKI_OBD], how_to_read=GENERIC_STEPS, warnings=[BRIDGE_WARN],
  notes=["The function of each lettered terminal varies by model; no reliable general pin table is listed."]))
c.append(C("ford-eec-iv", "Ford EEC-IV self-test connector", "car", ["Ford", "Mercury", "Lincoln"], "Roughly 1983–1995", None,
  "Self-test connector plus a separate single-wire self-test input (STI).", "blink_codes", ["Blink codes (KOEO/KOER self tests)", "STAR tester"],
  "not_needed", "A jumper wire (or Ford STAR tester) and the service manual.", PART, BLINK_SUPPORT, "low",
  [S("https://broncozone.com/topic/24492-eec-iv-technical-notes-by-tom-c/", "quotes Ford TSB 92-24-03")],
  how_to_read=GENERIC_STEPS, warnings=[BRIDGE_WARN], notes=["Codes are 2-digit before model year 1991 and 3-digit after (TSB as quoted)."]))
c.append(C("toyota-dlc1", "Toyota DLC1 (engine bay check connector)", "car", ["Toyota", "Lexus"], "Pre-1996", None,
  "Rectangular check connector under the bonnet; the terminal names are printed on its lid.", "blink_codes", ["Blink codes"],
  "not_needed", "A jumper wire and the service manual.", PART, BLINK_SUPPORT, "low",
  [S("https://alflash.com.ua/dtctoyota2.htm", "procedure description")],
  how_to_read=["Find the procedure for your model in its service manual.",
               "Ignition off. Bridge the two terminals the manual names (commonly reported as TE1 and E1, printed on the lid).",
               "Ignition on, engine off: the check-engine light flashes the codes (pauses separate digits and codes).",
               "Ignition off and remove the jumper."],
  warnings=[BRIDGE_WARN], notes=["DLC2 (under the dashboard): no reliable information found."]))
c.append(C("nissan-consult-14pin", "Nissan Consult 14-pin", "car", ["Nissan", "Infiniti"], "Roughly late 1980s to early 2000s", 14,
  "Grey 14-pin connector, usually under the dashboard.", "data_link", ["Nissan Consult (clocked serial, logic level)"], "no",
  "A Consult interface; it is not an OBD protocol.", UNS, "Not supported.", "low",
  [S("https://pinouts.ru/CarElectronics/nissan_14pin_diag.shtml", "community pinout"), S("https://nistune.com/forum/viewtopic.php?p=27460", "forum")],
  notes=[UNVERIFIED_PINS]))
c.append(C("mazda-17pin", "Mazda 17-pin diagnostic box", "car", ["Mazda"], "e.g. MX-5 1989–1995, RX-7", 17,
  "Diagnostic box in the engine bay with TEN (test) and FEN (fault output) terminals.", "blink_codes", ["Blink codes"], "not_needed",
  "A jumper wire and an LED (or test light) per the service manual.", PART, BLINK_SUPPORT, "low",
  [S("https://www.miata.net/garage/Engine_Diagnostics.html", "procedure")], how_to_read=GENERIC_STEPS, warnings=[BRIDGE_WARN]))
c.append(C("mitsubishi-12pin", "Mitsubishi 12-pin", "car", ["Mitsubishi"], "About 1989–1998", 12, "12-pin connector under the dashboard.",
  "data_link", ["Mitsubishi MUT (unconfirmed)"], "no", "A Mitsubishi-specific interface.", UNS, "Not supported.", "low",
  [S("https://pinouts.ru/CarElectronics/mitsubishi_old_12pin.shtml", "community pinout")], notes=[UNVERIFIED_PINS]))
c.append(C("honda-car-2pin", "Honda car 2-pin service check connector", "car", ["Honda", "Acura"], "About 1992–1995 (1986–91 used an LED on the ECU)", 2,
  "Small 2-pin connector, usually under the passenger-side dashboard.", "blink_codes", ["Blink codes"], "not_needed",
  "A jumper and the service manual.", PART, BLINK_SUPPORT, "low",
  [S("https://dannysengineportal.com/honda-obd1-codes/", "description")], how_to_read=GENERIC_STEPS, warnings=[BRIDGE_WARN],
  notes=["Reported: long flashes count tens, short flashes units. Honda 3-pin: no reliable information found."]))
c.append(C("fiat-3pin", "Fiat / Alfa Romeo / Lancia 3-pin", "car", ["Fiat", "Alfa Romeo", "Lancia"], "Before about 2001", 3,
  "Small 3-pin connector (also used on some Piaggio and Moto Guzzi motorcycles).", "data_link", ["K-line with L-line wake-up (Magneti Marelli / Bosch)"], "no",
  "A KKL cable with a genuine FTDI chip and a 3-pin adapter (forum reports).", UNS, "Not supported.", "low",
  [S("https://pinouts.ru/CarElectronics/fiat_test.shtml", "community pinout"), S("https://www.alfabb.com/threads/what-the-stock-75-ts-motronic-can-and-cannot-do.190352/page-18", "forum")],
  notes=[UNVERIFIED_PINS]))
c.append(C("renault-12pin", "Renault 12-pin", "car", ["Renault"], "Before 1998", 12, "Rectangular 12-pin connector.",
  "data_link", ["K/L-line"], "no", "A Renault-specific interface.", UNS, "Not supported.", "low",
  [S("https://pinouts.ru/CarElectronics/renault_old_12pin.shtml", "community pinout")], notes=[UNVERIFIED_PINS]))
c.append(C("psa-30pin", "Peugeot / Citroën 30-pin", "car", ["Peugeot", "Citroën"], "Until the early 2000s (2-pin before about 1997)", 30,
  "Rectangular 30-pin connector.", "data_link", ["K-line (PSA tools)"], "no", "A PSA-specific interface.", UNS, "Not supported.", "low",
  [S("https://frenchcarforum.co.uk/forum/viewtopic.php?p=711579", "forum")], notes=["No reliable pin layout found."]))
c.append(C("volvo-pre96", "Volvo diagnostic units A and B (engine bay)", "car", ["Volvo"], "Up to 1995 (some 1995 940s have a 16-pin socket that is not OBD-II)", None,
  "Two diagnostic boxes with a selector probe, a push button and an LED.", "blink_codes", ["Blink codes"], "not_needed",
  "Nothing extra: the boxes have their own button and LED. Use the service manual.", PART, BLINK_SUPPORT, "low",
  [S("https://www.matthewsvolvosite.com/read-codes-Volvo-850", "procedure")],
  how_to_read=["Find the procedure for your model in the service manual (which socket of the box selects which system).",
               "Ignition on. Put the probe in the socket for the system, press the button for the time the manual gives.",
               "Count the LED flashes with the reader below."]))
c.append(C("opel-10pin", "Opel / Vauxhall 10-pin", "car", ["Opel", "Vauxhall"], "1988–1996", 10, "Lettered 10-pin connector.",
  "data_link_or_blink_codes", ["Slow blink codes", "K-line"], "no", "A jumper for blink codes; an Opel-specific interface for data.", PART,
  BLINK_SUPPORT + " The data link is not implemented.", "low",
  [S("https://pinouts.ru/CarElectronics/opel_10pin_diag.shtml", "community pinout")], how_to_read=GENERIC_STEPS, warnings=[BRIDGE_WARN], notes=[UNVERIFIED_PINS]))

# ───────────── motorcycles ─────────────
c.append(C("iso19689-euro5-6pin", "Euro 5 motorcycle 6-pin (ISO 19689)", "motorcycle",
  ["Honda", "Yamaha", "Kawasaki", "Triumph", "Aprilia", "*"],
  "EU motorcycles at OBD stage II (Euro 5), e.g. Honda CRF1100L Africa Twin 2020+, Yamaha MT-09 2021+, Kawasaki ZX-4R 2023+, Triumph from about 2024, Aprilia RSV4/Tuono 2021+", 6,
  "Small sealed 6-pin connector, often red, usually under the seat (the owner's manual must say where).",
  "data_link", ["ISO 15765-4 CAN", "ISO 14230-4 / ISO 9141-2 (allowed)", "ISO 14229 UDS (allowed)"], "with_cable",
  "A 6-pin to 16-pin adapter cable and an ELM327-compatible adapter.", NV,
  "The regulation requires these bikes to answer a generic scan tool, so OpenDiag's generic OBD-II scan should work through the cable. Not yet tested on a real bike.", "medium",
  [EU_44, ISO19689, AIM_EURO5, S("https://www.hptuners.com/product/moto-euro5-obdii-adapter-cable/", "lists CRF1100L 2020–2023 among bikes with the Euro 5 6-pin")],
  pinout=[(2, "CAN high"), (3, "Ground"), (4, "Battery positive"), (5, "CAN low"), (6, "K-line")],
  notes=["Pin assignment from a data-logger maker (AiM), not from the ISO text; pin 1 is not listed.",
         "Enduro (L3e-AxE), trial (L3e-AxT), mopeds and some light vehicles are exempt from stage II.",
         "The regulation also allows the 16-pin ISO 15031-3 connector instead."]))
c.append(C("honda-moto-4pin-dlc", "Honda motorcycle 4-pin red DLC", "motorcycle", ["Honda"],
  "Fuel-injected (PGM-FI) Honda bikes from about 2003, e.g. CBR600RR 2003+, CTX700, CRF250L; CRF1000L Africa Twin 2016–2019 has a DLC under the seat (pin count not confirmed for that model)", 4,
  "Red 4-pin connector (often with a dummy plug), usually under the seat.", "data_link_or_blink_codes",
  ["Honda K-line (proprietary, 10400 baud)", "Blink codes via the SCS service connector"], "no",
  "For data: a raw K-line cable (FTDI-type USB serial with a K-line transceiver) wired to the 4-pin DLC. For blink codes: Honda's SCS service connector 070PZ-ZY30100 or equivalent jumper.", PART,
  "Blink codes: " + BLINK_SUPPORT + " Experimental read-only K-line mode (expert mode, raw K-line cable): wake-up and raw table reads, not yet tested on a real bike; sources disagree on the wake-up message. Reading fault codes over the K-line is not implemented.", "medium",
  [S("https://github.com/sophienyaa/Honda-Motorcycle-ECU-Tools", "pinout and protocol notes"), S("https://gonzos.net/projects/ctx-obd/", "protocol notes"),
   S("https://www.cyclepedia.com/?p=40745", "blink-code reading"), S("https://africatwin.com.pl/showthread.php?p=650454", "CRF1000L manual excerpt (forum)")],
  pinout=[(1, "Ground"), (2, "Reset"), (3, "+12 V"), (4, "K-line")],
  how_to_read=["Ignition off. Remove the dummy plug from the red DLC.",
               "Connect the SCS service connector (Honda 070PZ-ZY30100) to the DLC. The terminals to short differ per model: check the manual.",
               "Switch the ignition on (engine off, kill switch to RUN). The fuel-injection warning light (MIL) blinks the codes.",
               "Long blink (about 1.3 s) = 10, short blink (about 0.3 s) = 1; codes come smallest first. Use the reader below.",
               "Ignition off, remove the SCS connector and refit the dummy plug."],
  warnings=[BRIDGE_WARN, "On the CRF1000L, disconnecting the battery does not clear stored codes (manual excerpt quoted on a forum)."],
  notes=["The pinout comes from one open-source project; a second agrees on the functions but gives no numbers.",
         "A generic ELM327 does not work: Honda does not use the ISO wake-up and uses a different checksum."]))
c.append(C("honda-moto-2pin-service", "Honda motorcycle 2-pin service check connector", "motorcycle", ["Honda"],
  "Some older fuel-injected Hondas, e.g. VFR800 1998–2001 (forum reports)", 2, "Small 2-pin connector near the ECU or under the seat.",
  "blink_codes", ["Blink codes"], "not_needed", "A jumper wire and the service manual.", PART, BLINK_SUPPORT, "low",
  [S("https://vfrworld.com/posts/356578/", "forum")],
  how_to_read=["Find the procedure in the service manual for your model.",
               "Ignition off. Bridge the 2-pin connector with a jumper.",
               "Ignition on, side stand down (reported): count the fuel-injection light flashes with the reader below.",
               "Ignition off and remove the jumper."], warnings=[BRIDGE_WARN]))
c.append(C("no-ecu-carburettor", "No electronic diagnostics (carburettor engines)", "motorcycle", ["Honda", "*"],
  "Carburettor bikes without a fuel-injection ECU, e.g. Honda Africa Twin XRV650/750 (1988–2003)", None,
  "These bikes have carburettors and a simple ignition unit (CDI) with no stored fault codes, so there is nothing for a scan tool to read.",
  "no_diagnostics", [], "not_needed", "A multimeter and the service manual for electrical checks.", PART,
  "Use Guided Diagnostics for electrical checks (battery and charging, heated grips). There are no codes to read.", "medium",
  [S("https://en.wikipedia.org/wiki/Honda_XRV750", "XRV750: carburettors"), S("https://gegshop.nl/en/products/improved-cdi-xrv750-africa-twin-rd04-rd07-carmo", "XRV750 CDI ignition")],
  notes=["The newer Africa Twins (CRF1000L, CRF1100L) are fuel-injected and do have a diagnostic connector: see their entries."]))
c.append(C("yamaha-3pin", "Yamaha 3-pin coupler", "motorcycle", ["Yamaha"], "e.g. YZF-R3 2015–2017", 3, "3-pin coupler under the seat.",
  "data_link", ["Yamaha K-line (proprietary)"], "no", "A raw K-line interface and a 3-pin cable.", UNS, "The Yamaha protocol is not implemented.", "low",
  [S("https://aimsports.com/download/ecu/stock/yamaha/Yamaha_YZF-R3_2015-2017_eng.pdf", "AiM: K-line wire"), S("https://github.com/aster94/Keyword-Protocol-2000/issues/11", "protocol notes")],
  notes=["AiM gives the K-line as the yellow/blue wire on the R3. Baud rate not found."]))
c.append(C("yamaha-4pin", "Yamaha 4-pin coupler", "motorcycle", ["Yamaha"], "e.g. MT-09 / FZ-09 2014–2020, MT-07 2017+", 4,
  "4-pin coupler under the tail.", "data_link", ["CAN (MT-09, AiM)", "K-line on some models (forum, conflicting)"], "limited",
  "A 4-pin to 16-pin cable and an ELM327-compatible adapter (one forum report of it reading faults on an MT-07).", PART,
  "OpenDiag's generic OBD-II scan may work through the cable; untested. Yamaha-specific diagnostics are not implemented.", "medium",
  [S("https://store.aimsports.com/download/ecu/stock/yamaha/Yamaha_FZ_MT_09_all_models_ecu_eng.pdf", "AiM MT-09 wiring"), S("https://fz07.org/thread/13558/tested-out-4-pin-to-obd2-cable-for-diagnostics-and-fault-resets/", "forum report")],
  notes=["AiM (MT-09) by wire colour: blue/red CAN high, blue/black CAN low, red/black battery +, black ground. No pin numbers given.", "Do not assume every Yamaha 4-pin is CAN."]))
c.append(C("yamaha-dashboard-selftest", "Yamaha dashboard self-diagnosis", "motorcycle", ["Yamaha"], "Older fuel-injected models, e.g. FZ1, R1", None,
  "Diagnostic mode built into the instrument panel; no connector needed.", "dashboard_self_test", ["Dashboard codes"], "not_needed",
  "Nothing: use the dashboard buttons and the service manual.", PART, "Record the codes in the reader and look them up in your own table from the manual.", "low",
  [S("https://es.scribd.com/document/370389058", "third-party guide"), S("https://www.cyclepedia.com/?p=39894", "newer models: display menu")],
  how_to_read=["Check the service manual for your model.",
               "Reported for some older models: hold SELECT and RESET while switching the ignition on to enter the diagnostic mode.",
               "Write down the codes shown and look them up with the reader below."]))
c.append(C("suzuki-sds-6pin", "Suzuki SDS 6-pin (white)", "motorcycle", ["Suzuki"],
  "e.g. GSX-R600/750 2006–2016, GSX-R1000 2005–2016, Hayabusa 2008–2016", 6, "White 6-pin connector near the ECU under the seat.",
  "data_link_or_blink_codes", ["KWP2000 / ISO 14230 fast init (K-line)", "Dealer mode (codes on the dashboard)"], "no",
  "A raw K-line interface for data; Suzuki's dealer-mode jumper (09930-82720 / 09930-82710 reported) for codes.", PART,
  "Dealer-mode codes: record them in the reader. The SDS data protocol is not implemented.", "medium",
  [S("https://aim-sportline.com/download/ecu/stock/suzuki/Suzuki_SDS_Protocol_eng.pdf", "AiM: power, K-line, ground"), S("https://raw.githubusercontent.com/aster94/Keyword-Protocol-2000/master/src/KWP2000.cpp", "open-source KWP2000 for Suzuki/Kawasaki")],
  how_to_read=["Check the service manual for your model.", "Ignition off. Fit the dealer-mode jumper to the connector the manual names.",
               "Ignition on: the dashboard or FI light shows the codes. Record them below.", "Ignition off and remove the jumper."],
  warnings=[BRIDGE_WARN], notes=["AiM shows power, K-line and ground on this connector but without pin numbers."]))
c.append(C("kawasaki-kds-4pin", "Kawasaki KDS 4-pin", "motorcycle", ["Kawasaki"], "Many fuel-injected Kawasakis (e.g. Versys 650 2012)", 4,
  "4-pin connector under the seat.", "data_link_or_blink_codes", ["KWP2000 / ISO 14230 (K-line)", "Self-diagnosis via the FI light on older models"], "no",
  "A raw K-line interface (Kawasaki harness 57001-1868 reported) for data; the manual's grounding procedure for codes.", PART,
  "Blink codes: " + BLINK_SUPPORT + " The KDS data protocol is not implemented.", "low",
  [S("https://raw.githubusercontent.com/aster94/Keyword-Protocol-2000/master/src/KWP2000.cpp", "open-source KWP2000, Versys 650 tested"), S("https://www.vulcandrifterriders.com/ECUFaultCodes.html", "self-diagnosis description")],
  how_to_read=GENERIC_STEPS, warnings=[BRIDGE_WARN], notes=["Sources disagree on whether the 4-pin or the 6-pin carries the K-line.", UNVERIFIED_PINS]))
c.append(C("kawasaki-6pin", "Kawasaki 6-pin (CAN)", "motorcycle", ["Kawasaki"], "e.g. ZX-10R 2011+", 6, "6-pin connector under the seat.",
  "data_link", ["CAN"], "limited", "A 6-pin to 16-pin cable and a CAN-capable adapter (not confirmed).", PART,
  "Generic OBD-II may or may not answer; untested.", "medium",
  [S("https://cdn.shopify.com/s/files/1/0214/9392/files/KawasakiZX10R_2011_103_eng.pdf", "AiM ZX-10R wiring")],
  notes=["AiM by wire colour: grey/blue CAN high, light blue CAN low; power and ground not given."]))
c.append(C("bmw-moto-10pin-round", "BMW Motorrad 10-pin round", "motorcycle", ["BMW"], "About 1996–2016 (seller claim)", 10,
  "Round 10-pin connector, often near the battery.", "data_link", ["K-line (KWP2000)", "D-CAN on later bikes"], "no",
  "A BMW-specific tool (e.g. GS-911) with its round adapter; generic readers reportedly do not reach the gateway.", UNS, "Not supported.", "low",
  [S("https://pinouts.ru/CarElectronics/BMW_motorcycle_10_pin_dia.shtml", "community pinout"), S("https://biketrailspfannenstiel.ch/auskunft/bmw-motorrad-obd-stecker/", "conflicting pinout")],
  notes=[UNVERIFIED_PINS]))
c.append(C("bmw-moto-16pin", "BMW Motorrad 16-pin OBD", "motorcycle", ["BMW"], "From 2016/2017 (sources disagree)", 16,
  "Car-style 16-pin socket.", "data_link", ["CAN"], "limited", "An ELM327-compatible adapter may reach generic OBD only; BMW functions need a BMW tool.", PART,
  "Generic OBD-II scan may work; untested.", "low",
  [S("https://forum.hexcode.co.za/forum/index.php/topic,1507.0.html", "forum")], notes=["Reported: CAN on 6/14, power 16, ground 4 (forum only, not listed as pins)."]))
c.append(C("ducati-dda-4pin", "Ducati DDA 4-pin (under the tail)", "motorcycle", ["Ducati"], "848/1098/1198 2007–2013, Panigale 2012–2019", 4,
  "4-pin connector under the tail.", "data_link", ["CAN"], "limited", "A 4-pin to 16-pin cable and an ELM327-compatible adapter (seller claims).", PART,
  "Generic OBD-II scan may work; untested.", "medium",
  [S("https://www.aimsports.com/download/ecu/stock/ducati/Ducati_848_1098_1198_eng.pdf", "AiM wiring"), S("https://aim-sportline.com/download/ecu/stock/ducati/DucatiPanigale_eng.pdf", "AiM wiring")],
  pinout=[(3, "CAN high"), (4, "CAN low")], notes=["Older Ducatis (Marelli 59M) reportedly use a 3-pin connector; no reliable details."]))
c.append(C("triumph-16pin-kline", "Triumph 16-pin (K-line)", "motorcycle", ["Triumph"], "Daytona 675 2006–2012, Street/Speed Triple 2007–2012", 16,
  "Car-style 16-pin socket under the seat.", "data_link", ["K-line (Triumph)"], "limited",
  "TuneECU users report an FTDI KKL cable works; reports on ELM327 adapters conflict.", UNS, "Triumph diagnostics are not implemented; generic OBD-II not confirmed.", "high",
  [S("https://store.aimsports.com/download/ecu/stock/triumph/Triumph_Daytona675_2006-2012+StreetTriple+SpeedTriple_2007-2012_K_Line_101_eng.pdf", "AiM wiring")],
  pinout=[(4, "Ground"), (7, "K-line")]))
c.append(C("triumph-16pin-can", "Triumph 16-pin (CAN)", "motorcycle", ["Triumph"], "675/765 2013 to about 2023", 16,
  "Car-style 16-pin socket under the seat.", "data_link", ["CAN"], "limited", "An ELM327-class adapter (OBDLink LX/MX named by TuneECU users).", PART,
  "Generic OBD-II scan may work; untested.", "high",
  [S("https://aim-sportline.com/download/ecu/stock/triumph/Triumph_CAN_2013+_100_eng.pdf", "AiM wiring")],
  pinout=[(6, "CAN high"), (14, "CAN low")], notes=["From about 2024 Triumph uses the Euro 5 6-pin connector."]))
c.append(C("harley-4pin-j1850", "Harley-Davidson 4-pin Deutsch (J1850)", "motorcycle", ["Harley-Davidson"], "About 2001–2013, varies by family", 4,
  "Light grey 4-pin Deutsch connector.", "data_link", ["SAE J1850 VPW (10.4 kbit/s)"], "limited",
  "A 4-pin to 16-pin cable and a J1850 VPW-capable adapter (ELM327 supports VPW; Harley's own messages are not standard OBD).", PART,
  "OpenDiag speaks J1850 VPW through ELM327, but whether a Harley answers generic OBD-II requests is not confirmed.", "medium",
  [S("https://biketrailspfannenstiel.ch/auskunft/harley-davidson-diagnosestecker/", "protocol, 3 used pins"), S("https://university.fuelmotousa.com/article/power-vision-h-d-diagnostic-connector-locations/", "connector locations"), ELM_DS],
  notes=["One source: only 3 pins are used (J1850 bus +, ground, +12 V); pin numbers not given."]))
c.append(C("harley-6pin-can", "Harley-Davidson 6-pin Deutsch (CAN)", "motorcycle", ["Harley-Davidson"], "From 2011–2014 depending on model", 6,
  "6-pin Deutsch connector.", "data_link", ["CAN (HD LAN)"], "limited", "A 6-pin to 16-pin cable (sold by several vendors) and a CAN-capable adapter.", PART,
  "Generic OBD-II scan may work; untested.", "medium",
  [S("https://university.fuelmotousa.com/article/power-vision-h-d-diagnostic-connector-locations/", "connector locations")], notes=["No pinout found."]))
c.append(C("piaggio-3pin", "Piaggio / Vespa / Moto Guzzi 3-pin (Fiat-style)", "motorcycle", ["Piaggio", "Vespa", "Moto Guzzi"],
  "e.g. Vespa GTS (MIU1/MIU3G), Moto Guzzi Griso/Quota", 3, "Small 3-pin connector (same style as older Fiat cars).",
  "data_link", ["K-line (Magneti Marelli IAW/MIU)"], "no", "A VAG KKL cable with a genuine FTDI chip and a 3-pin adapter (GuzziDiag/IAWdiag users; a CH340 cable failed).", UNS,
  "Not supported.", "low",
  [S("https://modernvespa.com/forum/topic150992", "forum wiring"), S("https://www.thisoldtractor.com/moto_guzzi_quota_guzzidiag_howto_-_a_tutorial_for_beginners.html", "GuzziDiag tutorial")],
  warnings=["The forum poster warns that wiring it wrong can damage the ECU."], notes=[UNVERIFIED_PINS]))
c.append(C("royal-enfield-6pin", "Royal Enfield 6-pin", "motorcycle", ["Royal Enfield"], "BS6 bikes, e.g. Interceptor", 6, "6-pin connector.",
  "data_link", ["CAN (likely)"], "limited", "6-pin to 16-pin adapters are sold.", PART, "Generic OBD-II may work; untested.", "low",
  [S("https://www.team-bhp.com/news/setting-obd-reader-indian-motorcycles", "owner's report")]))
c.append(C("ktm-6pin-pre-euro5", "KTM / Husqvarna 6-pin (before Euro 5)", "motorcycle", ["KTM", "Husqvarna"], "Pre-Euro 5 (Keihin ECUs)", 6,
  "6-pin connector.", "data_link", ["K-line and CAN (seller claims)"], "no", "TuneECU-compatible adapter cables are sold.", UNS, "Not supported.", "low",
  [S("https://tuning-shop.com/dimsport/motorcycle-complete-kable-kit/", "cable kit")], notes=["Husqvarna: no sources found; sharing KTM's connector is a guess.", UNVERIFIED_PINS]))

# ───────────── trucks ─────────────
c.append(C("sae-j1962-type-b", "OBD 16-pin Type B (24 V)", "truck", ["*"], "24 V trucks and buses", 16,
  "Like the car socket but its centre groove is split, so a 12 V (Type A) plug does not fit.", "data_link",
  ["ISO 15765-4 CAN", "SAE J1939 (some trucks)", "ISO 27145 WWH-OBD (some trucks)"], "limited",
  "An adapter rated for 24 V. Check the adapter's specification: not every ELM327 adapter survives 24 V.", PART,
  "Generic OBD-II over ISO 15765-4 works the same as on cars; J1939 and WWH-OBD are not implemented.", "high",
  [WIKI_OBD, S("https://store.sfs.fi/en/iso-15031-3-2016", "ISO 15031-3")], voltage="24 V",
  pinout=[(4, "Chassis ground"), (5, "Signal ground"), (6, "CAN high"), (14, "CAN low"), (16, "Battery positive (24 V)")],
  warnings=["24 V on pin 16. A 12 V-only adapter can be destroyed."],
  notes=["Renault/Volvo trucks (community, low confidence): J1939 on 6/14, J1587 on 12/13, KWP2000 on 7. MAN 'HD-OBD' and others vary."]))
c.append(C("j1939-9pin-deutsch", "SAE J1939-13 9-pin Deutsch (Type I black / Type II green)", "truck", ["*"],
  "North American heavy trucks and buses; also many machines", 9,
  "Round 9-pin Deutsch connector. Type I (black) is 250 kbit/s; Type II (green) supports 250 or 500 kbit/s. A Type II cable fits both.",
  "data_link", ["SAE J1939 CAN (250/500 kbit/s)", "SAE J1708/J1587 (on some)"], "limited",
  "A J1939 interface, or an ELM327 v2-class adapter with a 9-pin cable (the ELM327 can monitor DM1 codes and request PGNs; no address claim).", PLAN,
  "J1939 is planned; OpenDiag cannot read it yet.", "high",
  [S("https://copperhilltech.com/blog/tag/SAE+J1939-13", "pin assignment"), S("https://digi.com/resources/documentation/digidocs/90001930/reference/r_hardware_connector_pinout.htm", "pin assignment"),
   S("https://kvaser.com/j1939-13-type-ii-connection-now-available/", "Type I/II"), ELM_DS], voltage="12/24 V",
  pinout=[("A", "Battery −"), ("B", "Battery + (usually unswitched)"), ("C", "CAN high (J1939)"), ("D", "CAN low (J1939)"), ("E", "CAN shield"),
          ("F", "J1708 (sources disagree on polarity)"), ("G", "J1708 (sources disagree on polarity)"), ("H", "Manufacturer use / second CAN (sources disagree)"), ("J", "Manufacturer use / second CAN (sources disagree)")],
  warnings=["Not every 9-pin Deutsch is standard: Caterpillar and John Deere use different layouts on 9-pin plugs."]))
c.append(C("j1708-6pin-deutsch", "SAE J1708/J1587 6-pin Deutsch", "truck", ["*"], "Heavy trucks, roughly early 1990s to mid-2000s", 6,
  "Round 6-pin Deutsch connector.", "data_link", ["SAE J1708 / J1587 (9600 bit/s)"], "no",
  "A J1708 interface; ELM327 adapters do not support J1708.", UNS, "Not supported.", "medium",
  [S("https://en.wikipedia.org/wiki/SAE_J1587", "pinout"), S("https://www.diesellaptops.com/blogs/tool-tutorials/troubleshooting-j1708-connections", "pinout, bit rate")], voltage="12/24 V",
  pinout=[("A", "J1708 +"), ("B", "J1708 −"), ("C", "Battery + (unswitched)"), ("E", "Ground")]))
for (id, name, makes, src, note) in [
  ("man-12pin", "MAN 12-pin diagnostic connector", ["MAN"], S("https://pinouts.ru/CarElectronics/MAN_12-pin_diagnostic_int.shtml", "community pinout"), "TGL/TGM/TGA, buses"),
  ("scania-16pin-round", "Scania round 16-pin", ["Scania"], S("https://docs.autopi.io/hardware/accessories/scania-16pin-j1939-obd2/", "vendor adapter pinout"), "Scania trucks"),
  ("mercedes-truck-14pin", "Mercedes-Benz truck 14-pin", ["Mercedes-Benz"], S("https://docs.autopi.io/hardware/accessories/mercedes-14pin-j1939-obd2/", "vendor adapter pinout"), "Mercedes-Benz trucks"),
]:
  c.append(C(id, name, "truck", makes, note, None, "Manufacturer-specific diagnostic connector.", "data_link", ["CAN / J1939 (vendor)", "K-line (some)"], "no",
    "A manufacturer adapter cable to a J1939 or 16-pin tool.", UNS, "Not supported.", "low", [src], voltage="24 V", notes=[UNVERIFIED_PINS]))

# ───────────── agricultural / construction ─────────────
c.append(C("isobus-iso11783", "ISOBUS (ISO 11783) diagnostic connector", "agricultural", ["*"], "Tractors and implements with ISOBUS", None,
  "ISO 11783 defines a diagnostic connector (part 2, clause 6.4.5); its pin layout was not available in free sources.", "data_link",
  ["ISO 11783 CAN, 250 kbit/s"], "limited", "A CAN interface; ELM327 user CAN modes might monitor it (not verified).", UNS,
  "Not supported.", "medium", [S("https://www.sis.se/api/document/preview/914356/", "ISO 11783-2 preview: 250 kbit/s")],
  notes=["The optional 9-pin in-cab connector of ISO 11783-2 is not the diagnostic connector."]))
c.append(C("caterpillar-cdl", "Caterpillar 9-pin / 14-pin (CDL)", "agricultural", ["Caterpillar"], "Caterpillar machines and engines", None,
  "Deutsch 9-pin or 14-pin with Caterpillar's own layout.", "data_link", ["Caterpillar CDL (proprietary)", "J1939"], "no",
  "A Caterpillar tool for CDL; a J1939 interface with the correct adapter for J1939.", UNS, "Not supported.", "low",
  [S("https://docs.autopi.io/hardware/accessories/caterpillar-538-5051-9pin-j1939-obd2/", "vendor adapter"), S("https://docs.autopi.io/hardware/accessories/caterpillar-14pin-j1939-obd2/", "vendor adapter")],
  voltage="12/24 V", notes=[UNVERIFIED_PINS]))
c.append(C("john-deere-9pin", "John Deere 9-pin (non-standard)", "agricultural", ["John Deere"], "e.g. Series 30", 9,
  "Deutsch 9-pin with John Deere's own layout (CAN not on C/D).", "data_link", ["CAN"], "no", "A John Deere-specific cable to a CAN/J1939 tool.", UNS,
  "Not supported.", "low", [S("https://pinouts.ru/CarElectronics/Jhon_Deere_9_pin_diagnost.shtml", "community pinout, marked unverified")], notes=[UNVERIFIED_PINS]))

# ───────────── powersports ─────────────
c.append(C("polaris-8pin", "Polaris 8-pin 'Digital Wrench' connector", "powersports", ["Polaris"], "Polaris EFI ATVs/UTVs with the 8-pin connector (some use 4-pin)", 8,
  "8-way connector (reported as Delphi GT150).", "data_link", ["CAN (some models)"], "no", "Polaris Digital Wrench or a matching CAN cable.", UNS,
  "Not supported.", "low", [S("https://www.rzrforums.net/threads/polaris-diagnostic-plug.103193/", "forum")], notes=["Forum posts disagree on pin order.", UNVERIFIED_PINS]))
c.append(C("brp-6pin", "BRP Can-Am 6-pin Deutsch", "powersports", ["Can-Am", "BRP"], "e.g. Renegade, Spyder", 6, "6-pin Deutsch DT connector.",
  "data_link", ["CAN (proprietary)", "K-line"], "no", "BRP's BUDS/BUDS2 with its interface.", UNS, "Not supported (proprietary, no OBD-II).", "low",
  [S("https://sparxeng.com/blog/hardware/reverse-engineering-the-can-am-spyder-diagnostic-interface", "reverse-engineering notes"), S("https://pinouts.ru/CarElectronics/BRP_Can-Am_Renegade_Quad.shtml", "community pinout")],
  notes=["Reported to carry a CAN pair, K-line, ground and two fused +12 V lines.", UNVERIFIED_PINS]))
c.append(C("sea-doo-buds", "Sea-Doo / BRP watercraft diagnostic connector", "powersports", ["Sea-Doo", "BRP"], "2002 and newer", None,
  "Diagnostic connector used with BRP's BUDS software and MPI interface.", "data_link", ["Proprietary"], "no", "BRP BUDS/BUDS2 with the MPI interface.", UNS,
  "Not supported.", "low", [S("https://watercraftjournal.com/the-ultimate-sea-doo-diagnostic-tool-for-the-diyer/", "overview")]))

# ───────────── marine ─────────────
c.append(C("nmea2000-m12", "NMEA 2000 (M12 5-pin Micro-C)", "marine", ["*"], "Boat networks: engines, instruments", 5,
  "Round M12 5-pin connector on the boat's backbone.", "data_link", ["NMEA 2000 (CAN, 250 kbit/s)"], "limited",
  "A CAN / NMEA 2000 gateway. ELM327 user CAN modes might monitor it (not verified).", UNS, "Not supported.", "high",
  [S("https://en.wikipedia.org/wiki/NMEA_2000", "bit rate, pinout"), S("https://www.peak-system.com/PCAN-Cable-M12.589.0.html?&L=1", "pinout")],
  pinout=[(1, "Shield"), (2, "NET-S (+12 V)"), (3, "NET-C (ground)"), (4, "NET-H (CAN high)"), (5, "NET-L (CAN low)")]))
c.append(C("mercury-smartcraft", "Mercury SmartCraft 10-pin", "marine", ["Mercury"], "Mercury engines with SmartCraft", 10,
  "10-pin connector with a CAN pair.", "data_link", ["CAN (SmartCraft)"], "no", "A Mercury-compatible interface.", UNS, "Not supported.", "medium",
  [S("https://aimsports.com/download/ecu/racing/Mercury_CAN_101_eng.pdf", "AiM wiring")], notes=["AiM by wire colour: white CAN high, blue CAN low. Pin numbers and bit rate not given."]))
c.append(C("yamaha-marine-3pin", "Yamaha outboard / WaveRunner 3-pin (YDS)", "marine", ["Yamaha"], "Yamaha outboards and WaveRunners", 3,
  "3-pin connector used with Yamaha Diagnostic System.", "data_link", ["Yamaha proprietary"], "no", "Yamaha YDS or a matching cable.", UNS, "Not supported.", "low",
  [S("https://www.hptuners.com/product/obdii-adapter-cable-yamaha-3-pin/", "cable")]))

# ───────────── light electric ─────────────
c.append(C("energybus-6pin", "EnergyBus 6-pin (e-bikes, LEV)", "light_electric", ["*"], "E-bikes and light electric vehicles using EnergyBus", 6,
  "Rosenberger RoPD connector carrying power and CAN.", "data_link", ["CANopen CiA 454"], "no", "A CAN interface with CANopen support.", UNS,
  "Not supported.", "medium", [S("https://en.wikipedia.org/wiki/EnergyBus", "pinout"), S("https://pedelec-elektro-fahrrad.de/news/cia-454-offener-energybus-kommunikationsstandard-freigegeben", "CiA 454")],
  voltage="12–48 V",
  pinout=[(1, "CAN high"), (2, "CAN low"), (3, "+12 V auxiliary"), (5, "Main power +"), (6, "Main power −")],
  warnings=["Main power pins carry battery voltage up to 48 V and high current."],
  notes=["Big e-bike brands (e.g. Bosch) use their own systems; nothing reliable found."]))

KL_NOTE = "OpenDiag's raw K-line driver speaks generic OBD-II (ISO 9141-2 / ISO 14230-4) only so far; this manufacturer protocol is not implemented."
for e in c:
    if e["id"] in ["vag-2x2", "fiat-3pin", "piaggio-3pin", "triumph-16pin-kline", "yamaha-3pin", "suzuki-sds-6pin", "kawasaki-kds-4pin", "renault-12pin", "opel-10pin"]:
        e["notes"].append(KL_NOTE)

out = {"format": "opendiag-connectors", "version": 1, "connectors": c}
json.dump(out, open("/home/user/opendiag/definitions/connectors.json", "w"), indent=1, ensure_ascii=False)
print(len(c))
