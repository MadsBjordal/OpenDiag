//! End-to-end tests: the real driver and workflows against the simulator.

use super::*;
use crate::report::{DtcKind, ModuleStatus, Verdict};
use opendiag_sim::{profiles, SimAdapter, SimSettings};

fn engine_with(profile: profiles::VehicleProfile, settings: SimSettings) -> (Engine, SimControl) {
    let mut e = Engine::new(Store::open_in_memory().unwrap(), "linux");
    let (link, ctl) = SimAdapter::new(profile, settings);
    e.connect(Box::new(link), Some(ctl.clone())).unwrap();
    (e, ctl)
}

fn engine(profile: profiles::VehicleProfile) -> (Engine, SimControl) {
    engine_with(
        profile,
        SimSettings {
            latency_ms: 0,
            ..Default::default()
        },
    )
}

fn no_progress() -> impl FnMut(ScanProgress) {
    |_| {}
}

#[test]
fn connect_reports_simulated_status() {
    let (mut e, _) = engine(profiles::car());
    let s = e.summary().unwrap();
    assert!(s.simulated);
    assert!(s.vehicle_connected);
    assert_eq!(s.protocol, Some(ObdProtocol::Can11_500));
    assert_eq!(s.ecus.len(), 1);
    assert!(s.ecus[0].supported_pids > 20);
    assert_eq!(s.adapter_voltage, Some(14.1));
    let dtc = s
        .compatibility
        .functions
        .iter()
        .find(|f| f.id == "read_dtcs")
        .unwrap();
    assert_eq!(dtc.status, SupportStatus::SimulatedOnly);
    assert!(s.compatibility.summary.contains("Simulated"));
    let prog = s
        .compatibility
        .functions
        .iter()
        .find(|f| f.id == "programming")
        .unwrap();
    assert_eq!(prog.status, SupportStatus::Unsupported);
}

#[test]
fn full_scan_of_the_simulated_car() {
    let (mut e, _) = engine(profiles::car());
    let mut steps = Vec::new();
    let r = e.scan(&mut |p| steps.push(p.step)).unwrap();
    assert_eq!(steps, (1..=8).collect::<Vec<u8>>());
    let codes: Vec<(String, DtcKind)> = r.dtcs.iter().map(|d| (d.code.clone(), d.kind)).collect();
    assert!(codes.contains(&("P0171".into(), DtcKind::Stored)));
    assert!(codes.contains(&("P0301".into(), DtcKind::Stored)));
    assert!(codes.contains(&("P0300".into(), DtcKind::Pending)));
    assert!(codes.contains(&("P0301".into(), DtcKind::Permanent)));
    assert!(r.dtcs.iter().all(|d| d.explanation.known));
    assert_eq!(r.vehicle.vin.as_ref().unwrap().vin, "SMLTD0CAR00000001");
    assert_eq!(r.vehicle.calibration_ids[0].1, "SIMCAL-CAR-0001");
    assert_eq!(r.vehicle.obd_standard.as_deref(), Some("EOBD (Europe)"));
    assert_eq!(r.vehicle.fuel_type.as_deref(), Some("Gasoline"));
    assert_eq!(r.health.verdict, Verdict::FaultsFound);
    assert!(
        r.communication_errors.is_empty(),
        "{:?}",
        r.communication_errors
    );
    // Never claims systems it did not reach.
    assert!(
        r.modules
            .iter()
            .filter(|m| m.status == ModuleStatus::NotScanned)
            .count()
            >= 10
    );
    let ff = &r.freeze_frames[0];
    assert_eq!(ff.dtc.as_deref(), Some("P0301"));
    assert!(ff.values.iter().any(|v| v.key == "rpm" && v.value > 0.0));
    let (_, mon) = &r.readiness[0];
    assert!(mon.mil_on);
    assert_eq!(mon.dtc_count, 2);
    assert!(r
        .measurements
        .iter()
        .any(|m| m.key == "ltft_b1" && (m.value - 13.3).abs() < 1.0));
    assert!(r.simulated);
    // Saved and reopened from the database.
    let id: i64 = r.id.parse().unwrap();
    let again = e.report(id).unwrap();
    assert_eq!(again.dtcs.len(), r.dtcs.len());
    assert_eq!(e.store().vehicles().unwrap().len(), 1);
    // Compatibility results were recorded as simulated.
    let recs = e.store().compat_records(&Default::default()).unwrap();
    assert!(recs
        .iter()
        .all(|r| r.verification == opendiag_store::compat::Verification::Simulated));
    assert!(recs.iter().any(|r| r.function == "read_dtcs"));
}

