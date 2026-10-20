//! A simulated ELM327 adapter connected to a simulated vehicle.
//!
//! It answers AT commands and OBD-II requests byte for byte the way an ELM327
//! does (echo, spaces, headers, ISO-TP frames, K-line checksums, error words),
//! so the real driver and the real scan workflow run against it unchanged.
//! Everything it reports is labelled SIMULATED.
//!
//! [`SimControl`] changes the vehicle while connected: engine on/off and faults
//! (timeouts, disconnects, malformed and cut-off answers, low voltage, wrong VIN…).

mod j1939;
pub mod kline;
pub mod profiles;
pub mod signals;

use opendiag_core::{ByteLink, LinkError, LinkInfo, LinkKind, ObdProtocol};
use opendiag_obd::{dtc::Dtc, pid::bitmap_for, readiness};
use profiles::{SimEcu, VehicleProfile};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Faults {
    /// The ignition is off: nothing on the vehicle answers.
    pub ignition_off: bool,
    /// The adapter swallows requests and never answers (timeouts).
    pub no_response: bool,
    /// The link drops after this many more requests (0 = now).
    pub disconnect_after: Option<u32>,
    /// Vehicle answers contain invalid characters.
    pub malformed: bool,
    /// Multi-frame answers lose their last frame.
    pub truncated: bool,
    /// Battery at 10.6 V.
    pub low_voltage: bool,
    /// The VIN contains characters a VIN may not contain.
    pub wrong_vin: bool,
    /// Every vehicle request ends in "CAN ERROR" / "BUS ERROR".
    pub bus_error: bool,
    /// Adapter behaves like a cheap clone: no headers, no spaces-off, no voltage.
    pub clone_adapter: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimSettings {
    pub engine_running: bool,
    pub faults: Faults,
    /// Delay before each vehicle answer, like a real bus.
    pub latency_ms: u64,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self {
            engine_running: true,
            faults: Faults::default(),
            latency_ms: 15,
        }
    }
}

struct Vehicle {
    profile: VehicleProfile,
    /// Codes cleared? Then freeze frame is gone and monitors are incomplete.
    cleared: bool,
}

struct State {
    vehicle: Vehicle,
    settings: SimSettings,
    echo: bool,
    linefeeds: bool,
    spaces: bool,
    headers: bool,
    /// None = automatic.
    protocol_choice: Option<u8>,
    connected: bool,
    started: Instant,
    disconnected: bool,
    requests: u64,
    /// J1939 header printed as 4 bytes (AT JHF0) instead of the default format.
    j1939_plain_header: bool,
    /// `AT MP` without a count: monitoring until any character arrives.
    monitoring: bool,
}

/// Change the simulated vehicle while the app is connected to it.
#[derive(Clone)]
pub struct SimControl(Arc<Mutex<State>>);

impl SimControl {
    pub fn settings(&self) -> SimSettings {
        self.0.lock().unwrap().settings.clone()
    }
    pub fn set(&self, settings: SimSettings) {
        self.0.lock().unwrap().settings = settings;
    }
    pub fn profile(&self) -> VehicleProfile {
        self.0.lock().unwrap().vehicle.profile.clone()
    }
    /// Number of vehicle requests answered so far.
    pub fn requests(&self) -> u64 {
        self.0.lock().unwrap().requests
    }
}

pub struct SimAdapter {
    state: Arc<Mutex<State>>,
    out: VecDeque<u8>,
    line: Vec<u8>,
}

impl SimAdapter {
    pub fn new(profile: VehicleProfile, settings: SimSettings) -> (Self, SimControl) {
        let state = Arc::new(Mutex::new(State::new(profile, settings)));
        (
            Self {
                state: state.clone(),
                out: VecDeque::new(),
                line: Vec::new(),
            },
            SimControl(state),
        )
    }

