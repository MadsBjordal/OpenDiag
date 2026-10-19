use crate::{LinkKind, ObdProtocol};
use serde::{Deserialize, Serialize};

/// Yes / no / not known yet. Capabilities start as `Unknown` and are only set
/// after the adapter has actually answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tri {
    Yes,
    No,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterFamily {
    /// Answers like an ELM327 (genuine or clone).
    Elm327,
    /// ScanTool STN chip (OBDLink family): ELM327 compatible plus ST commands.
    Stn,
    Simulated,
    /// A raw K-line cable ("KKL", FTDI-type): OpenDiag drives the protocol itself.
    RawKLine,
    Unknown,
}

/// What a connected adapter has shown it can do. Filled in by the driver's probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterCapabilities {
    pub family: AdapterFamily,
    pub link: LinkKind,
    /// The adapter's own identification (`ATI`), e.g. "ELM327 v1.4b".
    pub identity: String,
    /// `AT@1` device description, when the adapter has one.
    pub description: Option<String>,
    /// `STI` firmware string for STN chips.
    pub stn_firmware: Option<String>,
    /// Protocols this chip family is documented to support.
    pub protocols: Vec<ObdProtocol>,
    /// Protocol the adapter settled on with this vehicle.
    pub vehicle_protocol: Option<ObdProtocol>,
    pub headers: Tri,
    pub spaces_off: Tri,
    pub voltage_reading: Tri,
    pub multi_frame: Tri,
    pub can_29bit: Tri,
    /// Most data bytes in one CAN frame (8 for classic CAN).
    pub max_can_frame_bytes: Option<u8>,
    /// Probably a clone (unreleased version string, failed basic commands…).
    pub clone_suspected: bool,
    pub notes: Vec<String>,
}

impl AdapterCapabilities {
    pub fn unknown(link: LinkKind) -> Self {
        Self {
            family: AdapterFamily::Unknown,
            link,
            identity: String::new(),
            description: None,
            stn_firmware: None,
            protocols: Vec::new(),
            vehicle_protocol: None,
            headers: Tri::Unknown,
            spaces_off: Tri::Unknown,
            voltage_reading: Tri::Unknown,
            multi_frame: Tri::Unknown,
            can_29bit: Tri::Unknown,
            max_can_frame_bytes: None,
            clone_suspected: false,
            notes: Vec::new(),
        }
    }
}
