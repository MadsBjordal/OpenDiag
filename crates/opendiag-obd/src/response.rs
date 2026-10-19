use serde::{Deserialize, Serialize};

/// A diagnostic negative response (`7F <service> <code>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegativeResponse {
    pub service: u8,
    pub code: u8,
}

impl NegativeResponse {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() >= 3 && data[0] == 0x7F {
            Some(Self { service: data[1], code: data[2] })
        } else {
            None
        }
    }

    /// ISO 14229-1 negative response code names (shared with OBD on CAN).
    pub fn meaning(&self) -> &'static str {
        nrc_name(self.code)
    }

    /// 0x78 "response pending": the ECU will answer later.
    pub fn is_pending(&self) -> bool {
        self.code == 0x78
    }
}

pub fn nrc_name(code: u8) -> &'static str {
    match code {
        0x10 => "General reject",
        0x11 => "Service not supported",
        0x12 => "Sub-function not supported",
        0x13 => "Incorrect message length or invalid format",
        0x14 => "Response too long",
        0x21 => "Busy, repeat request",
        0x22 => "Conditions not correct",
        0x24 => "Request sequence error",
        0x31 => "Request out of range",
        0x33 => "Security access denied",
        0x35 => "Invalid key",
        0x36 => "Exceeded number of attempts",
        0x37 => "Required time delay not expired",
        0x72 => "General programming failure",
        0x78 => "Request received, response pending",
        0x7E => "Sub-function not supported in active session",
        0x7F => "Service not supported in active session",
        _ => "Unknown negative response code",
    }
}

/// True when `data` is the positive answer to `service` (service + 0x40).
pub fn is_positive(service: u8, data: &[u8]) -> bool {
    data.first() == Some(&(service.wrapping_add(0x40)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_response() {
        let n = NegativeResponse::parse(&[0x7F, 0x04, 0x22]).unwrap();
        assert_eq!(n.meaning(), "Conditions not correct");
        assert!(!n.is_pending());
        assert!(NegativeResponse::parse(&[0x44]).is_none());
        assert!(is_positive(0x04, &[0x44]));
        assert!(!is_positive(0x03, &[0x44]));
    }
}
