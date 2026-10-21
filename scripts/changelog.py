#!/usr/bin/env python3
"""CHANGELOG.md helper for the release workflow.

    python scripts/changelog.py notes                 # print the Unreleased section
    python scripts/changelog.py release "Test build 10" 2026-10-22
        # rename Unreleased to that build and start a new, empty Unreleased
"""
import re, sys
from pathlib import Path

PATH = Path(__file__).resolve().parent.parent / "CHANGELOG.md"
EMPTY = "## Unreleased\n\nNothing yet.\n\n"

def split(text):
    m = re.search(r"^## Unreleased\n(.*?)(?=^## |\Z)", text, re.S | re.M)
    if not m:
        sys.exit("CHANGELOG.md has no '## Unreleased' section")
    return m

def notes(text):
    body = split(text).group(1).strip()
    return "" if body in ("", "Nothing yet.") else body

def release(text, title, date):
    m = split(text)
    if not notes(text):
        return text  # nothing to release: keep the file as it is
    return text[: m.start()] + EMPTY + f"## {title} ({date})\n" + m.group(1) + text[m.end():]

if __name__ == "__main__":
    text = PATH.read_text(encoding="utf-8")
    if sys.argv[1] == "notes":
        print(notes(text) or "No changes listed for this build.")
    elif sys.argv[1] == "release":
        PATH.write_text(release(text, sys.argv[2], sys.argv[3]), encoding="utf-8")
    else:
        sys.exit(__doc__)
