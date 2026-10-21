#!/usr/bin/env python3
"""Builds OpenDiag's fault code title files from the MIT-licensed dtc-database
(https://github.com/Wal33D/dtc-database, data/dtc_codes.db).

    python scripts/import_dtc_titles.py path/to/dtc_codes.db

Writes crates/opendiag-knowledge/data/dtc-titles.json (generic codes) and
dtc-manufacturer.json (manufacturer-specific codes). Rules, so the result stays
honest:
  * generic titles only for codes in SAE J2012 generic ranges (P0, P2, P34-P39,
    B0, C0, U0): the source labels some manufacturer codes "generic";
  * manufacturer titles only for codes in manufacturer ranges, for named makes
    (the source's "OTHER" group has no make and is left out), and only when the
    text reads like a definition, not a list of possible causes;
  * makes whose source list is a copy of another brand's are left out
    (INFINITI: identical to HONDA; Infiniti is a Nissan brand);
  * curated explanations in dtc-generic.json always take precedence (in code).
"""
import json, re, sqlite3, sys
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "crates/opendiag-knowledge/data"
SOURCE = ("Titles from dtc-database by Wal33D (https://github.com/Wal33D/dtc-database), "
          "MIT License, Copyright (c) 2024 Wal33D (Waleed Judah). Generic titles follow SAE J2012.")

def ownership(code):
    s, d = code[0], code[1]
    if s == "P" and d in "02": return "generic"
    if s == "P" and d == "1": return "manufacturer"
    if s == "P" and d == "3": return "manufacturer" if code[2] in "0123" else "generic"
    if d == "0": return "generic"
    if d in "12": return "manufacturer"
    return "reserved"

WRONG_COPY = {"INFINITI"}

CAUSE_START = re.compile(r"^(a |an |the )?(faulty|defective|failed|failure|bad|worn|low|loose|damaged|broken|control module software|wiring|open or shorted)", re.I)

def looks_like_definition(text):
    return text.count(",") < 2 and not CAUSE_START.match(text) and len(text) <= 150

def clean(text):
    return re.sub(r"\s+", " ", text).strip().rstrip(".")

def main(db):
    c = sqlite3.connect(db)
    rows = c.execute("select code, manufacturer, description from dtc_definitions where locale='en'").fetchall()
    generic, manuf, skipped = {}, {}, {"generic_not_in_generic_range": 0, "manufacturer_generic_range": 0, "cause_list": 0, "no_make": 0, "wrong_copy": 0}
    for code, make, text in rows:
        code, text = code.strip().upper(), clean(text)
        if not re.fullmatch(r"[PCBU][0-9A-F]{4}", code) or not text:
            continue
        own = ownership(code)
        if make == "GENERIC":
            if own != "generic":
                skipped["generic_not_in_generic_range"] += 1; continue
            generic[code] = text
        elif make in WRONG_COPY:
            skipped["wrong_copy"] += 1
        elif make == "OTHER":
            skipped["no_make"] += 1
        elif own != "manufacturer":
            skipped["manufacturer_generic_range"] += 1
        elif not looks_like_definition(text):
            skipped["cause_list"] += 1
        else:
            manuf.setdefault(make, {})[code] = text
    (OUT / "dtc-titles.json").write_text(json.dumps({
        "format": "opendiag-dtc-titles", "version": 1, "source": SOURCE, "license": "MIT",
        "codes": [{"code": k, "title": v} for k, v in sorted(generic.items())],
    }, indent=0, ensure_ascii=False) + "\n")
    (OUT / "dtc-manufacturer.json").write_text(json.dumps({
        "format": "opendiag-dtc-manufacturer", "version": 1, "source": SOURCE, "license": "MIT",
        "note": "Community dataset, not verified against manufacturer documentation.",
        "makes": {m: [{"code": k, "title": v} for k, v in sorted(codes.items())] for m, codes in sorted(manuf.items())},
    }, indent=0, ensure_ascii=False) + "\n")
    print(f"generic titles: {len(generic)}; manufacturer titles: {sum(map(len, manuf.values()))} for {len(manuf)} makes; skipped: {skipped}")

if __name__ == "__main__":
    main(sys.argv[1])
