//! ELM327-compatible adapter driver.
//!
//! Works over any [`ByteLink`]. It only sends documented ELM327/STN adapter
//! commands (see [`ADAPTER_COMMANDS`]) and diagnostic requests the safety
//! policy allows ([`opendiag_safety::may_send`]); it never sends speculative
//! commands to discover more features.

mod parse;

pub use parse::{parse_lines, ParsedLine};

use opendiag_core::{AdapterCapabilities, AdapterFamily, ByteLink, ObdProtocol, Tri};
use opendiag_isotp::Reassembler;
use opendiag_obd::{response::NegativeResponse, EcuMessage};
use opendiag_safety::Authorization;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub use opendiag_driver::{Direction, Driver, DriverError as ElmError, LogEntry};

#[derive(Debug, Clone, Copy)]
pub struct Timeouts {
    pub reset: Duration,
    pub command: Duration,
    pub request: Duration,
    /// The first request with automatic protocol search (5-baud init can take seconds).
    pub search: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            reset: Duration::from_secs(4),
            command: Duration::from_millis(1500),
            request: Duration::from_millis(2500),
            search: Duration::from_secs(15),
        }
    }
}

/// Documented adapter (not vehicle) commands this driver may send. Commands
/// that store data in the adapter (`AT@3`, `ATPP…`) are deliberately absent.
pub const ADAPTER_COMMANDS: &[&str] = &[
    "ATZ", "ATWS", "ATD", "ATI", "AT@1", "ATE0", "ATE1", "ATL0", "ATL1", "ATS0", "ATS1", "ATH0",
    "ATH1", "ATDP", "ATDPN", "ATRV", "ATAT0", "ATAT1", "ATAT2", "ATCAF0", "ATCAF1", "ATPC", "STI",
    "STDI", "ATSP0", "ATSP1", "ATSP2", "ATSP3", "ATSP4", "ATSP5", "ATSP6", "ATSP7", "ATSP8",
    "ATSP9", "ATSPA1", "ATSPA2", "ATSPA3", "ATSPA4", "ATSPA5", "ATSPA6", "ATSPA7", "ATSPA8",
    "ATSPA9",
];

const LOG_LIMIT: usize = 4000;

pub struct Elm327 {
    link: Box<dyn ByteLink>,
    caps: AdapterCapabilities,
    protocol: Option<ObdProtocol>,
    headers: bool,
    pub timeouts: Timeouts,
    started: Instant,
    log: Vec<LogEntry>,
    /// Failed requests in a row (timeouts, garbled answers, bus errors).
    pub consecutive_errors: u32,
    /// Hide VIN answers in the log (default on).
    pub redact_vin: bool,
}

impl Elm327 {
    /// Resets the adapter and finds out what it supports. Does not talk to the vehicle.
    pub fn open(link: Box<dyn ByteLink>) -> Result<Self, ElmError> {
        Self::open_with(link, Timeouts::default())
    }

    pub fn open_with(link: Box<dyn ByteLink>, timeouts: Timeouts) -> Result<Self, ElmError> {
        let info = link.info();
        let mut elm = Self {
            caps: AdapterCapabilities::unknown(info.kind),
            link,
            protocol: None,
            headers: false,
            timeouts,
            started: Instant::now(),
            log: Vec::new(),
            consecutive_errors: 0,
            redact_vin: true,
        };
        elm.note(format!("Opening {} ({:?})", info.name, info.kind));
        elm.initialise(info.simulated)?;
        Ok(elm)
    }

