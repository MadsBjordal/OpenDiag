//! Driver tests against a scripted adapter (answers as documented in the ELM327 data sheet).

use super::*;
use opendiag_core::{LinkError, LinkInfo, LinkKind};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A fake serial line: each command gets the next scripted answer.
struct Scripted {
    script: Arc<Mutex<VecDeque<(String, String)>>>,
    sent: Arc<Mutex<Vec<String>>>,
    pending: Vec<u8>,
    line: Vec<u8>,
}

impl Scripted {
    fn new(script: &[(&str, &str)]) -> (Self, Arc<Mutex<Vec<String>>>) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                script: Arc::new(Mutex::new(
                    script
                        .iter()
                        .map(|(a, b)| (a.to_string(), b.to_string()))
                        .collect(),
                )),
                sent: sent.clone(),
                pending: Vec::new(),
                line: Vec::new(),
            },
            sent,
        )
    }
}

impl ByteLink for Scripted {
    fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError> {
        for &b in data {
            if b == b'\r' {
                let cmd = String::from_utf8(std::mem::take(&mut self.line)).unwrap();
                if cmd.is_empty() {
                    self.pending.extend_from_slice(b">");
                    continue;
                }
                self.sent.lock().unwrap().push(cmd.clone());
                let mut s = self.script.lock().unwrap();
                match s.front() {
                    Some((want, _)) if *want == cmd || want == "*" => {
                        let (_, answer) = s.pop_front().unwrap();
                        if answer != "<silence>" {
                            self.pending.extend_from_slice(answer.as_bytes());
                            self.pending.extend_from_slice(b"\r\r>");
                        }
                    }
                    _ => self.pending.extend_from_slice(b"?\r\r>"),
                }
            } else {
                self.line.push(b);
            }
        }
        Ok(())
    }
    fn read(&mut self, buf: &mut [u8], _t: Duration) -> Result<usize, LinkError> {
        let n = self.pending.len().min(buf.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        if n == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(n)
    }
    fn info(&self) -> LinkInfo {
        LinkInfo {
            kind: LinkKind::UsbSerial,
            name: "scripted".into(),
            simulated: false,
        }
    }
}

fn init_script() -> Vec<(&'static str, &'static str)> {
    vec![
        ("ATZ", "\r\rELM327 v1.4b"),
        ("ATE0", "ATE0\rOK"),
        ("ATL0", "OK"),
        ("ATS0", "OK"),
        ("ATH1", "OK"),
        ("AT@1", "OBDII to RS232 Interpreter"),
        ("STI", "?"),
        ("ATRV", "12.6V"),
    ]
}

fn fast() -> Timeouts {
    Timeouts {
        reset: Duration::from_millis(300),
        command: Duration::from_millis(300),
        request: Duration::from_millis(300),
        search: Duration::from_millis(300),
    }
}

fn open(extra: &[(&'static str, &'static str)]) -> (Elm327, Arc<Mutex<Vec<String>>>) {
    let mut s = init_script();
    s.extend_from_slice(extra);
    let (link, sent) = Scripted::new(&s);
    (Elm327::open_with(Box::new(link), fast()).unwrap(), sent)
}

#[test]
fn probes_a_genuine_style_adapter() {
    let (elm, sent) = open(&[]);
    let c = elm.capabilities();
    assert_eq!(c.identity, "ELM327 v1.4b");
    assert_eq!(c.headers, Tri::Yes);
    assert_eq!(c.voltage_reading, Tri::Yes);
    assert!(!c.clone_suspected);
    assert_eq!(c.description.as_deref(), Some("OBDII to RS232 Interpreter"));
    // Only documented adapter commands were sent while probing.
    for cmd in sent.lock().unwrap().iter() {
        assert!(ADAPTER_COMMANDS.contains(&cmd.as_str()), "sent {cmd}");
    }
}

#[test]
fn flags_a_clone_without_headers() {
    let (link, _) = Scripted::new(&[
        ("ATZ", "ELM327 v1.5"),
        ("ATE0", "OK"),
        ("ATL0", "OK"),
        ("ATS0", "?"),
        ("ATH1", "?"),
        ("AT@1", "?"),
        ("STI", "?"),
        ("ATRV", "?"),
    ]);
    let elm = Elm327::open_with(Box::new(link), fast()).unwrap();
    let c = elm.capabilities();
    assert!(c.clone_suspected);
    assert_eq!(c.headers, Tri::No);
    assert_eq!(c.spaces_off, Tri::No);
    assert_eq!(c.voltage_reading, Tri::No);
    assert_eq!(c.notes.len(), 2);
}

#[test]
fn rejects_a_device_that_is_not_an_elm() {
    let (link, _) = Scripted::new(&[("ATZ", "HELLO"), ("ATI", "HELLO")]);
    assert!(matches!(
        Elm327::open_with(Box::new(link), fast()),
        Err(ElmError::NotElm(_))
    ));
}

#[test]
fn accepts_compatible_adapters_with_their_own_name() {
    // An adapter that never says "ELM" but obeys the ELM327 commands.
    let (link, _) = Scripted::new(&[
        ("ATZ", "ACME Diag 2.1"),
        ("ATI", "ACME Diag 2.1"),
        ("ATE0", "OK"),
        ("ATE0", "OK"),
        ("*", "OK"),
        ("*", "OK"),
        ("*", "OK"),
        ("*", "?"),
        ("*", "?"),
        ("*", "12.6V"),
    ]);
    let elm = Elm327::open_with(Box::new(link), fast()).unwrap();
    assert_eq!(elm.capabilities().identity, "ACME Diag 2.1");
    assert!(elm
        .capabilities()
        .notes
        .iter()
        .any(|n| n.contains("ELM327-compatible")));
}

#[test]
fn accepts_an_adapter_that_does_not_identify_itself() {
    // Like the Vgate vLinker FS USB: ATZ and ATI answer "?", the rest works.
    let (link, _) = Scripted::new(&[
        ("ATZ", "?"),
        ("ATI", "?"),
        ("ATE0", "OK"),
        ("ATE0", "OK"),
        ("*", "OK"),
        ("*", "OK"),
        ("*", "OK"),
        ("*", "?"),
        ("*", "?"),
        ("*", "12.6V"),
    ]);
    let elm = Elm327::open_with(Box::new(link), fast()).unwrap();
    assert!(elm
        .capabilities()
        .identity
        .contains("did not identify itself"));
}

#[test]
fn connects_on_can_and_reads_two_ecus() {
    let (mut elm, _) = open(&[
        ("ATSP0", "OK"),
        (
            "0100",
            "SEARCHING...\r7E8 06 41 00 BE 3F A8 13 00\r7E9 06 41 00 98 18 80 11 00",
        ),
        ("ATDPN", "A6"),
        ("010C", "7E8 04 41 0C 1A F8 00 00 00"),
    ]);
    let (p, msgs) = elm.connect_vehicle(None).unwrap();
    assert_eq!(p, ObdProtocol::Can11_500);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].ecu, "7E8");
    assert_eq!(msgs[0].data, vec![0x41, 0x00, 0xBE, 0x3F, 0xA8, 0x13]);
    assert_eq!(msgs[1].ecu, "7E9");
    let r = elm.request(&[0x01, 0x0C], None).unwrap();
    assert_eq!(r[0].data, vec![0x41, 0x0C, 0x1A, 0xF8]);
}

