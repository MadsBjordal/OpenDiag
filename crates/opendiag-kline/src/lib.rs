//! Raw K-line driver for "KKL" / FTDI-type cables.
//!
//! Such a cable is only a USB-serial chip and a K-line transceiver, so
//! OpenDiag drives the protocol itself:
//!
//! * **ISO 14230-4 fast init**: K-line low 25 ms, high 25 ms, then
//!   StartCommunication `C1 33 F1 81 66` at 10400 baud.
//! * **5-baud init** (ISO 9141-2 and ISO 14230-4): address 0x33 sent at 5 bit/s
//!   by pulling the line low, then sync byte 0x55, two key bytes, the inverted
//!   second key byte back, and the inverted address from the ECU.
//!
//! The K-line is a single wire, so everything sent comes back as an echo; the
//! driver reads and checks it. Every request passes the safety policy.
//!
//! [`honda`] is a separate, experimental read-only mode for Honda motorcycles.

pub mod frame;
pub mod honda;

use opendiag_core::{
    AdapterCapabilities, AdapterFamily, KLinePort, LinkError, LinkInfo, ObdProtocol, Tri,
};
use opendiag_driver::{Direction, Driver, DriverError, Log, LogEntry};
use opendiag_obd::EcuMessage;
use opendiag_safety::Authorization;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const BAUD: u32 = 10400;

/// Waits `d` accurately enough for K-line timing (sleep, then spin for the last ms).
pub fn hold(d: Duration) {
    let end = Instant::now() + d;
    if d > Duration::from_millis(3) {
        std::thread::sleep(d - Duration::from_millis(2));
    }
    while Instant::now() < end {
        std::hint::spin_loop();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitMode {
    /// Fast init first, then 5-baud.
    Auto,
    FastInit,
    FiveBaud,
}

impl InitMode {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "auto" | "" => Self::Auto,
            "fast" | "kwp-fast" | "iso14230-fast" => Self::FastInit,
            "5baud" | "slow" | "iso9141" | "iso14230-slow" => Self::FiveBaud,
            _ => return None,
        })
    }
}

/// K-line timing (ISO 9141-2 / ISO 14230-2 values, with slack for USB latency).
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Bus idle before a wake-up (W5 ≥ 300 ms).
    pub idle_before_init: Duration,
    /// Tester inter-byte time for ISO 9141-2 (P4, 5–20 ms).
    pub p4_iso9141: Duration,
    /// Wait for the first answer byte (P2 max 50 ms + USB latency).
    pub first_byte: Duration,
    /// Silence that ends the answers to one request.
    pub end_of_answers: Duration,
    /// Minimum pause before the next request (P3 ≥ 55 ms).
    pub p3_min: Duration,
    /// Re-initialise when idle this long (P3 max is 5 s).
    pub reinit_after: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            idle_before_init: Duration::from_millis(300),
            p4_iso9141: Duration::from_millis(5),
            first_byte: Duration::from_millis(250),
            end_of_answers: Duration::from_millis(70),
            p3_min: Duration::from_millis(55),
            reinit_after: Duration::from_millis(4500),
        }
    }
}

/// Generic OBD-II over a raw K-line cable.
pub struct KLineObd {
    port: Box<dyn KLinePort>,
    caps: AdapterCapabilities,
    protocol: Option<ObdProtocol>,
    mode: InitMode,
    echo: Option<bool>,
    /// Answer bytes read while looking for an echo that was not there.
    stash: Vec<u8>,
    last_activity: Instant,
    log: Log,
    errors: u32,
    redact_vin: bool,
    pub timing: Timing,
}

fn kerr(e: LinkError) -> DriverError {
    DriverError::Link(e)
}

