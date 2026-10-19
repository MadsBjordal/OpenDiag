//! A simulated vehicle on a raw K-line, for testing the K-line driver.
//!
//! It watches the line itself: a 25 ms low pulse is a fast init, a 70 ms low
//! pulse a Honda wake-up, and a 200 ms-per-bit address a 5-baud init. It echoes
//! every byte like a real single-wire K-line. Labelled SIMULATED.

use crate::profiles::VehicleProfile;
use crate::{SimControl, SimSettings, State};
use opendiag_core::{KLinePort, LinkError, LinkInfo, LinkKind, ObdProtocol};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimKLineMode {
    /// ISO 9141-2 with 5-baud init (key bytes 08 08).
    Iso9141,
    /// ISO 14230-4 with 5-baud init (key bytes E9 8F).
    Kwp5Baud,
    /// ISO 14230-4 with fast init.
    KwpFast,
    /// Honda motorcycle K-line (experimental protocol, see opendiag-kline::honda).
    Honda,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Asleep,
    AwaitStartComm,
    AwaitInvKb2(u8),
    Awake,
    HondaWoken,
    HondaAwake,
}

pub struct SimKLinePort {
    state: Arc<Mutex<State>>,
    mode: SimKLineMode,
    rx: VecDeque<u8>,
    /// (time, line low?) for every change of the line.
    edges: Vec<(Instant, bool)>,
    pending: Vec<u8>,
    phase: Phase,
    /// Echo every byte (real K-line); switch off to test cables without echo.
    pub echo: bool,
}

const FRAME_ECU: u8 = 0x10;

fn checksum(b: &[u8]) -> u8 {
    b.iter().fold(0u8, |a, x| a.wrapping_add(*x))
}

impl SimKLinePort {
    pub fn new(
        mut profile: VehicleProfile,
        settings: SimSettings,
        mode: SimKLineMode,
    ) -> (Self, SimControl) {
        // Answers are formatted for the older protocols (no CAN counts in DTC answers).
        profile.protocol = match mode {
            SimKLineMode::Iso9141 | SimKLineMode::Honda => ObdProtocol::Iso9141_2,
            SimKLineMode::Kwp5Baud => ObdProtocol::Iso14230Slow,
            SimKLineMode::KwpFast => ObdProtocol::Iso14230Fast,
        };
        profile.name = format!("{} via K-line", profile.name);
        let state = Arc::new(Mutex::new(State::new(profile, settings)));
        (
            Self {
                state: state.clone(),
                mode,
                rx: VecDeque::new(),
                edges: Vec::new(),
                pending: Vec::new(),
                phase: Phase::Asleep,
                echo: true,
            },
            SimControl(state),
        )
    }

    fn ignition_off(&self) -> bool {
        self.state.lock().unwrap().settings.faults.ignition_off
    }

    /// Decodes a finished 5-baud address from the recorded line changes.
    fn check_five_baud(&mut self) {
        let Some(last) = self.edges.last().copied() else {
            return;
        };
        if last.1 || last.0.elapsed() < Duration::from_millis(150) {
            return;
        }
        // The start bit is the first low period lasting about 200 ms (shorter
        // pulses, e.g. a fast init tried before, are skipped).
        let start = (0..self.edges.len()).find(|&i| {
            self.edges[i].1
                && self
                    .edges
                    .get(i + 1)
                    .map(|n| n.0 - self.edges[i].0 >= Duration::from_millis(150))
                    .unwrap_or(false)
        });
        let Some(start) = start else { return };
        let t0 = self.edges[start].0;
        let level_low = |t: Instant| -> bool {
            let mut low = false;
            for (et, el) in &self.edges {
                if *et <= t {
                    low = *el;
                }
            }
            low
        };
        let mut addr = 0u8;
        for k in 0..8u64 {
            if !level_low(t0 + Duration::from_millis(200 * (k + 1) + 100)) {
                addr |= 1 << k;
            }
        }
        self.edges.clear();
        if addr != 0x33 || self.ignition_off() {
            return;
        }
        let (kb1, kb2) = match self.mode {
            SimKLineMode::Iso9141 => (0x08, 0x08),
            SimKLineMode::Kwp5Baud => (0xE9, 0x8F),
            _ => return,
        };
        self.rx.extend([0x55, kb1, kb2]);
        self.phase = Phase::AwaitInvKb2(kb2);
    }

    fn reply_obd(&mut self, req: &[u8]) {
        let answers = self.state.lock().unwrap().answers(req);
        for (addr, msg) in answers {
            let mut f = match self.mode {
                SimKLineMode::Iso9141 => vec![0x48, 0x6B, addr as u8],
                _ => vec![0x80 | msg.len() as u8, 0xF1, addr as u8],
            };
            f.extend_from_slice(&msg);
            f.push(checksum(&f));
            self.rx.extend(f);
        }
    }

