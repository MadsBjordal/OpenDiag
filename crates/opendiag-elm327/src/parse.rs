//! Turns ELM327 answer lines into per-ECU data.

use crate::ElmError;
use opendiag_core::ObdProtocol;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLine {
    pub ecu: String,
    /// With headers on CAN: the raw CAN data bytes (ISO-TP PCI first).
    /// Otherwise: the message data.
    pub data: Vec<u8>,
}

fn bytes_of(hex: &str, line: &str) -> Result<Vec<u8>, ElmError> {
    opendiag_core::parse_hex(hex).ok_or_else(|| ElmError::Garbled(format!("\"{line}\"")))
}

/// Parses answer lines (errors already checked) for the given protocol and header setting.
pub fn parse_lines(
    lines: &[String],
    protocol: Option<ObdProtocol>,
    headers: bool,
) -> Result<Vec<ParsedLine>, ElmError> {
    let protocol = protocol.ok_or_else(|| ElmError::Garbled("protocol unknown".into()))?;
    if !headers {
        return parse_headerless(lines, protocol);
    }
    let mut out = Vec::new();
    for line in lines {
        let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.is_empty() {
            continue;
        }
        if protocol.is_can() {
            let hdr_len = if protocol.is_29bit() { 8 } else { 3 };
            if compact.len() <= hdr_len || !compact.is_char_boundary(hdr_len) {
                return Err(ElmError::Garbled(format!("\"{line}\"")));
            }
            let (hdr, rest) = compact.split_at(hdr_len);
            if !hdr.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ElmError::Garbled(format!("\"{line}\"")));
            }
            out.push(ParsedLine {
                ecu: hdr.to_ascii_uppercase(),
                data: bytes_of(rest, line)?,
            });
        } else {
            // J1850 / ISO 9141 / KWP: 3 header bytes (priority/format, target, source), data, checksum.
            let b = bytes_of(&compact, line)?;
            if b.len() < 5 {
                return Err(ElmError::Garbled(format!("\"{line}\"")));
            }
            out.push(ParsedLine {
                ecu: format!("{:02X}", b[2]),
                data: b[3..b.len() - 1].to_vec(),
            });
        }
    }
    Ok(out)
}

/// Headers off: the adapter has removed headers, checksums and (on CAN) the ISO-TP bytes.
/// Multi-frame CAN answers come as a length line ("014") and numbered lines ("0: …").
fn parse_headerless(lines: &[String], protocol: ObdProtocol) -> Result<Vec<ParsedLine>, ElmError> {
    let mut out = Vec::new();
    let mut multi: Option<(usize, Vec<u8>)> = None;
    for line in lines {
        let t = line.trim();
        if protocol.is_can()
            && t.len() == 3
            && t.chars().all(|c| c.is_ascii_hexdigit())
            && !t.contains(' ')
        {
            let len = usize::from_str_radix(t, 16).unwrap_or(0);
            multi = Some((len, Vec::new()));
            continue;
        }
        if let Some((idx, rest)) = t.split_once(':') {
            if idx.trim().len() == 1 {
                let (_, buf) = multi
                    .as_mut()
                    .ok_or_else(|| ElmError::Garbled(format!("\"{line}\"")))?;
                let compact: String = rest.chars().filter(|c| !c.is_whitespace()).collect();
                buf.extend(bytes_of(&compact, line)?);
                continue;
            }
        }
        let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        out.push(ParsedLine {
            ecu: "?".into(),
            data: bytes_of(&compact, line)?,
        });
    }
    if let Some((len, mut buf)) = multi {
        if buf.len() < len {
            return Err(ElmError::Garbled("a multi-frame answer was cut off".into()));
        }
        buf.truncate(len);
        out.push(ParsedLine {
            ecu: "?".into(),
            data: buf,
        });
    }
    Ok(out)
}
