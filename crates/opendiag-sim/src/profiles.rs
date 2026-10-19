//! The demonstration vehicles. All are invented and labelled SIMULATED.

use opendiag_core::ObdProtocol;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleKind {
    Car,
    Motorcycle,
    Van,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Petrol,
    Diesel,
    MotorcyclePetrol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimEcu {
    /// CAN id the ECU answers from (e.g. 0x7E8), or the legacy source address (e.g. 0x10).
    pub address: u32,
    pub name: String,
    pub pids: Vec<u8>,
    pub stored: Vec<String>,
    pub pending: Vec<String>,
    pub permanent: Vec<String>,
    /// Code that caused the freeze frame.
    pub freeze_dtc: Option<String>,
    pub vin: Option<String>,
    pub calid: Option<String>,
    pub ecu_name: Option<String>,
    /// Readiness: monitors available (byte C of PID 01) and incomplete (byte D).
    pub monitors_available: u8,
    pub monitors_incomplete: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: VehicleKind,
    pub engine: Engine,
    pub protocol: ObdProtocol,
    pub year: u16,
    pub ecus: Vec<SimEcu>,
}

const PETROL_PIDS: &[u8] = &[
    0x01, 0x03, 0x04, 0x05, 0x06, 0x07, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x13, 0x14, 0x15,
    0x1C, 0x1F, 0x21, 0x2F, 0x31, 0x33, 0x42, 0x46, 0x49, 0x51, 0x5C,
];

pub fn all() -> Vec<VehicleProfile> {
    vec![car(), motorcycle(), multi_ecu(), legacy_car()]
}

pub fn by_id(id: &str) -> Option<VehicleProfile> {
    all().into_iter().find(|p| p.id == id)
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

pub fn car() -> VehicleProfile {
    VehicleProfile {
        id: "sim-car".into(),
        name: "SIMULATED generic OBD-II petrol car".into(),
        description: "Modern passenger car on CAN (11 bit, 500 kbaud). Lean mixture and a cylinder 1 misfire stored, MIL on.".into(),
        kind: VehicleKind::Car,
        engine: Engine::Petrol,
        protocol: ObdProtocol::Can11_500,
        year: 2016,
        ecus: vec![SimEcu {
            address: 0x7E8,
            name: "Engine".into(),
            pids: PETROL_PIDS.to_vec(),
            stored: s(&["P0171", "P0301"]),
            pending: s(&["P0300"]),
            permanent: s(&["P0301"]),
            freeze_dtc: Some("P0301".into()),
            vin: Some("SMLTD0CAR00000001".into()),
            calid: Some("SIMCAL-CAR-0001".into()),
            ecu_name: Some("ECM-EngineControl".into()),
            monitors_available: 0xE5,
            monitors_incomplete: 0x04,
        }],
    }
}

pub fn motorcycle() -> VehicleProfile {
    VehicleProfile {
        id: "sim-motorcycle".into(),
        name: "SIMULATED generic CAN motorcycle".into(),
        description: "Fuel-injected motorcycle on CAN (11 bit, 500 kbaud) through an adapter cable. Low system voltage stored.".into(),
        kind: VehicleKind::Motorcycle,
        engine: Engine::MotorcyclePetrol,
        protocol: ObdProtocol::Can11_500,
        year: 2021,
        ecus: vec![SimEcu {
            address: 0x7E8,
            name: "Engine".into(),
            pids: vec![0x01, 0x04, 0x05, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x11, 0x1C, 0x42, 0x51],
            stored: s(&["P0562"]),
            pending: vec![],
            permanent: vec![],
            freeze_dtc: Some("P0562".into()),
            vin: Some("SMLTDMC0000000002".into()),
            calid: Some("SIMCAL-MC-0002".into()),
            ecu_name: Some("ECM-Motorcycle".into()),
            monitors_available: 0x21,
            monitors_incomplete: 0x00,
        }],
    }
}

pub fn multi_ecu() -> VehicleProfile {
    VehicleProfile {
        id: "sim-multi-ecu".into(),
        name: "SIMULATED diesel van with several ECUs".into(),
        description: "Diesel van on CAN (11 bit, 500 kbaud) with engine, transmission and a third emissions-related ECU.".into(),
        kind: VehicleKind::Van,
        engine: Engine::Diesel,
        protocol: ObdProtocol::Can11_500,
        year: 2019,
        ecus: vec![
            SimEcu {
                address: 0x7E8,
                name: "Engine (diesel)".into(),
                pids: vec![0x01, 0x04, 0x05, 0x0B, 0x0C, 0x0D, 0x0F, 0x10, 0x11, 0x1C, 0x1F, 0x23, 0x2F, 0x31, 0x33, 0x42, 0x46, 0x51, 0x5E],
                stored: s(&["P2002", "P0401"]),
                pending: vec![],
                permanent: s(&["P2002"]),
                freeze_dtc: Some("P2002".into()),
                vin: Some("SMLTDVAN000000003".into()),
                calid: Some("SIMCAL-VAN-ENG".into()),
                ecu_name: Some("ECM-DieselEngine".into()),
                monitors_available: 0xC9,
                monitors_incomplete: 0x40,
            },
            SimEcu {
                address: 0x7E9,
                name: "Transmission".into(),
                pids: vec![0x01, 0x0D, 0x1C],
                stored: vec![],
                pending: s(&["P0730"]),
                permanent: vec![],
                freeze_dtc: None,
                vin: None,
                calid: Some("SIMCAL-VAN-TCM".into()),
                ecu_name: Some("TCM-Transmission".into()),
                monitors_available: 0x00,
                monitors_incomplete: 0x00,
            },
            SimEcu {
                address: 0x7EA,
                name: "Aftertreatment".into(),
                pids: vec![0x01, 0x1C, 0x46],
                stored: vec![],
                pending: vec![],
                permanent: vec![],
                freeze_dtc: None,
                vin: None,
                calid: Some("SIMCAL-VAN-ATC".into()),
                ecu_name: Some("ACM-Aftertreatment".into()),
                monitors_available: 0x00,
                monitors_incomplete: 0x00,
            },
        ],
    }
}

pub fn legacy_car() -> VehicleProfile {
    VehicleProfile {
        id: "sim-legacy".into(),
        name: "SIMULATED older petrol car (ISO 9141-2)".into(),
        description: "Early-2000s car on the K-line (ISO 9141-2). Catalyst efficiency code stored."
            .into(),
        kind: VehicleKind::Car,
        engine: Engine::Petrol,
        protocol: ObdProtocol::Iso9141_2,
        year: 2003,
        ecus: vec![SimEcu {
            address: 0x10,
            name: "Engine".into(),
            pids: vec![
                0x01, 0x03, 0x04, 0x05, 0x06, 0x07, 0x0C, 0x0D, 0x0E, 0x0F, 0x11, 0x13, 0x14, 0x15,
                0x1C,
            ],
            stored: s(&["P0420"]),
            pending: vec![],
            permanent: vec![],
            freeze_dtc: Some("P0420".into()),
            vin: Some("SMLTDK91410000044".into()),
            calid: None,
            ecu_name: None,
            monitors_available: 0x65,
            monitors_incomplete: 0x00,
        }],
    }
}