impl KLineObd {
    pub fn open(mut port: Box<dyn KLinePort>, mode: InitMode) -> Result<Self, DriverError> {
        port.set_baud(BAUD).map_err(kerr)?;
        port.set_break(false).map_err(kerr)?;
        let info = port.info();
        let mut caps = AdapterCapabilities::unknown(info.kind);
        caps.family = if info.simulated {
            AdapterFamily::Simulated
        } else {
            AdapterFamily::RawKLine
        };
        caps.identity = format!("Raw K-line cable ({})", info.name);
        caps.protocols = vec![
            ObdProtocol::Iso9141_2,
            ObdProtocol::Iso14230Slow,
            ObdProtocol::Iso14230Fast,
        ];
        caps.headers = Tri::Yes;
        caps.voltage_reading = Tri::No;
        caps.multi_frame = Tri::Yes;
        caps.can_29bit = Tri::No;
        caps.notes.push(
            "Raw K-line cable: OpenDiag sends the wake-up and messages itself. It cannot measure battery voltage. \
             On Windows, setting the FTDI latency timer to 1 ms (Device Manager → port → Advanced) improves timing."
                .into(),
        );
        let mut d = Self {
            port,
            caps,
            protocol: None,
            mode,
            echo: None,
            stash: Vec::new(),
            last_activity: Instant::now(),
            log: Log::default(),
            errors: 0,
            redact_vin: true,
            timing: Timing::default(),
        };
        d.log.push(
            Direction::Note,
            format!("Opened {} at {BAUD} baud", info.name),
        );
        Ok(d)
    }

    fn clear(&mut self) -> Result<(), DriverError> {
        self.stash.clear();
        self.port.clear_input().map_err(kerr)
    }

    fn note(&mut self, s: String) {
        self.log.push(Direction::Note, s);
    }

    fn idle_bus(&mut self) -> Result<(), DriverError> {
        self.port.set_break(false).map_err(kerr)?;
        let since = self.last_activity.elapsed();
        if since < self.timing.idle_before_init {
            hold(self.timing.idle_before_init - since);
        }
        Ok(())
    }

    /// Writes bytes and consumes their echo. ISO 9141-2 bytes are spaced by P4.
    fn send(&mut self, bytes: &[u8], spaced: bool) -> Result<(), DriverError> {
        self.log.push(Direction::Tx, opendiag_core::hex(bytes));
        if spaced {
            for b in bytes {
                self.port.write(&[*b]).map_err(kerr)?;
                self.take_echo(&[*b])?;
                hold(self.timing.p4_iso9141);
            }
        } else {
            self.port.write(bytes).map_err(kerr)?;
            self.take_echo(bytes)?;
        }
        self.last_activity = Instant::now();
        Ok(())
    }

    fn take_echo(&mut self, sent: &[u8]) -> Result<(), DriverError> {
        if self.echo == Some(false) {
            return Ok(());
        }
        let got = self.read_exact(
            sent.len(),
            Duration::from_millis(60 + sent.len() as u64 * 2),
        )?;
        match (self.echo, got.len()) {
            _ if got == sent => {
                self.echo = Some(true);
                Ok(())
            }
            (None, _) => {
                // No echo: what arrived (if anything) is already the answer.
                self.echo = Some(false);
                self.stash = got;
                self.note("The cable does not echo what it sends.".into());
                Ok(())
            }
            _ => Err(DriverError::Bus(format!(
                "echo mismatch: sent {}, line showed {} (wrong cable, bus collision or bad connection)",
                opendiag_core::hex(sent),
                opendiag_core::hex(&got)
            ))),
        }
    }