    fn handle_line(&mut self, cmd: &str) {
        let mut st = self.state.lock().unwrap();
        let echo = st.echo;
        let body = st.answer(cmd);
        let eol = if st.linefeeds { "\r\n" } else { "\r" };
        let mut reply = String::new();
        if echo {
            reply.push_str(cmd);
            reply.push_str(eol);
        }
        let Some(body) = body else {
            // Swallowed: no prompt either (adapter "hangs").
            self.out.extend(reply.as_bytes());
            return;
        };
        for l in body {
            reply.push_str(&l);
            reply.push_str(eol);
        }
        if !st.monitoring {
            reply.push_str(eol);
            reply.push('>');
        }
        self.out.extend(reply.as_bytes());
    }
}

impl ByteLink for SimAdapter {
    fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError> {
        if self.state.lock().unwrap().disconnected {
            return Err(LinkError::Disconnected);
        }
        for &b in data {
            {
                let mut st = self.state.lock().unwrap();
                if st.monitoring {
                    // Any character stops monitoring.
                    st.monitoring = false;
                    self.out.extend(b"\r>");
                    continue;
                }
            }
            match b {
                b'\r' => {
                    let cmd = String::from_utf8_lossy(&std::mem::take(&mut self.line))
                        .trim()
                        .to_string();
                    if cmd.is_empty() {
                        self.out.push_back(b'>');
                    } else {
                        self.handle_line(&cmd);
                    }
                }
                b'\n' => {}
                _ => self.line.push(b),
            }
        }
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, LinkError> {
        if self.state.lock().unwrap().disconnected {
            return Err(LinkError::Disconnected);
        }
        if self.out.is_empty() {
            std::thread::sleep(timeout.min(Duration::from_millis(10)));
            return Ok(0);
        }
        let n = self.out.len().min(buf.len());
        for slot in buf.iter_mut().take(n) {
            *slot = self.out.pop_front().unwrap();
        }
        Ok(n)
    }

    fn info(&self) -> LinkInfo {
        let st = self.state.lock().unwrap();
        LinkInfo {
            kind: LinkKind::Simulated,
            name: format!("Simulator: {}", st.vehicle.profile.name),
            simulated: true,
        }
    }
}

impl State {
    fn new(profile: VehicleProfile, settings: SimSettings) -> Self {
        State {
            vehicle: Vehicle {
                profile,
                cleared: false,
            },
            settings,
            echo: true,
            linefeeds: true,
            spaces: true,
            headers: false,
            protocol_choice: None,
            connected: false,
            started: Instant::now(),
            disconnected: false,
            requests: 0,
            j1939_plain_header: false,
            monitoring: false,
        }
    }

    /// Every ECU's answer messages to one request, for the K-line simulator.
    pub(crate) fn answers(&mut self, req: &[u8]) -> Vec<(u32, Vec<u8>)> {
        let t = self.started.elapsed().as_secs_f64();
        self.requests += 1;
        let ecus = self.vehicle.profile.ecus.clone();
        let mut out = Vec::new();
        for ecu in &ecus {
            for m in self.ecu_answer(ecu, req, t) {
                out.push((ecu.address, m));
            }
        }
        out
    }

    fn reset(&mut self) {
        self.echo = true;
        self.linefeeds = true;
        self.spaces = true;
        self.headers = false;
        self.protocol_choice = None;
        self.connected = false;
        self.j1939_plain_header = false;
    }

    fn ok(&self) -> Option<Vec<String>> {
        Some(vec!["OK".into()])
    }

