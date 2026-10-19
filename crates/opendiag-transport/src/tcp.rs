use opendiag_core::{ByteLink, LinkError, LinkInfo, LinkKind};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub const DEFAULT_WIFI_ADDRESS: &str = "192.168.0.10:35000";

pub struct TcpLink {
    stream: TcpStream,
    name: String,
    kind: LinkKind,
}

impl TcpLink {
    pub fn connect(address: &str, kind: LinkKind, timeout: Duration) -> Result<Self, LinkError> {
        let addr = address
            .to_socket_addrs()
            .map_err(|e| LinkError::Open(format!("{address}: {e}")))?
            .next()
            .ok_or_else(|| LinkError::Open(format!("{address}: no address")))?;
        let stream = TcpStream::connect_timeout(&addr, timeout).map_err(|e| {
            LinkError::Open(format!(
                "{address}: {e}. Is the phone/computer connected to the adapter's Wi-Fi network?"
            ))
        })?;
        stream.set_nodelay(true).ok();
        Ok(Self { stream, name: address.to_string(), kind })
    }
}

impl ByteLink for TcpLink {
    fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError> {
        self.stream.write_all(data).map_err(io_err)
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, LinkError> {
        self.stream
            .set_read_timeout(Some(timeout.max(Duration::from_millis(1))))
            .map_err(io_err)?;
        match self.stream.read(buf) {
            Ok(0) => Err(LinkError::Disconnected),
            Ok(n) => Ok(n),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Ok(0),
            Err(e) => Err(io_err(e)),
        }
    }

    fn info(&self) -> LinkInfo {
        LinkInfo { kind: self.kind, name: self.name.clone(), simulated: false }
    }

    fn close(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

fn io_err(e: std::io::Error) -> LinkError {
    match e.kind() {
        ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted | ErrorKind::BrokenPipe | ErrorKind::NotConnected => {
            LinkError::Disconnected
        }
        ErrorKind::PermissionDenied => LinkError::Permission(e.to_string()),
        _ => LinkError::Io(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn talks_to_a_tcp_adapter() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 16];
            let n = s.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"ATI\r");
            s.write_all(b"ELM327 v1.4b\r\r>").unwrap();
        });
        let mut link = TcpLink::connect(&addr, LinkKind::Wifi, Duration::from_secs(1)).unwrap();
        link.write_all(b"ATI\r").unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 64];
        while !got.contains(&b'>') {
            let n = link.read(&mut buf, Duration::from_millis(500)).unwrap();
            got.extend_from_slice(&buf[..n]);
        }
        assert_eq!(got, b"ELM327 v1.4b\r\r>");
        server.join().unwrap();
        // The adapter closed the socket.
        assert_eq!(link.read(&mut buf, Duration::from_millis(200)), Err(LinkError::Disconnected));
        assert!(!link.info().simulated);
    }

    #[test]
    fn explains_a_refused_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        drop(listener);
        match TcpLink::connect(&addr, LinkKind::Wifi, Duration::from_millis(300)) {
            Err(LinkError::Open(m)) => assert!(m.contains("Wi-Fi")),
            other => panic!("{:?}", other.map(|_| ())),
        }
    }
}
