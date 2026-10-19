#!/usr/bin/env python3
"""Drives the real Tauri app (Linux debug build) through tauri-driver / WebKitWebDriver.

Usage: tauri-driver & python3 scripts/e2e-tauri.py target/debug/opendiag-desktop <screenshot dir>
(under Xvfb on a headless machine)."""
import base64, json, sys, time, urllib.request

BASE = "http://127.0.0.1:4444"
APP, OUT = sys.argv[1], sys.argv[2]
EL = "element-6066-11e4-a52e-4f735466cecf"

def req(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(BASE + path, data=data, method=method, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(r, timeout=60) as resp:
        return json.loads(resp.read() or b"{}")

sid = req("POST", "/session", {"capabilities": {"alwaysMatch": {"tauri:options": {"application": APP}}}})["value"]["sessionId"]
S = f"/session/{sid}"

import atexit
atexit.register(lambda: req("DELETE", S))

def find(xpath, timeout=20):
    end = time.time() + timeout
    while True:
        try:
            v = req("POST", S + "/element", {"using": "xpath", "value": xpath})["value"]
            if EL in v:
                return v[EL]
        except Exception:
            pass
        if time.time() > end:
            raise SystemExit(f"not found: {xpath}")
        time.sleep(0.3)

def click(xpath, timeout=20):
    el = find(xpath, timeout)
    try:
        req("POST", f"{S}/element/{el}/click", {})
    except urllib.error.HTTPError:
        # WebKitWebDriver sometimes refuses native clicks in Xvfb; a DOM click is equivalent here.
        req("POST", S + "/execute/sync", {"script": "arguments[0].click()", "args": [{EL: el}]})

def js(script):
    return req("POST", S + "/execute/sync", {"script": script, "args": []})["value"]

def shot(name):
    png = req("GET", S + "/screenshot")["value"]
    open(f"{OUT}/{name}.png", "wb").write(base64.b64decode(png))

steps = []
try:
    click("//button[normalize-space()='Continue']", 8)
    steps.append("notice closed")
except SystemExit:
    steps.append("notice already seen")
steps.append("engine reachable: " + json.dumps(js("return document.body.innerText.includes('Beginner mode')")))
click("//button[.//b[text()='Connect to Vehicle']]")
click("//button[.//b[text()='Simulator']]")
click("//div[contains(@class,'card')][.//b[text()='SIMULATED generic OBD-II petrol car']]//button[contains(.,'Connect')]")
find("//*[text()='Step 3 · Adapter']"); steps.append("connected to simulator through Tauri IPC")
shot("tauri-01-connected")
click("//button[contains(.,'Step 6')]")
find("//*[contains(text(),'fault code(s) found')]", 40); steps.append("scan finished (progress events)")
shot("tauri-02-scan")
click("//button[normalize-space()='PDF']")
find("//*[contains(text(),'Saved:')]", 20)
saved = js("return [...document.querySelectorAll('.banner.ok')].map(e => e.textContent).join(' | ')")
steps.append("export: " + saved)
js("location.hash = '#/reports'")
find("//a[contains(.,'#1')]"); steps.append("report listed in history")
print(json.dumps(steps, indent=1))