    fn answer(&mut self, raw: &str) -> Option<Vec<String>> {
        let cmd = raw.to_ascii_uppercase().replace(' ', "");
        let clone = self.settings.faults.clone_adapter;
        if let Some(at) = cmd.strip_prefix("AT") {
            return match at {
                "Z" | "WS" | "D" => {
                    self.reset();
                    Some(vec![
                        String::new(),
                        if clone {
                            "ELM327 v1.5".into()
                        } else {
                            "ELM327 v1.4b".into()
                        },
                    ])
                }
                "I" => Some(vec![if clone {
                    "ELM327 v1.5".into()
                } else {
                    "ELM327 v1.4b".into()
                }]),
                "@1" if !clone => Some(vec!["OpenDiag simulator (SIMULATED)".into()]),
                "E0" | "E1" => {
                    self.echo = at == "E1";
                    self.ok()
                }
                "L0" | "L1" => {
                    self.linefeeds = at == "L1";
                    self.ok()
                }
                "S0" | "S1" if !clone => {
                    self.spaces = at == "S1";
                    self.ok()
                }
                "H0" | "H1" if !clone => {
                    self.headers = at == "H1";
                    self.ok()
                }
                "H0" if clone => self.ok(),
                "RV" if !clone => {
                    let v = signals::at(
                        0.0,
                        self.vehicle.profile.engine,
                        self.settings.engine_running,
                        self.settings.faults.low_voltage,
                    )
                    .voltage;
                    Some(vec![format!("{v:.1}V")])
                }
                "DP" => Some(vec![self.vehicle.profile.protocol.label().to_string()]),
                "DPN" => {
                    let n = self.vehicle.profile.protocol.elm_number();
                    Some(vec![if self.protocol_choice.is_none() {
                        format!("A{n}")
                    } else {
                        format!("{n}")
                    }])
                }
                "PC" => {
                    self.connected = false;
                    self.ok()
                }
                "AT0" | "AT1" | "AT2" | "CAF0" | "CAF1" => self.ok(),
                "SPA" => {
                    self.protocol_choice = Some(0xA);
                    self.connected = false;
                    self.ok()
                }
                "JHF0" | "JHF1" => {
                    self.j1939_plain_header = at == "JHF0";
                    self.ok()
                }
                "JE" | "JS" => self.ok(),
                mp if mp.starts_with("MP") && self.j1939_active() => {
                    let arg = &mp[2..];
                    let (pgn, count) = match arg.len() {
                        4 | 6 => (u32::from_str_radix(arg, 16).ok(), None),
                        5 | 7 => (
                            u32::from_str_radix(&arg[..arg.len() - 1], 16).ok(),
                            arg[arg.len() - 1..].parse::<usize>().ok(),
                        ),
                        _ => (None, None),
                    };
                    let Some(pgn) = pgn else {
                        return Some(vec!["?".into()]);
                    };
                    let mut lines = self.j1939_monitor(pgn);
                    match count {
                        Some(n) => {
                            lines.truncate(n.max(1));
                            Some(lines)
                        }
                        None => {
                            self.monitoring = true;
                            Some(lines)
                        }
                    }
                }
                sp if sp.starts_with("SP") => {
                    let n = sp.trim_start_matches("SP").trim_start_matches('A');
                    match u8::from_str_radix(n, 16) {
                        Ok(0) => {
                            self.protocol_choice = None;
                            self.ok()
                        }
                        Ok(n) if ObdProtocol::from_elm_number(n).is_some() => {
                            self.protocol_choice = Some(n);
                            self.ok()
                        }
                        _ => Some(vec!["?".into()]),
                    }
                }
                _ => Some(vec!["?".into()]),
            };
        }
        if cmd.starts_with("ST") {
            return Some(vec!["?".into()]);
        }
        let Some(req) = opendiag_core::parse_hex(&cmd) else {
            return Some(vec!["?".into()]);
        };
        if self.j1939_active() {
            if self.settings.faults.no_response {
                return None;
            }
            self.requests += 1;
            return Some(if req.len() == 3 {
                self.j1939_request(u32::from_be_bytes([0, req[0], req[1], req[2]]))
            } else {
                vec!["NO DATA".into()]
            });
        }
        self.vehicle_request(&req)
    }

