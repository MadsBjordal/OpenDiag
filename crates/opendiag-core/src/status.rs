use serde::{Deserialize, Serialize};

/// How far a function is from "works on real hardware". Shown next to every function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportStatus {
    /// Implemented, automated tests pass and it was tested on real hardware.
    ImplementedTested,
    /// Implemented and tested against the simulator, not yet on real hardware.
    ImplementedNotHardwareVerified,
    PartiallyImplemented,
    SimulatedOnly,
    Planned,
    Unsupported,
}

impl SupportStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::ImplementedTested => "Implemented and tested",
            Self::ImplementedNotHardwareVerified => "Implemented but not hardware-verified",
            Self::PartiallyImplemented => "Partially implemented",
            Self::SimulatedOnly => "Simulated only",
            Self::Planned => "Planned",
            Self::Unsupported => "Unsupported",
        }
    }
}