    fn initialise(&mut self, simulated: bool) -> Result<(), ElmError> {
        // Some adapters keep a half-typed line from before; a lone CR clears it.
        let _ = self.link.write_all(b"\r");
        let _ = self.read_until_prompt(Duration::from_millis(300));
        let reset = self.adapter_raw("ATZ", self.timeouts.reset)?;
        let identity = reset
            .iter()
            .find(|l| {
                l.to_ascii_uppercase().contains("ELM") || l.contains("STN") || l.contains("OBD")
            })
            .cloned();
        let identity = match identity {
            Some(i) => i,
            None => {
                // ATZ may only echo; ask ATI.
                let i = self.adapter_raw("ATI", self.timeouts.command)?;
                i.into_iter()
                    .find(|l| l.to_ascii_uppercase().contains("ELM"))
                    .ok_or_else(|| ElmError::NotElm(reset.join(" ")))?
            }
        };
        self.caps.identity = identity.trim().to_string();
        self.caps.family = if simulated {
            AdapterFamily::Simulated
        } else {
            AdapterFamily::Elm327
        };
        if self.caps.identity.contains("v1.5") {
            self.caps.clone_suspected = true;
            self.caps.notes.push(
                "Reports version 1.5, which ELM Electronics never released: very likely a clone. \
                 Clones differ in which commands they really support."
                    .into(),
            );
        }
        // Formatting: no echo, no line feeds, no spaces, headers on.
        for (cmd, required) in [("ATE0", true), ("ATL0", false)] {
            if !self.adapter_ok(cmd)? && required {
                return Err(ElmError::Unsupported(cmd.into()));
            }
        }
        self.caps.spaces_off = tri(self.adapter_ok("ATS0")?);
        self.headers = self.adapter_ok("ATH1")?;
        self.caps.headers = tri(self.headers);
        if !self.headers {
            self.caps.clone_suspected = true;
            self.caps.notes.push(
                "Does not support showing message headers (ATH1): answers from several ECUs cannot be told apart.".into(),
            );
        }
        if let Ok(d) = self.adapter_raw("AT@1", self.timeouts.command) {
            if let Some(d) = d.first().filter(|d| d.as_str() != "?") {
                self.caps.description = Some(d.clone());
            }
        }
        if let Ok(s) = self.adapter_raw("STI", self.timeouts.command) {
            if let Some(s) = s
                .first()
                .filter(|s| s.as_str() != "?" && s.starts_with("STN"))
            {
                self.caps.stn_firmware = Some(s.clone());
                if !simulated {
                    self.caps.family = AdapterFamily::Stn;
                }
            }
        }
        self.caps.voltage_reading = tri(self.read_voltage().is_some());
        self.caps.protocols = ObdProtocol::ALL.to_vec();
        self.caps.can_29bit = Tri::Yes;
        self.caps.max_can_frame_bytes = Some(8);
        self.note(format!("Adapter: {}", self.caps.identity));
        Ok(())
    }

    pub fn capabilities(&self) -> &AdapterCapabilities {
        &self.caps
    }

    pub fn protocol(&self) -> Option<ObdProtocol> {
        self.protocol
    }

    pub fn is_can(&self) -> bool {
        self.protocol.map(|p| p.is_can()).unwrap_or(true)
    }

    pub fn log(&self) -> &[LogEntry] {
        &self.log
    }

    pub fn link_info(&self) -> opendiag_core::LinkInfo {
        self.link.info()
    }

    /// Voltage at the adapter's power pin (OBD pin 16 = battery), via `ATRV`.
    pub fn read_voltage(&mut self) -> Option<f64> {
        let lines = self.adapter_raw("ATRV", self.timeouts.command).ok()?;
        let l = lines.first()?;
        l.trim_end_matches(['V', 'v']).trim().parse::<f64>().ok()
    }

    /// Connects to the vehicle: sets the protocol (`None` = automatic search),
    /// sends the read-only "supported PIDs" request 01 00 and asks which protocol was found.
    /// Returns the protocol and the 01 00 answers.
    pub fn connect_vehicle(
        &mut self,
        protocol: Option<ObdProtocol>,
    ) -> Result<(ObdProtocol, Vec<EcuMessage>), ElmError> {
        let sp = match protocol {
            None => "ATSP0".to_string(),
            Some(p) => format!("ATSP{}", p.elm_number()),
        };
        if !self.adapter_ok(&sp)? {
            return Err(ElmError::Unsupported(sp));
        }
        let lines = self.exchange("0100", self.timeouts.search)?;
        let found = self.detect_protocol()?;
        self.protocol = Some(found);
        self.caps.vehicle_protocol = Some(found);
        self.note(format!("Vehicle protocol: {}", found.label()));
        let messages = self.messages_from(&lines, &[0x01, 0x00])?;
        Ok((found, messages))
    }

    fn detect_protocol(&mut self) -> Result<ObdProtocol, ElmError> {
        let lines = self.adapter_raw("ATDPN", self.timeouts.command)?;
        let l = lines
            .first()
            .ok_or_else(|| ElmError::Garbled("empty ATDPN".into()))?;
        let digit = l
            .trim_start_matches(['A', 'a'])
            .chars()
            .next()
            .and_then(|c| c.to_digit(16));
        digit
            .and_then(|d| ObdProtocol::from_elm_number(d as u8))
            .ok_or_else(|| ElmError::Garbled(format!("unknown protocol \"{l}\"")))
    }