#[test]
fn reassembles_a_multi_frame_vin_without_spaces() {
    let (mut elm, _) = open(&[
        ("ATSP0", "OK"),
        ("0100", "7E8064100BE3FA81300"),
        ("ATDPN", "A6"),
        (
            "0902",
            "7E810144902013147\r7E82131484D3832\r7E82236333341303034\r7E8233335320000",
        ),
    ]);
    elm.connect_vehicle(None).unwrap();
    let r = elm.request(&[0x09, 0x02], None).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(&r[0].data[..3], &[0x49, 0x02, 0x01]);
    assert_eq!(&r[0].data[3..], b"1G1HM82633A004352");
    assert_eq!(elm.capabilities().multi_frame, Tri::Yes);
    // The VIN is not written to the log.
    assert!(elm
        .log()
        .iter()
        .any(|l| l.text.contains("VIN answer hidden")));
}

#[test]
fn legacy_iso9141_headers_and_checksum() {
    let (mut elm, _) = open(&[
        ("ATSP0", "OK"),
        (
            "0100",
            "SEARCHING...\rBUS INIT: ...OK\r48 6B 10 41 00 BE 1F B8 10 AA",
        ),
        ("ATDPN", "A3"),
        ("03", "48 6B 10 43 01 33 00 00 00 00 D4"),
    ]);
    let (p, msgs) = elm.connect_vehicle(None).unwrap();
    assert_eq!(p, ObdProtocol::Iso9141_2);
    assert_eq!(msgs[0].ecu, "10");
    assert_eq!(msgs[0].data, vec![0x41, 0x00, 0xBE, 0x1F, 0xB8, 0x10]);
    let d = elm.request(&[0x03], None).unwrap();
    assert_eq!(d[0].data, vec![0x43, 0x01, 0x33, 0, 0, 0, 0]);
}