    fn vehicle_request(&mut self, req: &[u8]) -> Option<Vec<String>> {
        let latency = self.settings.latency_ms;
        if latency > 0 {
            std::thread::sleep(Duration::from_millis(latency));
        }
        let f = self.settings.faults.clone();
        if let Some(n) = f.disconnect_after {
            if n == 0 {
                self.disconnected = true;
                return None;
            }
            self.settings.faults.disconnect_after = Some(n - 1);
        }
        if f.no_response {
            return None;
        }
        self.requests += 1;
        let proto = self.vehicle.profile.protocol;
        let mut prefix = Vec::new();
        if !self.connected {
            if let Some(choice) = self.protocol_choice {
                if choice != proto.elm_number() {
                    return Some(vec![if ObdProtocol::from_elm_number(choice)
                        .map(|p| p.is_can())
                        .unwrap_or(false)
                    {
                        "CAN ERROR".into()
                    } else {
                        "BUS INIT: ...ERROR".into()
                    }]);
                }
            } else {
                prefix.push("SEARCHING...".to_string());
            }
            if f.ignition_off || self.vehicle.profile.ecus.is_empty() {
                prefix.push("UNABLE TO CONNECT".into());
                return Some(prefix);
            }
            if !proto.is_can() {
                prefix.push("BUS INIT: ...OK".into());
            }
            self.connected = true;
        } else if f.ignition_off {
            return Some(vec!["NO DATA".into()]);
        }
        if f.bus_error {
            prefix.push(if proto.is_can() {
                "CAN ERROR".into()
            } else {
                "BUS ERROR".into()
            });
            return Some(prefix);
        }
        let t = self.started.elapsed().as_secs_f64();
        let ecus = self.vehicle.profile.ecus.clone();
        let mut lines = prefix;
        let mut any = false;
        for ecu in &ecus {
            for msg in self.ecu_answer(ecu, req, t) {
                any = true;
                lines.extend(self.format(ecu.address, &msg, proto));
            }
        }
        if !any {
            lines.push("NO DATA".into());
        }
        if f.malformed {
            if let Some(l) = lines.iter_mut().rev().find(|l| !l.starts_with("SEARCHING")) {
                l.replace_range(l.len().saturating_sub(2).., "G?");
            }
        }
        Some(lines)
    }

