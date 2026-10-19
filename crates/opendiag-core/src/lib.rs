//! Shared types used by every OpenDiag crate.
//!
//! Nothing in here talks to hardware. Drivers receive a [`ByteLink`] (the raw
//! byte pipe to an adapter: USB serial, Bluetooth serial, TCP, Android USB host,
//! or the simulator) and build protocol logic on top of it.

pub mod capability;
pub mod link;
pub mod protocol;
pub mod status;

pub use capability::{AdapterCapabilities, AdapterFamily, Tri};
pub use link::{ByteLink, LinkError, LinkInfo, LinkKind};
pub use protocol::ObdProtocol;
pub use status::SupportStatus;

/// The application name. Change it here (and in `branding.json`) to rename the project.
pub const APP_NAME: &str = "OpenDiag";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Formats bytes as upper-case hex separated by spaces, e.g. `41 0C 1A F8`.
pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parses hex text (spaces optional) into bytes. Returns `None` on any non-hex input.
pub fn parse_hex(text: &str) -> Option<Vec<u8>> {
    let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.is_empty() || !clean.len().is_multiple_of(2) {
        return None;
    }
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(hex(&[0x41, 0x0c, 0x1a]), "41 0C 1A");
        assert_eq!(parse_hex("41 0c1A"), Some(vec![0x41, 0x0c, 0x1a]));
        assert_eq!(parse_hex("4"), None);
        assert_eq!(parse_hex("zz"), None);
        assert_eq!(parse_hex(""), None);
    }
}
