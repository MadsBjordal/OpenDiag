//! Byte links to real adapters.
//!
//! * [`tcp::TcpLink`]: Wi-Fi and Ethernet adapters (ELM327 Wi-Fi adapters usually
//!   listen on 192.168.0.10:35000; check your adapter's manual).
//! * [`serial::SerialLink`] (feature `serial`): USB adapters and, on Windows,
//!   paired Bluetooth Classic adapters, which Windows exposes as COM ports.
//!
//! Android's USB host and Bluetooth APIs are bridged by the app (see
//! `apps/desktop/src-tauri/src/android.rs`); they implement the same
//! [`opendiag_core::ByteLink`] trait.

pub mod tcp;

#[cfg(feature = "serial")]
pub mod serial;

use opendiag_core::LinkKind;
use serde::{Deserialize, Serialize};

/// A device the user can pick in the connection wizard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCandidate {
    pub id: String,
    pub kind: LinkKind,
    pub name: String,
    pub detail: String,
    /// How the app opens it: "serial:COM5", "tcp:192.168.0.10:35000", "sim:sim-car", "android:…".
    pub address: String,
    pub simulated: bool,
}

/// USB vendor ids of common USB-to-serial chips used in OBD adapters.
pub fn usb_chip_vendor(vid: u16) -> Option<&'static str> {
    match vid {
        0x0403 => Some("FTDI"),
        0x1A86 => Some("WCH (CH340/CH341)"),
        0x10C4 => Some("Silicon Labs (CP210x)"),
        0x067B => Some("Prolific (PL2303)"),
        _ => None,
    }
}

/// Lists serial ports (USB and Bluetooth). Empty when the `serial` feature is off.
pub fn discover_serial() -> Vec<DeviceCandidate> {
    #[cfg(feature = "serial")]
    {
        serial::discover()
    }
    #[cfg(not(feature = "serial"))]
    {
        Vec::new()
    }
}
