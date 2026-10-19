use crate::{usb_chip_vendor, DeviceCandidate};
use opendiag_core::{ByteLink, LinkError, LinkInfo, LinkKind};
use serialport::{SerialPort, SerialPortType};
use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

/// Baud rates tried when the adapter's rate is unknown. 38400 is the ELM327 default;
/// many clones and Bluetooth adapters use 9600 or 115200.
pub const BAUD_RATES: [u32; 5] = [38400, 115200, 9600, 230400, 500000];

pub struct SerialLink {
    port: Box<dyn SerialPort>,
    name: String,
    kind: LinkKind,
}

impl SerialLink {
    pub fn open(path: &str, baud: u32, kind: LinkKind) -> Result<Self, LinkError> {
        let port = serialport::new(path, baud)
            .timeout(Duration::from_millis(50))
            .open()
            .map_err(|e| match e.kind() {
                serialport::ErrorKind::Io(ErrorKind::PermissionDenied) => {
                    LinkError::Permission(format!(
                    "{path}: permission denied. On Linux add yourself to the \"dialout\" group; \
                     on Windows close other programs using the port."
                ))
                }
                serialport::ErrorKind::NoDevice => {
                    LinkError::Open(format!("{path}: the device is gone (unplugged?)"))
                }
                _ => LinkError::Open(format!("{path}: {e}")),
            })?;
        Ok(Self {
            port,
            name: format!("{path} @ {baud}"),
            kind,
        })
    }

    /// Opens the port at the first baud rate where an adapter answers `ATI`
    /// (an adapter-only identification command; nothing reaches the vehicle).
    pub fn open_auto(path: &str, kind: LinkKind) -> Result<(Self, u32), LinkError> {
        let mut last = None;
        for baud in BAUD_RATES {
            match Self::open(path, baud, kind) {
                Ok(mut link) => {
                    if link.answers_ati() {
                        return Ok((link, baud));
                    }
                    last = Some(LinkError::Open(format!(
                        "{path}: no ELM327-compatible answer at any common speed"
                    )));
                }
                Err(e @ LinkError::Permission(_)) => return Err(e),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or_else(|| LinkError::Open(path.into())))
    }

    fn answers_ati(&mut self) -> bool {
        let _ = self.port.clear(serialport::ClearBuffer::All);
        if self.write_all(b"\rATI\r").is_err() {
            return false;
        }
        let deadline = Instant::now() + Duration::from_millis(1200);
        let mut got = Vec::new();
        let mut buf = [0u8; 128];
        while Instant::now() < deadline {
            match self.read(&mut buf, Duration::from_millis(100)) {
                Ok(n) => got.extend_from_slice(&buf[..n]),
                Err(_) => return false,
            }
            let text = String::from_utf8_lossy(&got).to_ascii_uppercase();
            if text.contains("ELM") || text.contains("STN") {
                // Let the rest of the answer arrive, then drop it.
                std::thread::sleep(Duration::from_millis(100));
                let _ = self.port.clear(serialport::ClearBuffer::Input);
                return true;
            }
        }
        false
    }
}

impl ByteLink for SerialLink {
    fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError> {
        self.port.write_all(data).map_err(io_err)?;
        self.port.flush().map_err(io_err)
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, LinkError> {
        self.port
            .set_timeout(timeout.max(Duration::from_millis(1)))
            .map_err(|e| LinkError::Io(e.to_string()))?;
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(io_err(e)),
        }
    }

    fn info(&self) -> LinkInfo {
        LinkInfo {
            kind: self.kind,
            name: self.name.clone(),
            simulated: false,
        }
    }
}

fn io_err(e: std::io::Error) -> LinkError {
    match e.kind() {
        ErrorKind::BrokenPipe | ErrorKind::NotConnected | ErrorKind::UnexpectedEof => {
            LinkError::Disconnected
        }
        ErrorKind::PermissionDenied => LinkError::Permission(e.to_string()),
        _ => LinkError::Io(e.to_string()),
    }
}

pub fn discover() -> Vec<DeviceCandidate> {
    let Ok(ports) = serialport::available_ports() else {
        return Vec::new();
    };
    ports
        .into_iter()
        .map(|p| {
            let (kind, detail) = match &p.port_type {
                SerialPortType::UsbPort(u) => {
                    let chip = usb_chip_vendor(u.vid)
                        .map(|c| format!(" · {c} chip"))
                        .unwrap_or_default();
                    let product = u
                        .product
                        .clone()
                        .unwrap_or_else(|| "USB serial device".into());
                    (
                        LinkKind::UsbSerial,
                        format!("{product} (USB {:04X}:{:04X}){chip}", u.vid, u.pid),
                    )
                }
                SerialPortType::BluetoothPort => {
                    (LinkKind::BluetoothClassic, "Bluetooth serial port".into())
                }
                SerialPortType::PciPort => (LinkKind::UsbSerial, "Built-in serial port".into()),
                SerialPortType::Unknown => (LinkKind::UsbSerial, "Serial port".into()),
            };
            DeviceCandidate {
                id: format!("serial:{}", p.port_name),
                kind,
                name: p.port_name.clone(),
                detail,
                address: format!("serial:{}", p.port_name),
                simulated: false,
            }
        })
        .collect()
}

/// A K-line cable ("KKL", FTDI-type) on a serial port, driven at the line level.
pub struct SerialKLine {
    port: Box<dyn SerialPort>,
    name: String,
}

impl SerialKLine {
    pub fn open(path: &str) -> Result<Self, LinkError> {
        let link = SerialLink::open(path, 10400, LinkKind::KLineCable)?;
        Ok(Self {
            port: link.port,
            name: format!("{path} (K-line)"),
        })
    }
}

impl opendiag_core::KLinePort for SerialKLine {
    fn write(&mut self, data: &[u8]) -> Result<(), LinkError> {
        self.port.write_all(data).map_err(io_err)?;
        self.port.flush().map_err(io_err)
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, LinkError> {
        self.port
            .set_timeout(timeout.max(Duration::from_millis(1)))
            .map_err(|e| LinkError::Io(e.to_string()))?;
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(io_err(e)),
        }
    }

    fn set_break(&mut self, low: bool) -> Result<(), LinkError> {
        let r = if low {
            self.port.set_break()
        } else {
            self.port.clear_break()
        };
        r.map_err(|e| LinkError::Io(format!("this cable cannot control the K-line ({e})")))
    }

    fn set_baud(&mut self, baud: u32) -> Result<(), LinkError> {
        self.port
            .set_baud_rate(baud)
            .map_err(|e| LinkError::Io(e.to_string()))
    }

    fn clear_input(&mut self) -> Result<(), LinkError> {
        self.port
            .clear(serialport::ClearBuffer::Input)
            .map_err(|e| LinkError::Io(e.to_string()))
    }

    fn info(&self) -> LinkInfo {
        LinkInfo {
            kind: LinkKind::KLineCable,
            name: self.name.clone(),
            simulated: false,
        }
    }
}
