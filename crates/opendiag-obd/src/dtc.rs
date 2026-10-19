use serde::{Deserialize, Serialize};

/// A diagnostic trouble code such as `P0301`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Dtc(pub String);

impl Dtc {
    /// Decodes the two-byte SAE J2012 form. `00 00` means "no code" (padding).
    pub fn from_bytes(a: u8, b: u8) -> Option<Self> {
        if a == 0 && b == 0 {
            return None;
        }
        let letter = ['P', 'C', 'B', 'U'][(a >> 6) as usize];
        let d1 = (a >> 4) & 0x03;
        Some(Dtc(format!("{letter}{d1}{:X}{:02X}", a & 0x0F, b)))
    }

    pub fn to_bytes(&self) -> Option<[u8; 2]> {
        let s = self.0.as_bytes();
        if s.len() != 5 {
            return None;
        }
        let sys = match s[0] {
            b'P' => 0,
            b'C' => 1,
            b'B' => 2,
            b'U' => 3,
            _ => return None,
        };
        let d1 = (s[1] as char).to_digit(4)? as u8;
        let rest = u16::from_str_radix(std::str::from_utf8(&s[2..]).ok()?, 16).ok()?;
        Some([(sys << 6) | (d1 << 4) | ((rest >> 8) as u8 & 0x0F), (rest & 0xFF) as u8])
    }

    pub fn system(&self) -> &'static str {
        match self.0.as_bytes().first() {
            Some(b'P') => "Powertrain",
            Some(b'C') => "Chassis",
            Some(b'B') => "Body",
            Some(b'U') => "Network / communication",
            _ => "Unknown",
        }
    }

    /// Whether the code number is defined by SAE (the same on every vehicle)
    /// or left to the manufacturer (SAE J2012 numbering).
    pub fn ownership(&self) -> CodeOwnership {
        let b = self.0.as_bytes();
        if b.len() != 5 {
            return CodeOwnership::Unknown;
        }
        match (b[0], b[1]) {
            (b'P', b'0') | (b'P', b'2') => CodeOwnership::Generic,
            (b'P', b'1') => CodeOwnership::Manufacturer,
            (b'P', b'3') => {
                // P30xx–P33xx manufacturer, P34xx–P39xx SAE reserved/generic.
                if matches!(b[2], b'0'..=b'3') {
                    CodeOwnership::Manufacturer
                } else {
                    CodeOwnership::Generic
                }
            }
            (_, b'0') => CodeOwnership::Generic,
            (_, b'1') | (_, b'2') => CodeOwnership::Manufacturer,
            _ => CodeOwnership::Reserved,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeOwnership {
    Generic,
    Manufacturer,
    Reserved,
    Unknown,
}

impl std::fmt::Display for Dtc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Decodes the data of a service 03 / 07 / 0A answer (`43 …`, `47 …`, `4A …`).
///
/// On CAN the first byte after the service is the number of codes; on the older
/// protocols there is no count and messages are padded with `00 00`.
pub fn parse_dtc_response(data: &[u8], is_can: bool) -> Vec<Dtc> {
    if data.is_empty() {
        return Vec::new();
    }
    let body = &data[1..];
    let (count, pairs) = if is_can {
        match body.split_first() {
            Some((n, rest)) => (Some(*n as usize), rest),
            None => return Vec::new(),
        }
    } else {
        (None, body)
    };
    let mut out: Vec<Dtc> = pairs
        .chunks_exact(2)
        .filter_map(|p| Dtc::from_bytes(p[0], p[1]))
        .collect();
    if let Some(n) = count {
        out.truncate(n);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_codes() {
        assert_eq!(Dtc::from_bytes(0x03, 0x01).unwrap().0, "P0301");
        assert_eq!(Dtc::from_bytes(0x01, 0x71).unwrap().0, "P0171");
        assert_eq!(Dtc::from_bytes(0x42, 0x0A).unwrap().0, "C020A");
        assert_eq!(Dtc::from_bytes(0x81, 0x23).unwrap().0, "B0123");
        assert_eq!(Dtc::from_bytes(0xC1, 0x00).unwrap().0, "U0100");
        assert_eq!(Dtc::from_bytes(0x24, 0x2F).unwrap().0, "P242F");
        assert_eq!(Dtc::from_bytes(0, 0), None);
    }

    #[test]
    fn encodes_codes() {
        for code in ["P0301", "C020A", "U0100", "P242F", "B3FFF", "P0A80"] {
            let d = Dtc(code.into());
            let [a, b] = d.to_bytes().unwrap();
            assert_eq!(Dtc::from_bytes(a, b).unwrap(), d);
        }
        assert_eq!(Dtc("X0000".into()).to_bytes(), None);
    }

    #[test]
    fn can_response_uses_count() {
        let d = parse_dtc_response(&[0x43, 0x02, 0x03, 0x01, 0x01, 0x71, 0x00, 0x00], true);
        assert_eq!(d, vec![Dtc("P0301".into()), Dtc("P0171".into())]);
        assert!(parse_dtc_response(&[0x43, 0x00], true).is_empty());
        // Count larger than the data present: use what is there.
        assert_eq!(parse_dtc_response(&[0x43, 0x05, 0x03, 0x01], true).len(), 1);
    }

    #[test]
    fn legacy_response_ignores_padding() {
        let d = parse_dtc_response(&[0x43, 0x01, 0x33, 0x00, 0x00, 0x00, 0x00], false);
        assert_eq!(d, vec![Dtc("P0133".into())]);
    }

    #[test]
    fn ownership() {
        assert_eq!(Dtc("P0301".into()).ownership(), CodeOwnership::Generic);
        assert_eq!(Dtc("P1234".into()).ownership(), CodeOwnership::Manufacturer);
        assert_eq!(Dtc("P3000".into()).ownership(), CodeOwnership::Manufacturer);
        assert_eq!(Dtc("P3400".into()).ownership(), CodeOwnership::Generic);
        assert_eq!(Dtc("U0100".into()).ownership(), CodeOwnership::Generic);
        assert_eq!(Dtc("B1000".into()).ownership(), CodeOwnership::Manufacturer);
        assert_eq!(Dtc("C3000".into()).ownership(), CodeOwnership::Reserved);
    }
}
