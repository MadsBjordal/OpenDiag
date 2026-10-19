//! What OpenDiag can do, with an honest status per function and platform.
//! Update this list (and docs/compatibility.md) when something is verified.

use opendiag_core::SupportStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub risk_level: u8,
    pub windows: SupportStatus,
    pub android: SupportStatus,
    pub linux: SupportStatus,
    pub note: &'static str,
}

use SupportStatus::*;

pub const FUNCTIONS: &[FunctionInfo] = &[
    FunctionInfo { id: "connect_usb_serial", name: "Connect: USB / USB-C serial adapter", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "Android uses the USB Host bridge (FTDI, CH34x, CP210x, PL2303, CDC-ACM)." },
    FunctionInfo { id: "connect_bluetooth_classic", name: "Connect: Bluetooth Classic adapter", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: PartiallyImplemented, note: "Windows: pair first, then pick its COM port. Linux: bind with rfcomm first." },
    FunctionInfo { id: "connect_ble", name: "Connect: Bluetooth Low Energy adapter", risk_level: 0, windows: Planned, android: Planned, linux: Planned, note: "Needs each adapter family's documented GATT service; planned for Phase 2." },
    FunctionInfo { id: "connect_wifi", name: "Connect: Wi-Fi / Ethernet (TCP) adapter", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "Automatic discovery is planned; enter the address (often 192.168.0.10:35000)." },
    FunctionInfo { id: "connect_j2534", name: "Connect: SAE J2534 PassThru", risk_level: 0, windows: Planned, android: Unsupported, linux: Unsupported, note: "J2534 drivers are Windows DLLs." },
    FunctionInfo { id: "adapter_probe", name: "Identify adapter and its capabilities", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "ELM327 and STN (OBDLink) families." },
    FunctionInfo { id: "read_vehicle_info", name: "Read VIN, calibration IDs, ECU names", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "OBD service 09." },
    FunctionInfo { id: "read_dtcs", name: "Read stored, pending and permanent fault codes", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "OBD services 03, 07, 0A." },
    FunctionInfo { id: "read_freeze_frame", name: "Read freeze frame", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "OBD service 02, frame 0." },
    FunctionInfo { id: "read_readiness", name: "Read MIL status and readiness monitors", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "OBD service 01 PID 01." },
    FunctionInfo { id: "live_data", name: "Live data (standard PIDs)", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "One PID per request; rate limited." },
    FunctionInfo { id: "clear_emission_dtcs", name: "Clear emissions-related fault codes", risk_level: 1, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "OBD service 04, after reading and saving, with explicit confirmation and engine off." },
    FunctionInfo { id: "reports", name: "Save, reopen, compare and export reports (PDF, JSON, CSV, HTML)", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "Fully local." },
    FunctionInfo { id: "guided_diagnostics", name: "Guided troubleshooting", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "Three general procedures so far." },
    FunctionInfo { id: "compat_database", name: "Compatibility database (local, submissions, moderation, snapshots)", risk_level: 0, windows: ImplementedNotHardwareVerified, android: ImplementedNotHardwareVerified, linux: ImplementedNotHardwareVerified, note: "Online sync with a community server is planned." },
    FunctionInfo { id: "uds_read", name: "Manufacturer modules (ABS, airbag, body…) via UDS, read only", risk_level: 0, windows: Planned, android: Planned, linux: Planned, note: "Phase 3." },
    FunctionInfo { id: "configuration", name: "Vehicle configuration / personalization", risk_level: 2, windows: Planned, android: Planned, linux: Planned, note: "Separate subsystem, off by default. No verified operations exist yet, so nothing can run." },
    FunctionInfo { id: "programming", name: "ECU programming, security access, immobiliser", risk_level: 4, windows: Unsupported, android: Unsupported, linux: Unsupported, note: "Not available in OpenDiag." },
];

pub fn status_for(f: &FunctionInfo, platform: &str) -> SupportStatus {
    match platform {
        "windows" => f.windows,
        "android" => f.android,
        _ => f.linux,
    }
}