    /// The messages one ECU sends back (several for legacy multi-message answers).
    fn ecu_answer(&mut self, ecu: &SimEcu, req: &[u8], t: f64) -> Vec<Vec<u8>> {
        let is_can = self.vehicle.profile.protocol.is_can();
        let running = self.settings.engine_running;
        let sig = signals::at(
            t,
            self.vehicle.profile.engine,
            running,
            self.settings.faults.low_voltage,
        );
        let sid = req[0];
        match sid {
            0x01 | 0x02 if req.len() >= 2 => {
                let pid = req[1];
                let freeze = sid == 0x02;
                if freeze && (self.vehicle.cleared || ecu.freeze_dtc.is_none()) {
                    return vec![];
                }
                let resp_head: Vec<u8> = if freeze {
                    vec![0x42, pid, 0x00]
                } else {
                    vec![0x41, pid]
                };
                let data: Option<Vec<u8>> = if pid.is_multiple_of(0x20) {
                    let supported: Vec<u8> = if freeze {
                        let mut v: Vec<u8> = ecu
                            .pids
                            .iter()
                            .copied()
                            .filter(|p| *p != 0x01 && *p != 0x1C)
                            .collect();
                        v.push(0x02);
                        v
                    } else {
                        ecu.pids.clone()
                    };
                    // Announce the next range when anything above it exists.
                    let mut s = supported.clone();
                    if let Some(next) = pid.checked_add(0x20) {
                        if supported.iter().any(|p| *p > next) {
                            s.push(next);
                        }
                    }
                    let bm = bitmap_for(pid, &s);
                    (bm != [0; 4] || pid == 0).then(|| bm.to_vec())
                } else if freeze && pid == 0x02 {
                    ecu.freeze_dtc
                        .as_ref()
                        .and_then(|d| Dtc(d.clone()).to_bytes())
                        .map(|b| b.to_vec())
                } else if !ecu.pids.contains(&pid) {
                    None
                } else if pid == 0x01 {
                    let diesel = self.vehicle.profile.engine == profiles::Engine::Diesel;
                    let stored = if self.vehicle.cleared {
                        0
                    } else {
                        ecu.stored.len() as u8
                    };
                    let incomplete = if self.vehicle.cleared {
                        ecu.monitors_available
                    } else {
                        ecu.monitors_incomplete
                    };
                    let mil = stored > 0
                        && ecu.address == self.vehicle.profile.ecus[0].address
                        && self.vehicle.profile.kind != profiles::VehicleKind::Motorcycle;
                    Some(
                        readiness::encode(
                            mil,
                            stored,
                            diesel,
                            ecu.monitors_available,
                            incomplete,
                            if self.vehicle.cleared { 0x01 } else { 0 },
                        )
                        .to_vec(),
                    )
                } else {
                    let s = if freeze {
                        // Frozen at the moment of the fault: warm engine at moderate load.
                        signals::at(30.0, self.vehicle.profile.engine, true, false)
                    } else {
                        sig
                    };
                    signals::encode(
                        pid,
                        &s,
                        self.vehicle.profile.year,
                        self.vehicle.profile.engine,
                    )
                };
                match data {
                    Some(d) => {
                        let mut m = resp_head;
                        m.extend(d);
                        vec![m]
                    }
                    None => vec![],
                }
            }
            // Permanent codes (service 0A) only exist on newer vehicles.
            0x0A if self.vehicle.profile.year < 2010 => vec![],
            0x03 | 0x07 | 0x0A => {
                let codes = match sid {
                    0x03 if !self.vehicle.cleared => ecu.stored.clone(),
                    0x07 if !self.vehicle.cleared => ecu.pending.clone(),
                    0x0A => ecu.permanent.clone(),
                    _ => vec![],
                };
                // Only ECUs with emissions monitors answer service 03/07/0A on this sim,
                // like most real vehicles.
                if ecu.pids.contains(&0x01) {
                    dtc_messages(sid + 0x40, &codes, is_can)
                } else {
                    vec![]
                }
            }
            0x04 => {
                if running {
                    vec![vec![0x7F, 0x04, 0x22]]
                } else {
                    if ecu.address == self.vehicle.profile.ecus[0].address {
                        self.vehicle.cleared = true;
                    }
                    vec![vec![0x44]]
                }
            }
            0x09 if req.len() >= 2 => {
                let text = |s: &str| s.as_bytes().to_vec();
                let wrong = self.settings.faults.wrong_vin;
                let (pid, body): (u8, Option<Vec<u8>>) = match req[1] {
                    0x00 => {
                        let mut s = vec![];
                        if ecu.vin.is_some() {
                            s.push(0x02);
                        }
                        if ecu.calid.is_some() {
                            s.push(0x04);
                        }
                        if ecu.ecu_name.is_some() {
                            s.push(0x0A);
                        }
                        (0x00, Some(bitmap_for(0, &s).to_vec()))
                    }
                    0x02 => (
                        0x02,
                        ecu.vin.as_ref().map(|v| {
                            if wrong {
                                text("SMLTD0OIQ00000001")
                            } else {
                                text(v)
                            }
                        }),
                    ),
                    0x04 => (
                        0x04,
                        ecu.calid.as_ref().map(|c| {
                            let mut b = text(c);
                            b.resize(16, 0);
                            b
                        }),
                    ),
                    0x0A => (
                        0x0A,
                        ecu.ecu_name.as_ref().map(|n| {
                            let mut b = text(n);
                            b.resize(20, 0);
                            b
                        }),
                    ),
                    _ => (0, None),
                };
                match body {
                    None => vec![],
                    Some(b) if pid == 0x00 => vec![[vec![0x49, 0x00], b].concat()],
                    Some(b) if is_can => vec![[vec![0x49, pid, 0x01], b].concat()],
                    Some(mut b) => {
                        // Legacy: numbered 4-byte messages; VIN padded at the front to 20 bytes.
                        if pid == 0x02 {
                            let mut padded = vec![0; 20 - b.len().min(20)];
                            padded.append(&mut b);
                            b = padded;
                        }
                        b.chunks(4)
                            .enumerate()
                            .map(|(i, c)| {
                                let mut m = vec![0x49, pid, i as u8 + 1];
                                m.extend_from_slice(c);
                                m.resize(7, 0);
                                m
                            })
                            .collect()
                    }
                }
            }
            0x10..=0x3E | 0x85 | 0x87 if is_can => vec![vec![0x7F, sid, 0x11]],
            _ => vec![],
        }
    }

