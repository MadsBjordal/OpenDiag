use serde::{Deserialize, Serialize};
use std::time::Duration;

/// How the computer or phone reaches the adapter. This is the physical/OS
/// channel only: the adapter's own protocol (ELM327, STN, J2534…) is separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// USB or USB-C adapter that shows up as a serial (COM / tty) port.
    UsbSerial,
    /// Bluetooth Classic serial port profile (on Windows: a paired "COM" port).
    BluetoothClassic,
    /// Bluetooth Low Energy.
    BluetoothLe,
    /// Wi-Fi adapter reached over TCP.
    Wifi,
    /// Wired Ethernet / TCP.
    Ethernet,
    /// USB device opened through Android's USB Host API.
    AndroidUsb,
    /// OpenDiag's built-in simulator. Never a real vehicle.
    Simulated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkInfo {
    pub kind: LinkKind,
    /// Human-readable name, e.g. "COM5", "OBDLink MX+", "192.168.0.10:35000".
    pub name: String,
    pub simulated: bool,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkError {
    #[error("the adapter disconnected")]
    Disconnected,
    #[error("permission denied: {0}")]
    Permission(String),
    #[error("could not open the connection: {0}")]
    Open(String),
    #[error("input/output error: {0}")]
    Io(String),
}

/// A raw, bidirectional byte pipe to a diagnostic adapter.
///
/// Implementations: serial ports, TCP sockets, the Android bridge and the simulator.
pub trait ByteLink: Send {
    fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError>;
    /// Reads whatever is available, waiting at most `timeout`.
    /// Returns `Ok(0)` when nothing arrived in time.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, LinkError>;
    fn info(&self) -> LinkInfo;
    fn close(&mut self) {}
}