    /// Sends a documented adapter command and returns its answer lines.
    pub fn adapter_command(&mut self, cmd: &str) -> Result<Vec<String>, ElmError> {
        let cmd = cmd.trim().to_ascii_uppercase().replace(' ', "");
        if !ADAPTER_COMMANDS.contains(&cmd.as_str()) {
            return Err(ElmError::Blocked(format!(
                "\"{cmd}\" is not on the list of adapter commands OpenDiag sends."
            )));
        }
        self.adapter_raw(&cmd, self.timeouts.command)
    }

    /// Sends a diagnostic request to the vehicle and returns each ECU's answer.
    /// Requests that change the vehicle need an [`Authorization`] from the safety policy.
    pub fn request(
        &mut self,
        payload: &[u8],
        auth: Option<&Authorization>,
    ) -> Result<Vec<EcuMessage>, ElmError> {
        opendiag_safety::may_send(payload, auth).map_err(ElmError::Blocked)?;
        if self.protocol.is_none() {
            return Err(ElmError::Garbled("not connected to the vehicle yet".into()));
        }
        let text: String = payload.iter().map(|b| format!("{b:02X}")).collect();
        let result = self
            .exchange(&text, self.timeouts.request)
            .and_then(|lines| self.messages_from(&lines, payload));
        match &result {
            Ok(_) | Err(ElmError::NoData) => self.consecutive_errors = 0,
            Err(_) => self.consecutive_errors += 1,
        }
        result
    }

    /// Closes the connection to the vehicle and the link.
    pub fn close(&mut self) {
        let _ = self.adapter_raw("ATPC", Duration::from_millis(500));
        self.link.close();
        self.note("Closed".into());
    }

    // ── internals ───────────────────────────────────────────────────────────

    fn messages_from(
        &mut self,
        lines: &[String],
        payload: &[u8],
    ) -> Result<Vec<EcuMessage>, ElmError> {
        if lines.iter().any(|l| l == "?") {
            return Err(ElmError::Unsupported(opendiag_core::hex(payload)));
        }
        let parsed = parse_lines(lines, self.protocol, self.headers)?;
        let mut out = Vec::new();
        if self.protocol.map(|p| p.is_can()).unwrap_or(false) && self.headers {
            let mut by_ecu: BTreeMap<String, Reassembler> = BTreeMap::new();
            for line in parsed {
                let r = by_ecu.entry(line.ecu.clone()).or_default();
                match r.push(&line.data) {
                    Ok(Some(msg)) => out.push(EcuMessage {
                        ecu: line.ecu,
                        data: msg,
                    }),
                    Ok(None) => {}
                    Err(e) => return Err(ElmError::Garbled(format!("{e} from {}", line.ecu))),
                }
            }
            if by_ecu.values().any(|r| r.in_progress()) {
                return Err(ElmError::Garbled("a multi-frame answer was cut off".into()));
            }
            if out.iter().any(|m| m.data.len() > 7) {
                self.caps.multi_frame = Tri::Yes;
            }
        } else {
            out = parsed
                .into_iter()
                .map(|l| EcuMessage {
                    ecu: l.ecu,
                    data: l.data,
                })
                .collect();
        }
        // An ECU that is still working on the answer ("response pending") is not an answer.
        out.retain(|m| {
            !NegativeResponse::parse(&m.data)
                .map(|n| n.is_pending())
                .unwrap_or(false)
        });
        if out.is_empty() {
            return Err(ElmError::NoData);
        }
        if self.redact_vin && payload.starts_with(&[0x09, 0x02]) {
            self.note("[VIN answer hidden in the log]".into());
        } else {
            for m in &out {
                self.log_entry(
                    Direction::Rx,
                    format!("{}: {}", m.ecu, opendiag_core::hex(&m.data)),
                );
            }
        }
        Ok(out)
    }

