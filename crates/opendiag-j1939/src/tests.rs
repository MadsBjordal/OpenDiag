use super::*;

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn datasheet_examples_parse_in_both_header_formats() {
    // ELM327 datasheet, "Using J1939".
    let a = parse_elm(&lines(&["6 0FECA 00 00 FF 00 00 00 00 FF FF"]), PGN_DM1).unwrap();
    let b = parse_elm(&lines(&["18 FE CA 00 00 FF 00 00 00 00 FF FF"]), PGN_DM1).unwrap();
    assert_eq!(a, b);
    let m = &a.messages[0];
    assert_eq!((m.priority, m.pgn, m.source, m.dest), (6, 0xFECA, 0, None));
    // "no active trouble codes"
    let dm = decode_dm(&m.data).unwrap();
    assert!(dm.dtcs.is_empty());
    assert_eq!(dm.lamps.mil, LampState::Off);

    // Coolant 0x8C = 140 - 40 = 100 °C.
    let c = parse_elm(&lines(&["6 0FEEE 00 8C FF FF FF FF FF FF FF"]), 0xFEEE).unwrap();
    let v = decode(&c.messages[0]);
    assert_eq!(v[0].key, "coolant_temp");
    assert_eq!(v[0].value, Some(100.0));

    // Engine hours 80 84 1E 00 = 2,000,000 x 0.05 h.
    let h = parse_elm(&lines(&["6 0FEE5 00 80 84 1E 00 FF FF FF FF"]), 0xFEE5).unwrap();
    assert_eq!(decode(&h.messages[0])[0].value, Some(100_000.0));
}

#[test]
fn negative_acknowledgement() {
    // Datasheet: engine refuses a request for 00FEEE.
    let p = parse_elm(&lines(&["6 0E8FF 00 01 FF FF FF FF EE FE 00"]), 0xFEEE).unwrap();
    assert!(p.messages.is_empty());
    assert_eq!(
        p.nacks,
        vec![Nack {
            source: 0,
            pgn: 0xFEEE
        }]
    );
}

#[test]
fn multi_segment_dm2_from_the_datasheet() {
    let p = parse_elm(
        &lines(&[
            "012",
            "7 0EBF9 00 01 04 FF 50 00 04 0B 54",
            "7 0EBF9 00 02 00 00 01 5F 05 02 31",
            "7 0EBF9 00 03 6D 05 03 03 FF FF FF",
        ]),
        PGN_DM2,
    )
    .unwrap();
    assert_eq!(p.messages.len(), 1);
    let m = &p.messages[0];
    assert_eq!(m.pgn, PGN_DM2);
    assert_eq!(m.data.len(), 18);
    let dm = decode_dm(&m.data).unwrap();
    assert_eq!(dm.lamps.amber_warning, LampState::On);
    // 50 00 04 0B: SPN 0x50 = 80, FMI 4, 11 occurrences.
    assert_eq!(
        dm.dtcs[0],
        J1939Dtc {
            spn: 80,
            fmi: 4,
            occurrences: 11,
            old_conversion: false
        }
    );
    assert_eq!(dm.dtcs.len(), 4);
}

#[test]
fn cut_off_multi_segment_is_an_error() {
    let r = parse_elm(
        &lines(&["012", "7 0EBF9 00 01 04 FF 50 00 04 0B 54"]),
        PGN_DM2,
    );
    assert!(r.unwrap_err().contains("cut off"));
}

#[test]
fn spn_uses_the_top_three_bits_of_byte_three() {
    // SPN 520192 (0x7F000) = bytes 00 F0 + top bits 111 in byte 3, FMI 31.
    let dm = decode_dm(&[0x40, 0xFF, 0x00, 0xF0, 0xFF, 0x81]).unwrap();
    assert_eq!(dm.dtcs[0].spn, 0x7F000);
    assert_eq!(dm.dtcs[0].fmi, 31);
    assert!(dm.dtcs[0].old_conversion);
    assert_eq!(dm.lamps.mil, LampState::On);
}

#[test]
fn not_available_and_error_values() {
    let m = J1939Message {
        priority: 6,
        pgn: 0xF004,
        source: 0,
        dest: None,
        data: vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    };
    assert_eq!(decode(&m)[0].status.as_deref(), Some("not available"));
    let m = J1939Message {
        data: vec![0xFF, 0xFF, 0xFF, 0x00, 0xFE, 0xFF, 0xFF, 0xFF],
        ..m
    };
    assert_eq!(decode(&m)[0].status.as_deref(), Some("error"));
    let m = J1939Message {
        data: vec![0xFF, 0xFF, 0xFF, 0x20, 0x1C, 0xFF, 0xFF, 0xFF],
        ..m
    };
    assert_eq!(decode(&m)[0].value, Some(900.0));
}

#[test]
fn vin_and_garbage() {
    assert_eq!(
        decode_vin(b"SMLTDTRK000000005*").as_deref(),
        Some("SMLTDTRK000000005")
    );
    assert_eq!(decode_vin(&[0xFF; 8]), None);
    assert!(parse_elm(&lines(&["6 0FEEE 00 8G"]), 0xFEEE).is_err());
}