    /// Turns one message into the text lines an ELM327 prints for it.
    fn format(&self, address: u32, msg: &[u8], proto: ObdProtocol) -> Vec<String> {
        let sep = if self.spaces { " " } else { "" };
        let hex = |b: &[u8]| {
            b.iter()
                .map(|x| format!("{x:02X}"))
                .collect::<Vec<_>>()
                .join(sep)
        };
        if proto.is_can() {
            let frames =
                opendiag_isotp::segment(msg, Some(0x00)).expect("short simulator messages");
            let mut frames = frames;
            if self.settings.faults.truncated && frames.len() > 1 {
                frames.pop();
            }
            let hdr = if proto.is_29bit() {
                let a = 0x18DAF100 | (address & 0xFF);
                if self.spaces {
                    format!(
                        "{:02X} {:02X} {:02X} {:02X}",
                        a >> 24,
                        (a >> 16) & 0xFF,
                        (a >> 8) & 0xFF,
                        a & 0xFF
                    )
                } else {
                    format!("{a:08X}")
                }
            } else {
                format!("{address:03X}")
            };
            if self.headers {
                frames
                    .iter()
                    .map(|f| format!("{hdr}{sep}{}", hex(f)))
                    .collect()
            } else if frames.len() == 1 {
                vec![hex(msg)]
            } else {
                let mut out = vec![format!("{:03X}", msg.len())];
                // First frame carries 6 bytes, the rest 7 each.
                let first = msg.len().min(6);
                out.push(format!("0:{sep}{}", hex(&msg[..first])));
                for (n, chunk) in msg[first..].chunks(7).enumerate() {
                    out.push(format!("{:X}:{sep}{}", (n + 1) % 16, hex(chunk)));
                }
                if self.settings.faults.truncated {
                    out.pop();
                }
                out
            }
        } else {
            let mut frame = vec![0x48, 0x6B, address as u8];
            frame.extend_from_slice(msg);
            let sum = frame.iter().fold(0u8, |a, b| a.wrapping_add(*b));
            frame.push(sum);
            if self.headers {
                vec![hex(&frame)]
            } else {
                vec![hex(msg)]
            }
        }
    }
}

fn dtc_messages(resp_sid: u8, codes: &[String], is_can: bool) -> Vec<Vec<u8>> {
    let bytes: Vec<u8> = codes
        .iter()
        .filter_map(|c| Dtc(c.clone()).to_bytes())
        .flatten()
        .collect();
    if is_can {
        let mut m = vec![resp_sid, codes.len() as u8];
        m.extend(bytes);
        vec![m]
    } else {
        let mut out: Vec<Vec<u8>> = bytes
            .chunks(6)
            .map(|c| {
                let mut m = vec![resp_sid];
                m.extend_from_slice(c);
                m.resize(7, 0);
                m
            })
            .collect();
        if out.is_empty() {
            out.push(vec![resp_sid, 0, 0, 0, 0, 0, 0]);
        }
        out
    }
}

#[cfg(test)]
mod tests;
