//! Parsing the lines an ELM327 prints in J1939 mode (headers on).

use crate::J1939Message;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A negative acknowledgement: the ECU says it will not answer this PGN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nack {
    pub source: u8,
    pub pgn: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parsed {
    pub messages: Vec<J1939Message>,
    pub nacks: Vec<Nack>,
}

struct Header {
    priority: u8,
    dp: u8,
    pf: u8,
    ps: u8,
    sa: u8,
}

fn hex_byte(t: &str) -> Result<u8, String> {
    if t.len() != 2 {
        return Err(format!("\"{t}\" is not a byte"));
    }
    u8::from_str_radix(t, 16).map_err(|_| format!("\"{t}\" is not hex"))
}

/// Reads the header at the start of a line. Returns it and how many tokens it used.
fn header(t: &[&str]) -> Result<(Header, usize), String> {
    // AT JHF1 (the ELM327 default): "6 0FEEE 00" = priority, EDP/DP digit + PF + PS, source.
    if t.len() >= 3 && t[0].len() == 1 && t[1].len() == 5 {
        let priority =
            u8::from_str_radix(t[0], 16).map_err(|_| format!("bad priority \"{}\"", t[0]))?;
        let id =
            u32::from_str_radix(t[1], 16).map_err(|_| format!("bad identifier \"{}\"", t[1]))?;
        return Ok((
            Header {
                priority,
                dp: ((id >> 16) & 1) as u8,
                pf: ((id >> 8) & 0xFF) as u8,
                ps: (id & 0xFF) as u8,
                sa: hex_byte(t[2])?,
            },
            3,
        ));
    }
    // AT JHF0: "18 FE EE 00" = 3 priority bits + EDP + DP, PF, PS, source.
    if t.len() >= 4 {
        let b0 = hex_byte(t[0])?;
        return Ok((
            Header {
                priority: (b0 >> 2) & 0x7,
                dp: b0 & 1,
                pf: hex_byte(t[1])?,
                ps: hex_byte(t[2])?,
                sa: hex_byte(t[3])?,
            },
            4,
        ));
    }
    Err("line too short for a J1939 header".into())
}

/// Parses ELM327 J1939 output (headers on). Multi-segment answers (transport
/// protocol, PF 0xEB) are put back together and given `expected_pgn`, the PGN
/// that was requested or monitored, since their header does not carry it.
pub fn parse_elm(lines: &[String], expected_pgn: u32) -> Result<Parsed, String> {
    let mut out = Parsed::default();
    // Per source: announced total (if a count line came first) and bytes so far.
    let mut multi: BTreeMap<u8, (Option<usize>, Vec<u8>, u8)> = BTreeMap::new();
    let mut announced: Option<usize> = None;
    for line in lines {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        if t.len() == 1 && t[0].len() == 3 {
            // Byte count of the multi-segment message that follows.
            announced = Some(
                usize::from_str_radix(t[0], 16)
                    .map_err(|_| format!("bad byte count \"{}\"", t[0]))?,
            );
            continue;
        }
        let (h, used) = header(&t)?;
        let data = t[used..]
            .iter()
            .map(|x| hex_byte(x))
            .collect::<Result<Vec<u8>, String>>()?;
        match h.pf {
            0xEB => {
                // TP.DT: sequence number, then 7 bytes.
                let Some((&seq, rest)) = data.split_first() else {
                    return Err("empty transport segment".into());
                };
                let e = multi
                    .entry(h.sa)
                    .or_insert_with(|| (announced.take(), Vec::new(), 0));
                if seq != e.2.wrapping_add(1) {
                    return Err(format!("segment {seq} out of order from address {}", h.sa));
                }
                e.2 = seq;
                e.1.extend_from_slice(rest);
                if let Some(total) = e.0 {
                    if e.1.len() >= total {
                        let (_, mut bytes, _) = multi.remove(&h.sa).unwrap();
                        bytes.truncate(total);
                        out.messages.push(J1939Message {
                            priority: h.priority,
                            pgn: expected_pgn,
                            source: h.sa,
                            dest: None,
                            data: bytes,
                        });
                    }
                }
            }
            0xEC => {} // TP.CM: the ELM327 handles the handshake.
            0xE8 => {
                if data.len() >= 8 {
                    let pgn = data[5] as u32 | (data[6] as u32) << 8 | (data[7] as u32) << 16;
                    if data[0] == 1 {
                        out.nacks.push(Nack { source: h.sa, pgn });
                    }
                }
            }
            pf => {
                let pdu2 = pf >= 0xF0;
                let pgn =
                    (h.dp as u32) << 16 | (pf as u32) << 8 | if pdu2 { h.ps as u32 } else { 0 };
                out.messages.push(J1939Message {
                    priority: h.priority,
                    pgn,
                    source: h.sa,
                    dest: (!pdu2).then_some(h.ps),
                    data,
                });
            }
        }
    }
    for (sa, (total, bytes, _)) in multi {
        match total {
            Some(n) => {
                return Err(format!(
                    "a multi-segment answer from address {sa} was cut off ({} of {n} bytes)",
                    bytes.len()
                ))
            }
            // Broadcast (no count line seen): keep what came; trailing FF is padding.
            None => out.messages.push(J1939Message {
                priority: 7,
                pgn: expected_pgn,
                source: sa,
                dest: None,
                data: bytes,
            }),
        }
    }
    Ok(out)
}
