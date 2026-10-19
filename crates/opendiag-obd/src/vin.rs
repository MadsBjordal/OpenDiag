//! VIN (vehicle identification number) reading and decoding.
//!
//! A VIN tells you the manufacturer and usually the model year. It does **not**
//! prove which ECU hardware or software is fitted; that has to be read from the ECUs.

use crate::EcuMessage;
use serde::{Deserialize, Serialize};

/// Joins the text of a service 09 answer (VIN = PID 02, calibration IDs = 04,
/// ECU name = 0A) from one ECU. Works for CAN (one message with a count byte)
/// and the older protocols (several numbered messages).
pub fn info_text(messages: &[&EcuMessage], pid: u8, is_can: bool) -> Option<String> {
    let mut parts: Vec<(u8, Vec<u8>)> = Vec::new();
    for m in messages {
        let d = &m.data;
        if d.len() < 3 || d[0] != 0x49 || d[1] != pid {
            continue;
        }
        // CAN: 49 PID count data…  Legacy: 49 PID seq data(4)
        let seq = if is_can { 0 } else { d[2] };
        parts.push((seq, d[3..].to_vec()));
    }
    if parts.is_empty() {
        return None;
    }
    parts.sort_by_key(|p| p.0);
    let bytes: Vec<u8> = parts.into_iter().flat_map(|p| p.1).collect();
    let text: String = bytes
        .into_iter()
        .filter(|b| b.is_ascii_graphic() || *b == b' ')
        .map(|b| b as char)
        .collect();
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Extracts a VIN from joined service 09 PID 02 text (the last 17 alphanumerics).
pub fn vin_from_text(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if chars.len() < 17 {
        return None;
    }
    Some(
        chars[chars.len() - 17..]
            .iter()
            .collect::<String>()
            .to_ascii_uppercase(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VinInfo {
    pub vin: String,
    pub valid_format: bool,
    /// `Some(true/false)` when position 9 can be checked. The check digit is
    /// mandatory in North America but not in Europe, so `false` is only a warning.
    pub check_digit_ok: Option<bool>,
    pub wmi: String,
    pub manufacturer: Option<String>,
    /// Possible model years (position 10 repeats every 30 years).
    pub model_years: Vec<u16>,
    pub warnings: Vec<String>,
}

fn transliterate(c: char) -> Option<u32> {
    Some(match c {
        '0'..='9' => c.to_digit(10)?,
        'A' | 'J' => 1,
        'B' | 'K' | 'S' => 2,
        'C' | 'L' | 'T' => 3,
        'D' | 'M' | 'U' => 4,
        'E' | 'N' | 'V' => 5,
        'F' | 'W' => 6,
        'G' | 'P' | 'X' => 7,
        'H' | 'Y' => 8,
        'R' | 'Z' => 9,
        _ => return None,
    })
}

pub fn check_digit(vin: &str) -> Option<char> {
    const W: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];
    if vin.len() != 17 {
        return None;
    }
    let mut sum = 0;
    for (i, c) in vin.chars().enumerate() {
        sum += transliterate(c)? * W[i];
    }
    let r = sum % 11;
    Some(if r == 10 {
        'X'
    } else {
        char::from_digit(r, 10)?
    })
}

fn model_years(code: char) -> Vec<u16> {
    const CODES: &str = "ABCDEFGHJKLMNPRSTVWXY123456789";
    match CODES.find(code) {
        Some(i) => vec![1980 + i as u16, 2010 + i as u16],
        None => Vec::new(),
    }
}

/// World manufacturer identifiers (first 3 VIN characters) for common makes.
/// From public WMI listings; a missing entry only means "not in our list".
const WMI: &[(&str, &str)] = &[
    ("WVW", "Volkswagen"),
    ("WV1", "Volkswagen Commercial Vehicles"),
    ("WV2", "Volkswagen Commercial Vehicles"),
    ("WAU", "Audi"),
    ("WBA", "BMW"),
    ("WBS", "BMW M"),
    ("WB1", "BMW Motorrad"),
    ("WDB", "Mercedes-Benz"),
    ("WDD", "Mercedes-Benz"),
    ("WP0", "Porsche"),
    ("W0L", "Opel / Vauxhall"),
    ("WF0", "Ford (Germany)"),
    ("VF1", "Renault"),
    ("VF3", "Peugeot"),
    ("VF7", "Citroën"),
    ("VSS", "SEAT"),
    ("TMB", "Škoda"),
    ("ZFA", "Fiat"),
    ("ZAR", "Alfa Romeo"),
    ("ZDM", "Ducati"),
    ("YV1", "Volvo Cars"),
    ("SAL", "Land Rover"),
    ("SAJ", "Jaguar"),
    ("SMT", "Triumph Motorcycles"),
    ("VBK", "KTM"),
    ("JHM", "Honda"),
    ("JH2", "Honda (motorcycles)"),
    ("JKA", "Kawasaki (motorcycles)"),
    ("JYA", "Yamaha (motorcycles)"),
    ("JS1", "Suzuki (motorcycles)"),
    ("JTD", "Toyota"),
    ("JTE", "Toyota"),
    ("JN1", "Nissan"),
    ("JM1", "Mazda"),
    ("KMH", "Hyundai"),
    ("KNA", "Kia"),
    ("1FA", "Ford (USA)"),
    ("1FT", "Ford (USA, trucks)"),
    ("1G1", "Chevrolet (USA)"),
    ("1HG", "Honda (USA)"),
    ("5YJ", "Tesla"),
];

pub fn decode(vin: &str) -> VinInfo {
    let vin = vin.trim().to_ascii_uppercase();
    let mut warnings = Vec::new();
    let valid_format = vin.len() == 17
        && vin
            .chars()
            .all(|c| c.is_ascii_alphanumeric() && !matches!(c, 'I' | 'O' | 'Q'));
    if !valid_format {
        warnings.push(
            "This is not a valid 17-character VIN (letters I, O and Q are never used).".into(),
        );
    }
    let check_digit_ok = if valid_format {
        check_digit(&vin).map(|c| vin.chars().nth(8) == Some(c))
    } else {
        None
    };
    if check_digit_ok == Some(false) {
        warnings.push(
            "The check digit (9th character) does not match. That is normal for many European vehicles, \
             but could also mean the VIN was read or typed incorrectly."
                .into(),
        );
    }
    let wmi: String = vin.chars().take(3).collect();
    let manufacturer = WMI
        .iter()
        .find(|(w, _)| *w == wmi)
        .map(|(_, m)| m.to_string());
    let model_years = if valid_format {
        model_years(vin.chars().nth(9).unwrap_or('0'))
    } else {
        Vec::new()
    };
    VinInfo {
        vin,
        valid_format,
        check_digit_ok,
        wmi,
        manufacturer,
        model_years,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_digit_known_vin() {
        // Widely used example VIN with a correct check digit.
        assert_eq!(check_digit("1M8GDM9AXKP042788"), Some('X'));
        let i = decode("1M8GDM9AXKP042788");
        assert!(i.valid_format);
        assert_eq!(i.check_digit_ok, Some(true));
        assert_eq!(i.model_years, vec![1989, 2019]);
    }

    #[test]
    fn bad_vins() {
        assert!(!decode("ABC").valid_format);
        assert!(!decode("1M8GDM9AXKP04278O").valid_format);
        let eu = decode("WVWZZZ1KZAW000001");
        assert!(eu.valid_format);
        assert_eq!(eu.manufacturer.as_deref(), Some("Volkswagen"));
        assert_eq!(eu.check_digit_ok, Some(false));
        assert_eq!(eu.warnings.len(), 1);
    }

    #[test]
    fn joins_can_and_legacy_vin() {
        let mut can = vec![0x49, 0x02, 0x01];
        can.extend_from_slice(b"1M8GDM9AXKP042788");
        let m = EcuMessage {
            ecu: "7E8".into(),
            data: can,
        };
        let t = info_text(&[&m], 0x02, true).unwrap();
        assert_eq!(vin_from_text(&t).unwrap(), "1M8GDM9AXKP042788");

        let v = b"\0\0\x001M8GDM9AXKP042788";
        let msgs: Vec<EcuMessage> = v
            .chunks(4)
            .enumerate()
            .map(|(i, c)| {
                let mut d = vec![0x49, 0x02, i as u8 + 1];
                d.extend_from_slice(c);
                EcuMessage {
                    ecu: "10".into(),
                    data: d,
                }
            })
            .rev() // out of order on purpose
            .collect();
        let refs: Vec<&EcuMessage> = msgs.iter().collect();
        let t = info_text(&refs, 0x02, false).unwrap();
        assert_eq!(vin_from_text(&t).unwrap(), "1M8GDM9AXKP042788");
        assert_eq!(vin_from_text("SHORT"), None);
    }
}