    fn process(&mut self) {
        loop {
            match self.phase {
                Phase::Asleep => {
                    self.pending.clear();
                    return;
                }
                Phase::AwaitInvKb2(kb2) => {
                    let Some(b) = self.pending.first().copied() else {
                        return;
                    };
                    self.pending.remove(0);
                    if b == !kb2 {
                        self.rx.push_back(!0x33u8);
                        self.phase = Phase::Awake;
                    }
                }
                Phase::AwaitStartComm => {
                    if self.pending.len() < 5 {
                        return;
                    }
                    let f: Vec<u8> = self.pending.drain(..5).collect();
                    if f == [0xC1, 0x33, 0xF1, 0x81, 0x66] {
                        let mut r = vec![0x83, 0xF1, FRAME_ECU, 0xC1, 0xEF, 0x8F];
                        r.push(checksum(&r));
                        self.rx.extend(r);
                        self.phase = Phase::Awake;
                    }
                }
                Phase::Awake => {
                    let p = &self.pending;
                    let len = match self.mode {
                        SimKLineMode::Iso9141 => {
                            if p.len() < 5 || p[..3] != [0x68, 0x6A, 0xF1] {
                                if p.len() >= 3 && p[..3] != [0x68, 0x6A, 0xF1] {
                                    self.pending.clear();
                                }
                                return;
                            }
                            match (5..=p.len().min(11)).find(|&j| checksum(&p[..j - 1]) == p[j - 1])
                            {
                                Some(j) => j,
                                None => return,
                            }
                        }
                        _ => {
                            if p.is_empty() {
                                return;
                            }
                            let n = (p[0] & 0x3F) as usize + 4;
                            if p.len() < n {
                                return;
                            }
                            n
                        }
                    };
                    let f: Vec<u8> = self.pending.drain(..len).collect();
                    if checksum(&f[..f.len() - 1]) != f[f.len() - 1] {
                        continue;
                    }
                    let req = f[3..f.len() - 1].to_vec();
                    self.reply_obd(&req);
                }
                Phase::HondaWoken | Phase::HondaAwake => {
                    if self.pending.len() < 2 {
                        return;
                    }
                    let n = self.pending[1] as usize;
                    if n < 3 || self.pending.len() < n {
                        if n < 3 {
                            self.pending.clear();
                        }
                        return;
                    }
                    let f: Vec<u8> = self.pending.drain(..n).collect();
                    if checksum(&f) != 0 {
                        continue;
                    }
                    let reply = |head: &[u8]| {
                        let mut r = head.to_vec();
                        r.push(0u8.wrapping_sub(checksum(&r)));
                        r
                    };
                    match f.as_slice() {
                        [0xFE, 0x04, 0x72, _] => self.rx.extend(reply(&[0x0E, 0x04, 0x72])),
                        [0xFE, 0x04, 0xFF, _] => {}
                        [0x72, 0x05, 0x00, 0xF0, _] => {
                            self.rx.extend(reply(&[0x02, 0x04, 0x00]));
                            self.phase = Phase::HondaAwake;
                        }
                        [0x72, 0x05, 0x71, table, _] if self.phase == Phase::HondaAwake => {
                            // Simulated table contents: just numbered bytes, clearly not real data.
                            let mut head = vec![0x02, 0, 0x71, *table];
                            head.extend((0..16u8).map(|i| i.wrapping_mul(*table)));
                            head[1] = head.len() as u8 + 1;
                            self.rx.extend(reply(&head));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

impl KLinePort for SimKLinePort {
    fn write(&mut self, data: &[u8]) -> Result<(), LinkError> {
        self.check_five_baud();
        if self.echo {
            self.rx.extend(data.iter().copied());
        }
        if !self.ignition_off() {
            self.pending.extend_from_slice(data);
            self.process();
        }
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, LinkError> {
        self.check_five_baud();
        if self.rx.is_empty() {
            std::thread::sleep(timeout.min(Duration::from_millis(5)));
            return Ok(0);
        }
        let n = self.rx.len().min(buf.len());
        for slot in buf.iter_mut().take(n) {
            *slot = self.rx.pop_front().unwrap();
        }
        Ok(n)
    }

    fn set_break(&mut self, low: bool) -> Result<(), LinkError> {
        let now = Instant::now();
        if !low {
            if let Some((t, true)) = self.edges.last().copied() {
                let held = now - t;
                if held >= Duration::from_millis(15)
                    && held <= Duration::from_millis(40)
                    && self.mode == SimKLineMode::KwpFast
                    && !self.ignition_off()
                {
                    self.phase = Phase::AwaitStartComm;
                    self.pending.clear();
                } else if held >= Duration::from_millis(50)
                    && held <= Duration::from_millis(110)
                    && self.mode == SimKLineMode::Honda
                    && !self.ignition_off()
                {
                    self.phase = Phase::HondaWoken;
                    self.pending.clear();
                }
            }
        }
        if self.edges.last().map(|e| e.1) != Some(low) {
            self.edges.push((now, low));
        }
        if self.edges.len() > 64 {
            self.edges.drain(..32);
        }
        Ok(())
    }

    fn set_baud(&mut self, _baud: u32) -> Result<(), LinkError> {
        Ok(())
    }

    fn clear_input(&mut self) -> Result<(), LinkError> {
        self.check_five_baud();
        // The sync and key bytes of a 5-baud init arrive right after the
        // address; the driver clears the input just before reading them.
        if !matches!(self.phase, Phase::AwaitInvKb2(_)) {
            self.rx.clear();
        }
        Ok(())
    }

    fn info(&self) -> LinkInfo {
        LinkInfo {
            kind: LinkKind::Simulated,
            name: format!("Simulated K-line ({:?})", self.mode),
            simulated: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;

    #[test]
    fn decodes_five_baud_address() {
        let (mut p, _) = SimKLinePort::new(
            profiles::legacy_car(),
            SimSettings::default(),
            SimKLineMode::Iso9141,
        );
        p.set_break(false).unwrap();
        let bit = Duration::from_millis(200);
        p.set_break(true).unwrap();
        std::thread::sleep(bit);
        for i in 0..8 {
            p.set_break((0x33u8 >> i) & 1 == 0).unwrap();
            std::thread::sleep(bit);
        }
        p.set_break(false).unwrap();
        std::thread::sleep(bit);
        p.clear_input().unwrap();
        let mut buf = [0u8; 8];
        let n = p.read(&mut buf, Duration::from_millis(50)).unwrap();
        assert_eq!(&buf[..n], &[0x55, 0x08, 0x08], "edges: {:?}", p.edges.len());
    }
}
