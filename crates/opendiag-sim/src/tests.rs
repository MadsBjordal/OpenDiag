use super::*;

fn talk(sim: &mut SimAdapter, cmd: &str) -> String {
    sim.write_all(format!("{cmd}\r").as_bytes()).unwrap();
    let mut out = Vec::new();
    let mut buf = [0u8; 512];
    for _ in 0..50 {
        let n = sim.read(&mut buf, Duration::from_millis(5)).unwrap();
        out.extend_from_slice(&buf[..n]);
        if out.contains(&b'>') {
            break;
        }
    }
    String::from_utf8(out).unwrap()
}

fn sim(profile: VehicleProfile) -> (SimAdapter, SimControl) {
    SimAdapter::new(profile, SimSettings { latency_ms: 0, ..Default::default() })
}

#[test]
fn behaves_like_an_elm_after_reset() {
    let (mut s, _) = sim(profiles::car());
    assert!(talk(&mut s, "ATZ").contains("ELM327 v1.4b"));
    // Echo is on after a reset.
    assert!(talk(&mut s, "ATE0").starts_with("ATE0"));
    assert_eq!(talk(&mut s, "ATL0"), "OK\r\r>");
    assert_eq!(talk(&mut s, "ATXYZ"), "?\r\r>");
    assert_eq!(talk(&mut s, "STI"), "?\r\r>");
    assert!(talk(&mut s, "AT@1").contains("SIMULATED"));
    assert!(s.info().simulated);
}

#[test]
fn searches_then_answers_with_headers() {
    let (mut s, _) = sim(profiles::car());
    for c in ["ATZ", "ATE0", "ATL0", "ATS0", "ATH1", "ATSP0"] {
        talk(&mut s, c);
    }
    let a = talk(&mut s, "0100");
    assert!(a.starts_with("SEARCHING...\r7E806410"), "{a}");
    assert_eq!(talk(&mut s, "ATDPN"), "A6\r\r>");
    let a = talk(&mut s, "03");
    // 43, 2 codes: P0171 (01 71), P0301 (03 01).
    assert!(a.starts_with("7E80643020171030100"), "{a}");
    let vin = talk(&mut s, "0902");
    assert_eq!(vin.matches("7E8").count(), 3, "{vin}");
}

#[test]
fn refuses_to_clear_while_running_then_clears() {
    let (mut s, ctl) = sim(profiles::car());
    for c in ["ATZ", "ATE0", "ATL0", "ATS1", "ATH0", "ATSP0", "0100"] {
        talk(&mut s, c);
    }
    assert!(talk(&mut s, "04").starts_with("7F 04 22"));
    ctl.set(SimSettings { engine_running: false, latency_ms: 0, ..Default::default() });
    assert!(talk(&mut s, "04").starts_with("44"));
    assert!(talk(&mut s, "03").starts_with("43 00"));
    // Permanent codes survive a clear.
    assert!(talk(&mut s, "0A").starts_with("4A 01 03 01"));
    // The freeze frame is gone.
    assert!(talk(&mut s, "020200").contains("NO DATA"));
}

#[test]
fn legacy_k_line_format() {
    let (mut s, _) = sim(profiles::legacy_car());
    for c in ["ATZ", "ATE0", "ATL0", "ATS1", "ATH1", "ATSP0"] {
        talk(&mut s, c);
    }
    let a = talk(&mut s, "0100");
    assert!(a.contains("BUS INIT: ...OK\r48 6B 10 41 00"), "{a}");
    assert_eq!(talk(&mut s, "ATDPN"), "A3\r\r>");
    let vin = talk(&mut s, "0902");
    assert_eq!(vin.matches("48 6B 10 49 02").count(), 5, "{vin}");
}

#[test]
fn multi_ecu_answers() {
    let (mut s, _) = sim(profiles::multi_ecu());
    for c in ["ATZ", "ATE0", "ATL0", "ATS1", "ATH1", "ATSP0", "0100"] {
        talk(&mut s, c);
    }
    let a = talk(&mut s, "0100");
    for ecu in ["7E8", "7E9", "7EA"] {
        assert!(a.contains(ecu), "{a}");
    }
    let p = talk(&mut s, "07");
    assert!(p.contains("7E9 04 47 01 07 30"), "{p}");
}

#[test]
fn faults() {
    let (mut s, ctl) = sim(profiles::car());
    for c in ["ATZ", "ATE0", "ATL0", "ATS1", "ATH1"] {
        talk(&mut s, c);
    }
    let mut set = ctl.settings();
    set.latency_ms = 0;
    set.faults.ignition_off = true;
    ctl.set(set.clone());
    assert!(talk(&mut s, "0100").contains("UNABLE TO CONNECT"));
    set.faults.ignition_off = false;
    set.faults.bus_error = true;
    ctl.set(set.clone());
    assert!(talk(&mut s, "0100").contains("CAN ERROR"));
    set.faults.bus_error = false;
    set.faults.low_voltage = true;
    ctl.set(set.clone());
    assert_eq!(talk(&mut s, "ATRV"), "10.6V\r\r>");
    set.faults.no_response = true;
    ctl.set(set.clone());
    assert!(!talk(&mut s, "010C").contains('>'));
    set.faults.no_response = false;
    set.faults.disconnect_after = Some(0);
    ctl.set(set);
    assert!(s.write_all(b"010C\r").is_ok());
    assert_eq!(s.write_all(b"010C\r"), Err(LinkError::Disconnected));
}

#[test]
fn clone_adapter() {
    let mut set = SimSettings { latency_ms: 0, ..Default::default() };
    set.faults.clone_adapter = true;
    let (mut s, _) = SimAdapter::new(profiles::car(), set);
    assert!(talk(&mut s, "ATZ").contains("v1.5"));
    assert!(talk(&mut s, "ATH1").contains('?'));
    talk(&mut s, "ATE0");
    talk(&mut s, "ATSP0");
    talk(&mut s, "0100");
    let v = talk(&mut s, "0902");
    assert!(v.starts_with("014\r\n0: 49 02 01"), "{v:?}");
}

#[test]
fn profiles_are_valid_and_labelled() {
    for p in profiles::all() {
        assert!(p.name.starts_with("SIMULATED"), "{}", p.name);
        for e in &p.ecus {
            for c in e.stored.iter().chain(&e.pending).chain(&e.permanent) {
                assert!(Dtc(c.clone()).to_bytes().is_some(), "{c}");
            }
            if let Some(v) = &e.vin {
                assert_eq!(v.len(), 17, "{v}");
                assert!(opendiag_obd::vin::decode(v).valid_format, "{v}");
            }
        }
    }
}