#[test]
fn errors_are_reported() {
    let (mut elm, _) = open(&[("ATSP0", "OK"), ("0100", "SEARCHING...\rUNABLE TO CONNECT")]);
    assert_eq!(
        elm.connect_vehicle(None).unwrap_err(),
        ElmError::UnableToConnect
    );

    let (mut elm, _) = open(&[
        ("ATSP0", "OK"),
        ("0100", "7E8 06 41 00 BE 3F A8 13 00"),
        ("ATDPN", "A6"),
        ("0151", "NO DATA"),
        ("0105", "CAN ERROR"),
        ("0104", "7E8 ZZ"),
        ("010C", "<silence>"),
    ]);
    elm.connect_vehicle(None).unwrap();
    assert_eq!(
        elm.request(&[0x01, 0x51], None).unwrap_err(),
        ElmError::NoData
    );
    assert!(matches!(
        elm.request(&[0x01, 0x05], None).unwrap_err(),
        ElmError::Bus(_)
    ));
    assert!(matches!(
        elm.request(&[0x01, 0x04], None).unwrap_err(),
        ElmError::Garbled(_)
    ));
    assert!(matches!(
        elm.request(&[0x01, 0x0C], None).unwrap_err(),
        ElmError::Timeout(_)
    ));
    assert_eq!(elm.consecutive_errors, 3);
}

#[test]
fn write_requests_need_authorization() {
    let (mut elm, sent) = open(&[
        ("ATSP0", "OK"),
        ("0100", "7E8 06 41 00 BE 3F A8 13 00"),
        ("ATDPN", "A6"),
    ]);
    elm.connect_vehicle(None).unwrap();
    let before = sent.lock().unwrap().len();
    assert!(matches!(
        elm.request(&[0x04], None),
        Err(ElmError::Blocked(_))
    ));
    assert!(matches!(
        elm.request(&[0x2E, 0xF1, 0x90], None),
        Err(ElmError::Blocked(_))
    ));
    assert!(matches!(
        elm.request(&[0x27, 0x01], None),
        Err(ElmError::Blocked(_))
    ));
    assert_eq!(
        sent.lock().unwrap().len(),
        before,
        "nothing reached the adapter"
    );
    assert!(matches!(
        elm.adapter_command("AT@3 HELLO"),
        Err(ElmError::Blocked(_))
    ));
    assert!(matches!(
        elm.adapter_command("ATPP 0C SV 23"),
        Err(ElmError::Blocked(_))
    ));
}

#[test]
fn headerless_multi_frame() {
    let lines: Vec<String> = [
        "014",
        "0: 49 02 01 31 47 31",
        "1: 4A 43 35 34 34 34 52",
        "2: 37 32 35 32 33 36 37",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let p = parse_lines(&lines, Some(ObdProtocol::Can11_500), false).unwrap();
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].data.len(), 20);
    assert_eq!(&p[0].data[3..], b"1G1JC5444R7252367");
}

#[test]
fn twenty_nine_bit_headers() {
    let lines = vec!["18 DA F1 10 06 41 00 BE 3F A8 13 00".to_string()];
    let p = parse_lines(&lines, Some(ObdProtocol::Can29_500), true).unwrap();
    assert_eq!(p[0].ecu, "18DAF110");
    assert_eq!(
        p[0].data,
        vec![0x06, 0x41, 0x00, 0xBE, 0x3F, 0xA8, 0x13, 0x00]
    );
}

#[test]
fn j1939_truck_through_the_real_driver() {
    use opendiag_sim::{profiles, SimAdapter, SimSettings};
    let (sim, _ctl) = SimAdapter::new(profiles::truck(), SimSettings::default());
    let mut elm = Elm327::open(Box::new(sim)).unwrap();
    // A J1939-only truck does not answer generic OBD-II.
    assert!(matches!(
        elm.connect_vehicle(None),
        Err(ElmError::UnableToConnect)
    ));
    elm.j1939_start().unwrap();
    let w = Duration::from_millis(100);
    let dm1 = elm.j1939_monitor(opendiag_j1939::PGN_DM1, w).unwrap();
    assert_eq!(dm1.messages.len(), 3);
    let engine = dm1.messages.iter().find(|m| m.source == 0).unwrap();
    let dm = opendiag_j1939::decode_dm(&engine.data).unwrap();
    assert_eq!(dm.dtcs[0].spn, 110);
    assert_eq!(dm.lamps.amber_warning, opendiag_j1939::LampState::On);
    // Broadcast engine speed.
    let eec1 = elm.j1939_monitor(0xF004, w).unwrap();
    assert!(!eec1.messages.is_empty());
    assert!(opendiag_j1939::decode(&eec1.messages[0])[0].value.unwrap() > 500.0);
    // Multi-segment VIN on request, hidden in the log.
    let vi = elm.j1939_request(opendiag_j1939::PGN_VI).unwrap();
    assert_eq!(
        opendiag_j1939::decode_vin(&vi.messages[0].data).as_deref(),
        Some("SMLTDTRK000000005")
    );
    assert!(!elm.log().iter().any(|l| l.text.contains("53 4D 4C")));
    // Nothing on the bus for a PGN nobody has; clearing groups are blocked.
    assert!(elm.j1939_request(0xFEE0).is_ok());
    assert!(matches!(
        elm.j1939_request(0xFED3),
        Err(ElmError::Blocked(_))
    ));
    // The adapter is usable again afterwards.
    assert!(elm.read_voltage().is_some());
}
