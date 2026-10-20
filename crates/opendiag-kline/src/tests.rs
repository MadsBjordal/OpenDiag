//! The K-line driver against the simulated K-line vehicle (real timing).

use super::*;
use opendiag_sim::kline::{SimKLineMode, SimKLinePort};
use opendiag_sim::{profiles, SimSettings};

fn sim(mode: SimKLineMode) -> (KLineObd, opendiag_sim::SimControl) {
    let (port, ctl) = SimKLinePort::new(
        profiles::legacy_car(),
        SimSettings {
            latency_ms: 0,
            ..Default::default()
        },
        mode,
    );
    let mut d = KLineObd::open(Box::new(port), InitMode::Auto).unwrap();
    d.timing.idle_before_init = Duration::from_millis(50);
    (d, ctl)
}

#[test]
fn fast_init_and_obd_requests() {
    let (mut d, _) = sim(SimKLineMode::KwpFast);
    let (p, msgs) = d.connect_vehicle(None).unwrap();
    assert_eq!(p, ObdProtocol::Iso14230Fast);
    assert_eq!(msgs[0].ecu, "10");
    assert_eq!(&msgs[0].data[..2], &[0x41, 0x00]);
    let r = d.request(&[0x01, 0x0C], None).unwrap();
    assert_eq!(&r[0].data[..2], &[0x41, 0x0C]);
    // Stored codes (legacy format: no count byte).
    let r = d.request(&[0x03], None).unwrap();
    assert_eq!(r[0].data[0], 0x43);
    assert!(opendiag_obd::dtc::parse_dtc_response(&r[0].data, false)
        .iter()
        .any(|c| c.0 == "P0420"));
}

#[test]
fn five_baud_iso9141() {
    let (mut d, _) = sim(SimKLineMode::Iso9141);
    d.mode = InitMode::FiveBaud;
    let (p, msgs) = d.connect_vehicle(None).unwrap();
    assert_eq!(p, ObdProtocol::Iso9141_2);
    assert_eq!(&msgs[0].data[..2], &[0x41, 0x00]);
    // VIN comes as several numbered messages and is hidden in the log.
    let vin = d.request(&[0x09, 0x02], None).unwrap();
    assert_eq!(vin.len(), 5);
    let refs: Vec<&EcuMessage> = vin.iter().collect();
    let text = opendiag_obd::vin::info_text(&refs, 0x02, false).unwrap();
    assert_eq!(
        opendiag_obd::vin::vin_from_text(&text).unwrap(),
        "SMLTDK91410000044"
    );
    assert!(d.log().iter().any(|l| l.text.contains("VIN answer hidden")));
}

#[test]
fn auto_falls_back_to_five_baud_kwp() {
    let (mut d, _) = sim(SimKLineMode::Kwp5Baud);
    let (p, _) = d.connect_vehicle(None).unwrap();
    assert_eq!(p, ObdProtocol::Iso14230Slow);
    assert!(d.log().iter().any(|l| l.text.contains("Fast init failed")));
    assert!(d.request(&[0x01, 0x05], None).is_ok());
}

#[test]
fn ignition_off_and_blocked_requests() {
    let (mut d, ctl) = sim(SimKLineMode::KwpFast);
    d.mode = InitMode::FastInit;
    let mut s = ctl.settings();
    s.faults.ignition_off = true;
    ctl.set(s);
    assert!(matches!(
        d.connect_vehicle(None),
        Err(DriverError::InitFailed(_))
    ));
    ctl.set(SimSettings {
        latency_ms: 0,
        ..Default::default()
    });
    d.connect_vehicle(None).unwrap();
    assert!(matches!(
        d.request(&[0x04], None),
        Err(DriverError::Blocked(_))
    ));
    assert!(matches!(
        d.request(&[0x2E, 0x01, 0x02], None),
        Err(DriverError::Blocked(_))
    ));
    // Unsupported PID: no answer.
    assert_eq!(
        d.request(&[0x01, 0x5E], None).unwrap_err(),
        DriverError::NoData
    );
}

#[test]
fn cable_without_echo() {
    let (mut port, _) = SimKLinePort::new(
        profiles::legacy_car(),
        SimSettings {
            latency_ms: 0,
            ..Default::default()
        },
        SimKLineMode::KwpFast,
    );
    port.echo = false;
    let mut d = KLineObd::open(Box::new(port), InitMode::FastInit).unwrap();
    d.timing.idle_before_init = Duration::from_millis(50);
    let r = d.connect_vehicle(None);
    assert!(r.is_ok(), "{r:?} {:?}", d.log());
    assert!(d.log().iter().any(|l| l.text.contains("does not echo")));
}

#[test]
fn honda_experimental_mode() {
    use crate::honda::{HondaKLine, Wake};
    let (port, _) = SimKLinePort::new(
        profiles::motorcycle(),
        SimSettings::default(),
        SimKLineMode::Honda,
    );
    let mut h = HondaKLine::open(Box::new(port)).unwrap();
    let w = h.wake(Wake::Ping72).unwrap();
    assert!(w.awake, "{w:?}");
    assert_eq!(w.wake_reply, "0E 04 72 7C");
    let t = h.read_table(0x11).unwrap();
    assert_eq!(t.len(), 16);
    assert!(h.wake(Wake::FfFf).unwrap().awake);
    assert!(h.read_table(0x10).is_ok());
}

#[test]
fn hold_is_accurate_enough_for_fast_init() {
    for _ in 0..5 {
        let t = Instant::now();
        hold(Duration::from_millis(25));
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        assert!((25.0..27.0).contains(&ms), "{ms}");
    }
}
