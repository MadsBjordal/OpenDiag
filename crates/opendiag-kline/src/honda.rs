//! Experimental, read-only Honda motorcycle K-line mode (4-pin red DLC).
//!
//! Based only on open-source projects (see docs/connectors.md), not on Honda
//! documentation, and **not verified on a real bike by OpenDiag**. Sources
//! disagree on the wake-up message, so both variants are offered. Only the
//! wake-up, the initialise message and table reads can be sent
//! ([`opendiag_safety::may_send_honda`]). Table contents are shown as raw bytes:
//! their meaning differs per model and is not decoded.

use crate::{frame, hold, BAUD};
use opendiag_core::{KLinePort, LinkError, LinkInfo};
use opendiag_driver::{Direction, DriverError, Log, LogEntry};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wake {
    /// `FE 04 FF FF` (sophienyaa/Honda-Motorcycle-ECU-Tools, gonzos.net).
    FfFf,
    /// `FE 04 72 8C` (AutotronicCommunity/Honda_Keihin_KLine_Protocol).
    Ping72,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeResult {
    pub awake: bool,
    pub wake_reply: String,
    pub init_reply: String,
}

pub struct HondaKLine {
    port: Box<dyn KLinePort>,
    log: Log,
    echo: Option<bool>,
    last: Instant,
}

fn kerr(e: LinkError) -> DriverError {
    DriverError::Link(e)
}

impl HondaKLine {
    pub fn open(mut port: Box<dyn KLinePort>) -> Result<Self, DriverError> {
        port.set_baud(BAUD).map_err(kerr)?;
        port.set_break(false).map_err(kerr)?;
        Ok(Self {
            port,
            log: Log::default(),
            echo: None,
            last: Instant::now(),
        })
    }

    pub fn info(&self) -> LinkInfo {
        self.port.info()
    }

    pub fn log(&self) -> &[LogEntry] {
        self.log.entries()
    }

    fn send(&mut self, f: &[u8]) -> Result<(), DriverError> {
        opendiag_safety::may_send_honda(f).map_err(DriverError::Blocked)?;
        self.log.push(Direction::Tx, opendiag_core::hex(f));
        self.port.clear_input().map_err(kerr)?;
        self.port.write(f).map_err(kerr)?;
        if self.echo != Some(false) {
            let echo = self.read_n(f.len(), Duration::from_millis(80))?;
            match (self.echo, echo.len()) {
                (None, 0) => self.echo = Some(false),
                _ if echo == f => self.echo = Some(true),
                _ => {
                    return Err(DriverError::Bus(format!(
                        "echo mismatch: line showed {}",
                        opendiag_core::hex(&echo)
                    )))
                }
            }
        }
        Ok(())
    }

    fn read_n(&mut self, n: usize, timeout: Duration) -> Result<Vec<u8>, DriverError> {
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        let mut buf = [0u8; 64];
        while out.len() < n && Instant::now() < deadline {
            let k = self
                .port
                .read(
                    &mut buf[..(n - out.len()).min(64)],
                    deadline - Instant::now(),
                )
                .map_err(kerr)?;
            out.extend_from_slice(&buf[..k]);
        }
        Ok(out)
    }

    /// Reads one answer: `<type> <total length> … <cs>`.
    fn answer(&mut self, timeout: Duration) -> Result<Vec<u8>, DriverError> {
        let head = self.read_n(2, timeout)?;
        if head.len() < 2 {
            return Ok(head);
        }
        let total = head[1] as usize;
        let mut msg = head;
        if total > 2 {
            msg.extend(self.read_n(total - 2, Duration::from_millis(200))?);
        }
        self.log.push(Direction::Rx, opendiag_core::hex(&msg));
        self.last = Instant::now();
        Ok(msg)
    }

    /// Wake-up: K-line low 70 ms, high 120 ms, the wake message, 200 ms, then
    /// `72 05 00 F0 99`. Awake when the bike answers `02 04 00 FA`.
    pub fn wake(&mut self, variant: Wake) -> Result<WakeResult, DriverError> {
        self.port.set_break(false).map_err(kerr)?;
        hold(Duration::from_millis(300));
        self.log.push(
            Direction::Note,
            "Honda wake-up: K-line low 70 ms, high 120 ms".into(),
        );
        self.port.set_break(true).map_err(kerr)?;
        hold(Duration::from_millis(70));
        self.port.set_break(false).map_err(kerr)?;
        hold(Duration::from_millis(120));
        let wake: &[u8] = match variant {
            Wake::FfFf => &[0xFE, 0x04, 0xFF, 0xFF],
            Wake::Ping72 => &[0xFE, 0x04, 0x72, 0x8C],
        };
        self.send(wake)?;
        let wake_reply = self.answer(Duration::from_millis(200))?;
        hold(Duration::from_millis(200));
        self.send(&[0x72, 0x05, 0x00, 0xF0, 0x99])?;
        let init = self.answer(Duration::from_millis(300))?;
        let awake = init == [0x02, 0x04, 0x00, 0xFA];
        self.log.push(
            Direction::Note,
            if awake {
                "The ECU answered the initialise message.".into()
            } else {
                "No valid answer to the initialise message.".into()
            },
        );
        Ok(WakeResult {
            awake,
            wake_reply: opendiag_core::hex(&wake_reply),
            init_reply: opendiag_core::hex(&init),
        })
    }

    /// Reads one data table (`72 05 71 <table>`). Returns the table bytes.
    /// The ECU goes back to sleep after a few seconds without messages.
    pub fn read_table(&mut self, table: u8) -> Result<Vec<u8>, DriverError> {
        if self.last.elapsed() > Duration::from_millis(1500) {
            return Err(DriverError::InitFailed(
                "the ECU has probably gone back to sleep: wake it up again".into(),
            ));
        }
        self.send(&frame::honda_frame(&[0x72, 0x05, 0x71, table]))?;
        let a = self.answer(Duration::from_millis(300))?;
        if a.is_empty() {
            return Err(DriverError::NoData);
        }
        if frame::checksum(&a) != 0 {
            return Err(DriverError::Garbled(format!(
                "checksum error in {}",
                opendiag_core::hex(&a)
            )));
        }
        if a.len() < 5 || a[0] != 0x02 || a[2] != 0x71 || a[3] != table {
            return Err(DriverError::Garbled(format!(
                "unexpected answer {}",
                opendiag_core::hex(&a)
            )));
        }
        Ok(a[4..a.len() - 1].to_vec())
    }

    pub fn close(&mut self) {
        let _ = self.port.set_break(false);
        self.port.close();
    }
}