#[test]
fn exports_mask_the_vin_by_default() {
    let (mut e, _) = engine(profiles::car());
    let id: i64 = e.scan(&mut no_progress()).unwrap().id.parse().unwrap();
    let (name, pdf) = e.export_report(id, "pdf", false).unwrap();
    assert!(name.ends_with("-SIMULATED.pdf"));
    assert!(pdf.starts_with(b"%PDF"));
    let (_, json) = e.export_report(id, "json", false).unwrap();
    let json = String::from_utf8(json).unwrap();
    assert!(!json.contains("SMLTD0CAR00000001"));
    assert!(json.contains("SML**************"));
    let (_, json) = e.export_report(id, "json", true).unwrap();
    assert!(String::from_utf8(json)
        .unwrap()
        .contains("SMLTD0CAR00000001"));
    let (_, csv) = e.export_report(id, "csv", false).unwrap();
    let csv = String::from_utf8(csv).unwrap();
    assert!(csv.starts_with("section,ecu,item"));
    assert!(csv.contains("dtc,7E8,P0171"));
    let (_, html) = e.export_report(id, "html", false).unwrap();
    assert!(String::from_utf8(html)
        .unwrap()
        .contains("SIMULATED VEHICLE"));
    assert!(e.export_report(id, "docx", false).is_err());
}

#[test]
fn clearing_goes_through_every_safety_check() {
    let (mut e, ctl) = engine(profiles::car());
    let before: i64 = e.scan(&mut no_progress()).unwrap().id.parse().unwrap();

    // 1. Without accepting the risk notice.
    let plan = e.prepare_clear().unwrap();
    assert_eq!(plan.codes.len(), 3);
    assert_eq!(plan.permanent.len(), 1);
    assert!(!plan.preview.allowed);
    let r = e.execute_clear(&plan.id, true).unwrap();
    assert!(!r.performed);
    assert!(r
        .decision
        .blockers
        .iter()
        .any(|b| b.contains("risk notice")));

    // 2. Engine running.
    e.accept_terms(true).unwrap();
    let plan = e.prepare_clear().unwrap();
    let r = e.execute_clear(&plan.id, true).unwrap();
    assert!(!r.performed);
    assert!(r
        .decision
        .blockers
        .iter()
        .any(|b| b.contains("engine is running")));

    // 3. Not confirmed.
    ctl.set(SimSettings {
        engine_running: false,
        latency_ms: 0,
        ..Default::default()
    });
    let plan = e.prepare_clear().unwrap();
    assert!(plan.preview.allowed, "{:?}", plan.preview);
    let r = e.execute_clear(&plan.id, false).unwrap();
    assert!(!r.performed);

    // 4. A plan can only be used once.
    let plan = e.prepare_clear().unwrap();
    let r = e.execute_clear(&plan.id, true).unwrap();
    assert!(r.performed, "{:?}", r.decision);
    assert!(r.modules[0].accepted);
    assert!(r.remaining.iter().all(|d| d.kind == DtcKind::Permanent));
    assert!(e.execute_clear(&plan.id, true).is_err());

    // The snapshot holds what was erased.
    let snap = e.store().clear_snapshot(plan.snapshot_id).unwrap().unwrap();
    assert_eq!(snap["codes"].as_array().unwrap().len(), 3);

    // A new scan shows the change; the comparison lists what went away.
    let after_report = e.scan(&mut no_progress()).unwrap();
    assert!(after_report
        .operations
        .iter()
        .any(|o| o.what == "clear_dtcs"));
    let after: i64 = after_report.id.parse().unwrap();
    let cmp = e.compare_reports(before, after).unwrap();
    assert!(cmp.gone.iter().any(|(c, _)| c == "P0171"));
    assert!(cmp
        .still_present
        .iter()
        .any(|(c, k)| c == "P0301" && k == "permanent"));
    assert!(cmp.new_codes.is_empty());
}

