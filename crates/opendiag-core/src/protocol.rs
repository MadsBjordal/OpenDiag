use serde::{Deserialize, Serialize};

/// Vehicle-side OBD protocols, numbered as the ELM327 `ATSP`/`ATDPN` commands number them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObdProtocol {
    J1850Pwm,
    J1850Vpw,
    Iso9141_2,
    Iso14230Slow,
    Iso14230Fast,
    Can11_500,
    Can29_500,
    Can11_250,
    Can29_250,
}

impl ObdProtocol {
    pub const ALL: [ObdProtocol; 9] = [
        Self::J1850Pwm,
        Self::J1850Vpw,
        Self::Iso9141_2,
        Self::Iso14230Slow,
        Self::Iso14230Fast,
        Self::Can11_500,
        Self::Can29_500,
        Self::Can11_250,
        Self::Can29_250,
    ];

    /// ELM327 protocol number (1–9).
    pub fn elm_number(self) -> u8 {
        match self {
            Self::J1850Pwm => 1,
            Self::J1850Vpw => 2,
            Self::Iso9141_2 => 3,
            Self::Iso14230Slow => 4,
            Self::Iso14230Fast => 5,
            Self::Can11_500 => 6,
            Self::Can29_500 => 7,
            Self::Can11_250 => 8,
            Self::Can29_250 => 9,
        }
    }

    pub fn from_elm_number(n: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.elm_number() == n)
    }

    pub fn is_can(self) -> bool {
        matches!(
            self,
            Self::Can11_500 | Self::Can29_500 | Self::Can11_250 | Self::Can29_250
        )
    }

    pub fn is_29bit(self) -> bool {
        matches!(self, Self::Can29_500 | Self::Can29_250)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::J1850Pwm => "SAE J1850 PWM (41.6 kbaud)",
            Self::J1850Vpw => "SAE J1850 VPW (10.4 kbaud)",
            Self::Iso9141_2 => "ISO 9141-2 (5 baud init)",
            Self::Iso14230Slow => "ISO 14230-4 KWP (5 baud init)",
            Self::Iso14230Fast => "ISO 14230-4 KWP (fast init)",
            Self::Can11_500 => "ISO 15765-4 CAN (11 bit, 500 kbaud)",
            Self::Can29_500 => "ISO 15765-4 CAN (29 bit, 500 kbaud)",
            Self::Can11_250 => "ISO 15765-4 CAN (11 bit, 250 kbaud)",
            Self::Can29_250 => "ISO 15765-4 CAN (29 bit, 250 kbaud)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn elm_numbers_round_trip() {
        for p in ObdProtocol::ALL {
            assert_eq!(ObdProtocol::from_elm_number(p.elm_number()), Some(p));
        }
        assert_eq!(ObdProtocol::from_elm_number(0), None);
        assert!(ObdProtocol::Can29_250.is_29bit());
        assert!(!ObdProtocol::Iso9141_2.is_can());
    }
}