    fn rd(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, DriverError> {
        if !self.stash.is_empty() {
            let n = self.stash.len().min(buf.len());
            buf[..n].copy_from_slice(&self.stash[..n]);
            self.stash.drain(..n);
            return Ok(n);
        }
        self.port.read(buf, timeout).map_err(kerr)
    }

    fn read_exact(&mut self, n: usize, timeout: Duration) -> Result<Vec<u8>, DriverError> {
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        let mut buf = [0u8; 64];
        while out.len() < n {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let k = self.rd(&mut buf[..(n - out.len()).min(64)], deadline - now)?;
            out.extend_from_slice(&buf[..k]);
        }
        Ok(out)
    }

    /// Collects answer bytes until the line has been quiet for `end_of_answers`.
    fn receive(&mut self) -> Result<Vec<u8>, DriverError> {
        let mut out = Vec::new();
        let start = Instant::now();
        let mut last = Instant::now();
        let mut buf = [0u8; 128];
        loop {
            let k = self.rd(&mut buf, Duration::from_millis(5))?;
            if k > 0 {
                out.extend_from_slice(&buf[..k]);
                last = Instant::now();
                continue;
            }
            if out.is_empty() && start.elapsed() > self.timing.first_byte {
                break;
            }
            if !out.is_empty() && last.elapsed() > self.timing.end_of_answers {
                break;
            }
        }
        self.last_activity = Instant::now();
        Ok(out)
    }

    fn wake_fast(&mut self) -> Result<ObdProtocol, DriverError> {
        self.idle_bus()?;
        self.note("Fast init: K-line low 25 ms, high 25 ms".into());
        self.clear()?;
        self.port.set_break(true).map_err(kerr)?;
        hold(Duration::from_millis(25));
        self.port.set_break(false).map_err(kerr)?;
        hold(Duration::from_millis(25));
        self.clear()?;
        self.send(&frame::KWP_START_COMMUNICATION, false)?;
        let raw = self.receive()?;
        if raw.is_empty() {
            return Err(DriverError::InitFailed(
                "no answer to the fast-init StartCommunication request".into(),
            ));
        }
        self.log.push(Direction::Rx, opendiag_core::hex(&raw));
        let frames = frame::split_kwp(&raw).map_err(DriverError::Garbled)?;
        match frames.iter().find(|f| f.data.first() == Some(&0xC1)) {
            Some(f) => {
                self.note(format!(
                    "ECU {:02X} accepted StartCommunication; key bytes {}",
                    f.source,
                    opendiag_core::hex(&f.data[1..])
                ));
                Ok(ObdProtocol::Iso14230Fast)
            }
            None => Err(DriverError::InitFailed(format!(
                "unexpected answer {}",
                opendiag_core::hex(&raw)
            ))),
        }
    }

    fn wake_5baud(&mut self) -> Result<ObdProtocol, DriverError> {
        const ADDRESS: u8 = 0x33;
        const BIT: Duration = Duration::from_millis(200);
        self.idle_bus()?;
        self.note("5-baud init: sending address 33 at 5 bit/s (about 2 s)".into());
        // Start bit, 8 data bits LSB first, stop bit. Low = break.
        self.port.set_break(true).map_err(kerr)?;
        hold(BIT);
        for i in 0..8 {
            self.port.set_break((ADDRESS >> i) & 1 == 0).map_err(kerr)?;
            hold(BIT);
        }
        self.port.set_break(false).map_err(kerr)?;
        hold(BIT);
        self.clear()?;
        let got = self.read_exact(3, Duration::from_millis(600))?;
        self.log.push(Direction::Rx, opendiag_core::hex(&got));
        if got.len() < 3 || got[0] != 0x55 {
            return Err(DriverError::InitFailed(format!(
                "expected sync byte 55 and two key bytes, got \"{}\"",
                opendiag_core::hex(&got)
            )));
        }
        let (kb1, kb2) = (got[1], got[2]);
        hold(Duration::from_millis(30));
        self.send(&[!kb2], false)?;
        let inv = self.read_exact(1, Duration::from_millis(200))?;
        self.log.push(Direction::Rx, opendiag_core::hex(&inv));
        if inv != [!ADDRESS] {
            return Err(DriverError::InitFailed(format!(
                "expected CC from the ECU, got \"{}\"",
                opendiag_core::hex(&inv)
            )));
        }
        self.last_activity = Instant::now();
        let proto = if kb2 == 0x8F {
            ObdProtocol::Iso14230Slow
        } else {
            ObdProtocol::Iso9141_2
        };
        if proto == ObdProtocol::Iso9141_2 && !(kb1 == kb2 && (kb1 == 0x08 || kb1 == 0x94)) {
            self.caps.notes.push(format!(
                "Unusual key bytes {kb1:02X} {kb2:02X}; treating the link as ISO 9141-2."
            ));
        }
        self.note(format!("Key bytes {kb1:02X} {kb2:02X}: {}", proto.label()));
        Ok(proto)
    }

    fn wake(&mut self, proto: Option<ObdProtocol>) -> Result<ObdProtocol, DriverError> {
        let fast_first = match proto {
            Some(ObdProtocol::Iso14230Fast) => return self.wake_fast(),
            Some(ObdProtocol::Iso9141_2) | Some(ObdProtocol::Iso14230Slow) => {
                return self.wake_5baud()
            }
            Some(p) => {
                return Err(DriverError::Unsupported(format!(
                    "{} on a K-line cable",
                    p.label()
                )))
            }
            None => self.mode != InitMode::FiveBaud,
        };
        if fast_first {
            match self.wake_fast() {
                Ok(p) => return Ok(p),
                Err(e) if self.mode == InitMode::FastInit => return Err(e),
                Err(e) => self.note(format!("Fast init failed ({e}); trying 5-baud init")),
            }
        }
        self.wake_5baud()
    }

    fn exchange(&mut self, payload: &[u8]) -> Result<Vec<EcuMessage>, DriverError> {
        let proto = self
            .protocol
            .ok_or_else(|| DriverError::Garbled("not connected to the vehicle yet".into()))?;
        if self.last_activity.elapsed() > self.timing.reinit_after {
            self.note("Idle too long: waking the vehicle again".into());
            self.wake(Some(proto))?;
        }
        let since = self.last_activity.elapsed();
        if since < self.timing.p3_min {
            hold(self.timing.p3_min - since);
        }
        let iso9141 = proto == ObdProtocol::Iso9141_2;
        let req = if iso9141 {
            frame::iso9141_request(payload)
        } else {
            frame::kwp_request(payload)
        }
        .map_err(DriverError::Unsupported)?;
        self.clear()?;
        self.send(&req, iso9141)?;
        let raw = self.receive()?;
        if raw.is_empty() {
            return Err(DriverError::NoData);
        }
        let frames = if iso9141 {
            frame::split_iso9141(&raw)
        } else {
            frame::split_kwp(&raw)
        }
        .map_err(DriverError::Garbled)?;
        let msgs: Vec<EcuMessage> = frames
            .into_iter()
            .map(|f| EcuMessage {
                ecu: format!("{:02X}", f.source),
                data: f.data,
            })
            // An ECU still working on the answer (7F xx 78) is not an answer.
            .filter(|m| !(m.data.len() >= 3 && m.data[0] == 0x7F && m.data[2] == 0x78))
            .collect();
        if self.redact_vin && payload.starts_with(&[0x09, 0x02]) {
            self.note("[VIN answer hidden in the log]".into());
        } else {
            self.log.push(Direction::Rx, opendiag_core::hex(&raw));
        }
        if msgs.is_empty() {
            return Err(DriverError::NoData);
        }
        Ok(msgs)
    }
}

impl Driver for KLineObd {
    fn capabilities(&self) -> &AdapterCapabilities {
        &self.caps
    }
    fn protocol(&self) -> Option<ObdProtocol> {
        self.protocol
    }
    fn is_can(&self) -> bool {
        false
    }
    fn link_info(&self) -> LinkInfo {
        self.port.info()
    }
    fn read_voltage(&mut self) -> Option<f64> {
        None
    }
    fn connect_vehicle(
        &mut self,
        protocol: Option<ObdProtocol>,
    ) -> Result<(ObdProtocol, Vec<EcuMessage>), DriverError> {
        let p = self.wake(protocol)?;
        self.protocol = Some(p);
        self.caps.vehicle_protocol = Some(p);
        let msgs = self.exchange(&[0x01, 0x00])?;
        Ok((p, msgs))
    }
    fn request(
        &mut self,
        payload: &[u8],
        auth: Option<&Authorization>,
    ) -> Result<Vec<EcuMessage>, DriverError> {
        opendiag_safety::may_send(payload, auth).map_err(DriverError::Blocked)?;
        let r = self.exchange(payload);
        match &r {
            Ok(_) | Err(DriverError::NoData) => self.errors = 0,
            Err(_) => self.errors += 1,
        }
        r
    }
    fn log(&self) -> &[LogEntry] {
        self.log.entries()
    }
    fn consecutive_errors(&self) -> u32 {
        self.errors
    }
    fn set_redact_vin(&mut self, on: bool) {
        self.redact_vin = on;
    }
    fn close(&mut self) {
        let _ = self.port.set_break(false);
        self.port.close();
        self.note("Closed".into());
    }
}

#[cfg(test)]
mod tests;
