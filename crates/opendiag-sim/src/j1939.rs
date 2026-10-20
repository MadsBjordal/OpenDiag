//! The simulated J1939 network: answers what an ELM327 in protocol A prints
//! (datasheet "Using J1939"): `AT MP` monitoring and PGN requests.

use crate::profiles::SimJ1939Ecu;
use crate::{signals, State};

const TOOL: u8 = 0xF9;

impl State {
    pub(crate) fn j1939_active(&self) -> bool {
        self.protocol_choice == Some(0xA)
    }

    /// The data one ECU has for a parameter group, if any.
    fn j1939_data(&self, ecu: &SimJ1939Ecu, pgn: u32, t: f64) -> Option<Vec<u8>> {
        let dm = |lamps: u8, codes: &[(u32, u8, u8)]| {
            let mut d = vec![lamps, 0xFF];
            for (spn, fmi, oc) in codes {
                d.extend([
                    *spn as u8,
                    (*spn >> 8) as u8,
                    ((*spn >> 16) as u8) << 5 | (fmi & 0x1F),
                    oc & 0x7F,
                ]);
            }
            if codes.is_empty() {
                d.extend([0, 0, 0, 0]);
            }
            d
        };
        match pgn {
            0xFECA => return Some(dm(ecu.lamps, &ecu.active)),
            0xFECB => {
                return Some(dm(
                    if ecu.previous.is_empty() {
                        0
                    } else {
                        ecu.lamps
                    },
                    &ecu.previous,
                ))
            }
            0xFEEC => {
                let mut v = ecu.vin.clone()?.into_bytes();
                v.push(b'*');
                return Some(v);
            }
            _ => {}
        }
        if !ecu.pgns.contains(&pgn) {
            return None;
        }
        let running = self.settings.engine_running;
        let s = signals::at(
            t,
            self.vehicle.profile.engine,
            running,
            self.settings.faults.low_voltage,
        );
        let u16le = |v: f64| {
            let r = v.round().clamp(0.0, 64255.0) as u16;
            [r as u8, (r >> 8) as u8]
        };
        let u32le = |v: f64| (v.round() as u32).to_le_bytes();
        let mut d = vec![0xFF; 8];
        match pgn {
            0xF004 => d[3..5].copy_from_slice(&u16le(s.rpm / 0.125)),
            0xFEEE => d[0] = (s.coolant + 40.0).round().clamp(0.0, 250.0) as u8,
            0xFEEF => d[3] = if running { 88 } else { 0 },
            0xFEF1 => d[1..3].copy_from_slice(&u16le(s.speed * 256.0)),
            0xFEF2 => d[0..2].copy_from_slice(&u16le(s.fuel_rate / 0.05)),
            0xFEF5 => d[3..5].copy_from_slice(&u16le((14.0 + 273.0) / 0.03125)),
            0xFEF7 => d[4..6].copy_from_slice(&u16le(s.voltage / 0.05)),
            0xFEFC => d[1] = 155,
            0xFEE5 => d[0..4].copy_from_slice(&u32le(12_345.6 / 0.05)),
            0xFEE0 => d[4..8].copy_from_slice(&u32le(456_789.5 / 0.125)),
            _ => return None,
        }
        Some(d)
    }

    fn j1939_header(&self, priority: u8, pf: u8, ps: u8, sa: u8) -> String {
        let sep = if self.spaces { " " } else { "" };
        if self.j1939_plain_header {
            [priority << 2, pf, ps, sa]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(sep)
        } else {
            format!("{priority:X}{sep}0{pf:02X}{ps:02X}{sep}{sa:02X}")
        }
    }

    /// Lines for one message: a single frame, or (more than 8 bytes) the
    /// transport protocol segments. `to` = the tool for a requested answer
    /// (with the byte-count line), `None` = broadcast (BAM).
    fn j1939_lines(&self, sa: u8, pgn: u32, data: &[u8], to: Option<u8>) -> Vec<String> {
        let sep = if self.spaces { " " } else { "" };
        let hex = |b: &[u8]| {
            b.iter()
                .map(|x| format!("{x:02X}"))
                .collect::<Vec<_>>()
                .join(sep)
        };
        if data.len() <= 8 {
            let mut d = data.to_vec();
            d.resize(8, 0xFF);
            return vec![format!(
                "{}{sep}{}",
                self.j1939_header(6, (pgn >> 8) as u8, pgn as u8, sa),
                hex(&d)
            )];
        }
        let mut out = Vec::new();
        if to.is_some() {
            out.push(format!("{:03X}", data.len()));
        }
        for (i, chunk) in data.chunks(7).enumerate() {
            let mut d = vec![(i + 1) as u8];
            d.extend_from_slice(chunk);
            d.resize(8, 0xFF);
            out.push(format!(
                "{}{sep}{}",
                self.j1939_header(7, 0xEB, to.unwrap_or(0xFF), sa),
                hex(&d)
            ));
        }
        if self.settings.faults.truncated {
            out.pop();
        }
        out
    }

    /// `AT MP`: what the bus broadcasts for a PGN (a few messages per ECU).
    pub(crate) fn j1939_monitor(&mut self, pgn: u32) -> Vec<String> {
        if self.settings.faults.ignition_off {
            return vec![];
        }
        let t = self.started.elapsed().as_secs_f64();
        let fast = matches!(pgn, 0xF004 | 0xFEF1 | 0xFEF2);
        let ecus = self.vehicle.profile.j1939.clone();
        let mut lines = Vec::new();
        for _ in 0..if fast { 3 } else { 1 } {
            for e in &ecus {
                // DM1 is broadcast by every ECU; on-request groups are not broadcast.
                let broadcast = pgn == 0xFECA
                    || opendiag_j1939::PARAMS
                        .iter()
                        .any(|d| d.pgn == pgn && d.broadcast);
                if !broadcast {
                    continue;
                }
                if let Some(d) = self.j1939_data(e, pgn, t) {
                    lines.extend(self.j1939_lines(e.address, pgn, &d, None));
                }
            }
        }
        lines
    }

    /// A PGN request (global): every ECU that has it answers; others stay silent.
    pub(crate) fn j1939_request(&mut self, pgn: u32) -> Vec<String> {
        let f = &self.settings.faults;
        if f.ignition_off {
            return vec!["NO DATA".into()];
        }
        if f.bus_error {
            return vec!["CAN ERROR".into()];
        }
        let t = self.started.elapsed().as_secs_f64();
        let ecus = self.vehicle.profile.j1939.clone();
        let mut lines = Vec::new();
        for e in &ecus {
            if let Some(d) = self.j1939_data(e, pgn, t) {
                lines.extend(self.j1939_lines(e.address, pgn, &d, Some(TOOL)));
            }
        }
        if lines.is_empty() {
            lines.push("NO DATA".into());
        }
        lines
    }
}