    fn adapter_ok(&mut self, cmd: &str) -> Result<bool, ElmError> {
        match self.adapter_raw(cmd, self.timeouts.command) {
            Ok(lines) => Ok(lines.iter().any(|l| l == "OK")),
            Err(ElmError::Unsupported(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn adapter_raw(&mut self, cmd: &str, timeout: Duration) -> Result<Vec<String>, ElmError> {
        let lines = self.exchange(cmd, timeout)?;
        for l in &lines {
            self.log_entry(Direction::Rx, l.clone());
        }
        if lines.iter().any(|l| l == "?") {
            return Err(ElmError::Unsupported(cmd.into()));
        }
        Ok(lines)
    }

    /// Sends one line and returns the answer lines (echo and status lines removed).
    fn exchange(&mut self, cmd: &str, timeout: Duration) -> Result<Vec<String>, ElmError> {
        self.log_entry(Direction::Tx, cmd.to_string());
        self.link.write_all(format!("{cmd}\r").as_bytes())?;
        let raw = self
            .read_until_prompt(timeout)?
            .ok_or_else(|| ElmError::Timeout(cmd.into()))?;
        let lines: Vec<String> = raw
            .split(['\r', '\n'])
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .filter(|l| l.replace(' ', "") != cmd.replace(' ', ""))
            .filter(|l| {
                !l.starts_with("SEARCHING") && !(l.starts_with("BUS INIT") && !l.contains("ERROR"))
            })
            .collect();
        check_errors(&lines)?;
        Ok(lines)
    }

    /// Reads until the `>` prompt. `Ok(None)` on timeout.
    fn read_until_prompt(&mut self, timeout: Duration) -> Result<Option<String>, ElmError> {
        let deadline = Instant::now() + timeout;
        let mut acc = Vec::new();
        let mut buf = [0u8; 256];
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let n = self.link.read(&mut buf, deadline - now)?;
            acc.extend(buf[..n].iter().copied().filter(|b| *b != 0));
            if let Some(pos) = acc.iter().position(|b| *b == b'>') {
                acc.truncate(pos);
                return Ok(Some(String::from_utf8_lossy(&acc).into_owned()));
            }
        }
    }

    fn note(&mut self, text: String) {
        self.log_entry(Direction::Note, text);
    }

    fn log_entry(&mut self, dir: Direction, text: String) {
        if self.log.len() >= LOG_LIMIT {
            self.log.drain(..LOG_LIMIT / 4);
        }
        self.log.push(LogEntry {
            ms: self.started.elapsed().as_millis() as u64,
            dir,
            text,
        });
    }
}

fn tri(b: bool) -> Tri {
    if b {
        Tri::Yes
    } else {
        Tri::No
    }
}

fn check_errors(lines: &[String]) -> Result<(), ElmError> {
    for l in lines {
        let u = l.to_ascii_uppercase();
        let u = u.trim();
        if u == "NO DATA" {
            return Err(ElmError::NoData);
        }
        if u.contains("UNABLE TO CONNECT") {
            return Err(ElmError::UnableToConnect);
        }
        for bus in [
            "CAN ERROR",
            "BUS ERROR",
            "BUS BUSY",
            "FB ERROR",
            "DATA ERROR",
            "BUFFER FULL",
            "LV RESET",
            "ACT ALERT",
            "STOPPED",
            "RX ERROR",
        ] {
            if u.contains(bus) {
                return Err(ElmError::Bus(bus.to_string()));
            }
        }
        if u.starts_with("BUS INIT") && u.contains("ERROR") {
            return Err(ElmError::Bus(u.to_string()));
        }
        if u.len() == 5 && u.starts_with("ERR") {
            return Err(ElmError::Bus(u.to_string()));
        }
    }
    Ok(())
}

impl Driver for Elm327 {
    fn capabilities(&self) -> &AdapterCapabilities {
        Elm327::capabilities(self)
    }
    fn protocol(&self) -> Option<ObdProtocol> {
        Elm327::protocol(self)
    }
    fn is_can(&self) -> bool {
        Elm327::is_can(self)
    }
    fn link_info(&self) -> opendiag_core::LinkInfo {
        Elm327::link_info(self)
    }
    fn read_voltage(&mut self) -> Option<f64> {
        Elm327::read_voltage(self)
    }
    fn connect_vehicle(
        &mut self,
        protocol: Option<ObdProtocol>,
    ) -> Result<(ObdProtocol, Vec<EcuMessage>), ElmError> {
        Elm327::connect_vehicle(self, protocol)
    }
    fn request(
        &mut self,
        payload: &[u8],
        auth: Option<&Authorization>,
    ) -> Result<Vec<EcuMessage>, ElmError> {
        Elm327::request(self, payload, auth)
    }
    fn log(&self) -> &[LogEntry] {
        Elm327::log(self)
    }
    fn consecutive_errors(&self) -> u32 {
        self.consecutive_errors
    }
    fn set_redact_vin(&mut self, on: bool) {
        self.redact_vin = on;
    }
    fn close(&mut self) {
        Elm327::close(self)
    }
}

#[cfg(test)]
mod tests;