#[test]
fn clear_is_blocked_on_low_voltage_and_stale_plans() {
    let mut s = SimSettings {
        engine_running: false,
        latency_ms: 0,
        ..Default::default()
    };
    s.faults.low_voltage = true;
    let (mut e, _) = engine_with(profiles::car(), s);
    e.accept_terms(true).unwrap();
    let plan = e.prepare_clear().unwrap();
    let r = e.execute_clear(&plan.id, true).unwrap();
    assert!(!r.performed);
    assert!(r.decision.blockers.iter().any(|b| b.contains("10.6 V")));
    assert!(e.execute_clear("not-a-plan", true).is_err());
}

#[test]
fn live_data() {
    let (mut e, _) = engine(profiles::car());
    let params = e.live_params().unwrap();
    assert!(params.iter().any(|p| p.key == "rpm"));
    assert!(params
        .iter()
        .any(|p| p.key == "adapter_voltage" && p.source == "adapter"));
    assert!(params
        .iter()
        .any(|p| p.key == "boost_derived" && p.source == "derived"));
    let keys: Vec<String> = [
        "rpm",
        "coolant_temp",
        "fuel_rail_pressure",
        "adapter_voltage",
        "nonsense",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let v = e.live_read(&keys).unwrap();
    assert!(v[0].value.unwrap() > 500.0);
    assert_eq!(v[0].ecu.as_deref(), Some("7E8"));
    assert!(v[1].value.is_some());
    assert_eq!(
        v[2].error.as_deref(),
        Some("Not supported by this vehicle.")
    );
    assert_eq!(v[3].value, Some(14.1));
    assert!(v[4].error.is_some());
    let too_many: Vec<String> = (0..20).map(|_| "rpm".to_string()).collect();
    assert!(e.live_read(&too_many).is_err());
}

#[test]
fn expert_console_is_read_only() {
    let (mut e, _) = engine(profiles::car());
    assert!(matches!(
        e.raw_request("010C"),
        Err(EngineError::Blocked(_))
    ));
    e.set_setting("mode", "expert").unwrap();
    let r = e.raw_request("01 0C").unwrap();
    assert_eq!(r[0].data[0], 0x41);
    for blocked in [
        "04", "14FFFFFF", "2EF19001", "2701", "3101FF00", "1002", "08",
    ] {
        assert!(
            matches!(e.raw_request(blocked), Err(EngineError::Blocked(_))),
            "{blocked}"
        );
    }
    assert!(matches!(e.raw_request("zz"), Err(EngineError::Invalid(_))));
    assert!(e.set_setting("mode", "god").is_err());
    assert!(e.set_setting("anything", "x").is_err());
}

#[test]
fn multi_ecu_van() {
    let (mut e, _) = engine(profiles::multi_ecu());
    let r = e.scan(&mut no_progress()).unwrap();
    assert_eq!(r.modules.iter().filter(|m| m.ecu.is_some()).count(), 3);
    assert!(r
        .dtcs
        .iter()
        .any(|d| d.code == "P0730" && d.ecu == "7E9" && d.kind == DtcKind::Pending));
    assert!(r.dtcs.iter().any(|d| d.code == "P2002" && d.ecu == "7E8"));
    assert_eq!(r.vehicle.calibration_ids.len(), 3);
    assert_eq!(r.vehicle.fuel_type.as_deref(), Some("Diesel"));
    assert!(r.readiness.iter().any(|(_, m)| m.compression_ignition));
}

#[test]
fn legacy_k_line_car() {
    let (mut e, _) = engine(profiles::legacy_car());
    let s = e.summary().unwrap();
    assert_eq!(s.protocol, Some(ObdProtocol::Iso9141_2));
    assert_eq!(s.ecus[0].address, "10");
    let r = e.scan(&mut no_progress()).unwrap();
    assert_eq!(
        r.dtcs.iter().map(|d| d.code.as_str()).collect::<Vec<_>>(),
        vec!["P0420"]
    );
    assert_eq!(r.vehicle.vin.as_ref().unwrap().vin, "SMLTDK91410000044");
    assert!(r.warnings.iter().any(|w| w.contains("Permanent codes")));
}

#[test]
fn motorcycle() {
    let (mut e, _) = engine(profiles::motorcycle());
    let r = e.scan(&mut no_progress()).unwrap();
    assert_eq!(r.dtcs[0].code, "P0562");
    assert!(r.dtcs[0]
        .explanation
        .related_measurements
        .contains(&"module_voltage".to_string()));
    assert!(!r.readiness[0].1.mil_on);
}

#[test]
fn ignition_off_keeps_the_adapter_and_can_retry() {
    let mut s = SimSettings {
        latency_ms: 0,
        ..Default::default()
    };
    s.faults.ignition_off = true;
    let (mut e, ctl) = engine_with(profiles::car(), s);
    let sum = e.summary().unwrap();
    assert!(!sum.vehicle_connected);
    assert!(sum.vehicle_error.unwrap().contains("UNABLE TO CONNECT"));
    assert!(matches!(
        e.scan(&mut no_progress()),
        Err(EngineError::NoVehicle(_))
    ));
    ctl.set(SimSettings {
        latency_ms: 0,
        ..Default::default()
    });
    assert!(e.connect_vehicle().unwrap().vehicle_connected);
    assert!(e.scan(&mut no_progress()).is_ok());
}

#[test]
fn communication_faults_are_reported_not_hidden() {
    let (mut e, ctl) = engine(profiles::car());
    let mut s = ctl.settings();
    s.faults.truncated = true;
    ctl.set(s);
    let r = e.scan(&mut no_progress()).unwrap();
    assert!(
        r.communication_errors.iter().any(|x| x.contains("cut off")),
        "{:?}",
        r.communication_errors
    );
    assert!(r.vehicle.vin.is_none());

    let (mut e, ctl) = engine(profiles::car());
    let mut s = ctl.settings();
    s.faults.malformed = true;
    ctl.set(s);
    let r = e.scan(&mut no_progress());
    match r {
        Ok(r) => assert!(!r.communication_errors.is_empty()),
        Err(err) => assert!(err.to_string().contains("understood"), "{err}"),
    }

    let (mut e, ctl) = engine(profiles::car());
    let mut s = ctl.settings();
    s.faults.disconnect_after = Some(3);
    ctl.set(s);
    let r = e.scan(&mut no_progress());
    let ok_with_errors = matches!(&r, Ok(r) if !r.communication_errors.is_empty());
    assert!(r.is_err() || ok_with_errors);
}

#[test]
fn bad_vin_and_clone_adapter_warnings() {
    let mut s = SimSettings {
        latency_ms: 0,
        ..Default::default()
    };
    s.faults.wrong_vin = true;
    s.faults.clone_adapter = true;
    let (mut e, _) = engine_with(profiles::car(), s);
    let sum = e.summary().unwrap();
    assert!(sum.adapter.clone_suspected);
    assert_eq!(sum.adapter_voltage, None);
    let r = e.scan(&mut no_progress()).unwrap();
    assert!(
        r.warnings
            .iter()
            .any(|w| w.contains("not a valid 17-character VIN")),
        "{:?}",
        r.warnings
    );
    assert!(r
        .warnings
        .iter()
        .any(|w| w.contains("cannot show which module")));
    // Headers off: the ECU is unknown but codes are still read.
    assert!(r.dtcs.iter().any(|d| d.code == "P0171" && d.ecu == "?"));
    // An invalid VIN is not saved to the garage.
    assert!(e.store().vehicles().unwrap().is_empty());
}

#[test]
fn guided_and_settings() {
    let (e, _) = engine(profiles::motorcycle());
    let s = e.guided("heated-grips-not-working", &[]).unwrap();
    assert_eq!(s.current.unwrap().id, "confirm");
    assert!(e.guided("nope", &[]).is_err());
    assert_eq!(e.settings()["mode"], "beginner");
    assert_eq!(e.settings()["configuration_enabled"], false);
}

#[test]
fn devices_list_includes_simulators() {
    let d = devices();
    assert!(d.iter().filter(|x| x.simulated).count() >= 3);
    assert!(d.iter().any(|x| x.address.starts_with("tcp:")));
}

#[test]
fn scan_and_clear_over_a_raw_k_line() {
    use opendiag_sim::kline::{SimKLineMode, SimKLinePort};
    let mut e = Engine::new(Store::open_in_memory().unwrap(), "linux");
    let (port, ctl) = SimKLinePort::new(
        profiles::legacy_car(),
        SimSettings {
            latency_ms: 0,
            ..Default::default()
        },
        SimKLineMode::KwpFast,
    );
    let drv =
        opendiag_kline::KLineObd::open(Box::new(port), opendiag_kline::InitMode::FastInit).unwrap();
    let s = e.connect_driver(Box::new(drv), Some(ctl.clone())).unwrap();
    assert!(s.vehicle_connected, "{:?}", s.vehicle_error);
    assert_eq!(s.protocol, Some(ObdProtocol::Iso14230Fast));
    assert_eq!(s.adapter.family, opendiag_core::AdapterFamily::Simulated);
    let r = e.scan(&mut no_progress()).unwrap();
    assert!(r.dtcs.iter().any(|d| d.code == "P0420"));
    assert_eq!(r.vehicle.vin.as_ref().unwrap().vin, "SMLTDK91410000044");
    let live = e.live_read(&["rpm".to_string()]).unwrap();
    assert!(live[0].value.unwrap() > 500.0);
    e.accept_terms(true).unwrap();
    ctl.set(SimSettings {
        engine_running: false,
        latency_ms: 0,
        ..Default::default()
    });
    let plan = e.prepare_clear().unwrap();
    let res = e.execute_clear(&plan.id, true).unwrap();
    assert!(res.performed, "{:?}", res.decision);
    assert!(res.remaining.is_empty());
}

#[test]
fn j1939_truck_scan() {
    let (mut e, ctl) = engine(profiles::truck());
    let s = e.summary().unwrap();
    assert!(
        !s.vehicle_connected,
        "a J1939-only truck does not answer OBD-II"
    );
    let r = e.j1939_scan().unwrap();
    assert!(r.simulated);
    assert_eq!(r.modules.len(), 3);
    let engine = r.modules.iter().find(|m| m.address == 0).unwrap();
    assert_eq!(engine.active.len(), 1);
    assert_eq!(engine.active[0].spn, 110);
    assert_eq!(
        engine.active[0].spn_name.as_deref(),
        Some("Engine coolant temperature")
    );
    assert_eq!(engine.previous.as_ref().unwrap()[0].spn, 168);
    assert_eq!(
        engine.lamps.unwrap().amber_warning,
        opendiag_j1939::LampState::On
    );
    let keys: Vec<&str> = r.values.iter().map(|v| v.key.as_str()).collect();
    for k in [
        "engine_speed",
        "coolant_temp",
        "vehicle_speed",
        "battery_voltage",
        "engine_hours",
        "total_distance",
    ] {
        assert!(keys.contains(&k), "{k} missing: {keys:?}");
    }
    let hours = r.values.iter().find(|v| v.key == "engine_hours").unwrap();
    assert!((hours.value.unwrap() - 12_345.6).abs() < 0.1);
    assert_eq!(r.vin.as_deref(), Some("SMLTDTRK000000005"));
    // Recorded as simulated, never as hardware evidence.
    let st = e.compat_status().unwrap();
    assert!(st.functions.iter().any(|f| f.id == "j1939_read_codes"));

    // Ignition off: nothing heard, explained.
    let mut set = ctl.settings();
    set.faults.ignition_off = true;
    ctl.set(set);
    let r = e.j1939_scan().unwrap();
    assert!(r.modules.is_empty());
    assert!(r.notes[0].contains("Nothing was heard"));
}

#[test]
fn j1939_needs_an_elm_adapter_and_cars_stay_quiet() {
    // A car without J1939: the scan explains that nothing was heard.
    let (mut e, _) = engine(profiles::car());
    let r = e.j1939_scan().unwrap();
    assert!(r.modules.is_empty() && r.notes[0].contains("Nothing was heard"));
    // OBD-II works again after reconnecting.
    assert!(e.connect_vehicle().unwrap().vehicle_connected);
}
