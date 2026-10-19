//! The interface between the engine and a vehicle driver.
//!
//! The engine's workflows (scan, live data, clear) only use [`Driver`], so the
//! same code runs whether the vehicle is reached through an ELM327-class
//! adapter or a raw K-line cable. Every driver must check requests with
//! [`opendiag_safety::may_send`] before they reach the vehicle.

use opendiag_core::{AdapterCapabilities, LinkError, LinkInfo, ObdProtocol};
use opendiag_obd::EcuMessage;
use opendiag_safety::Authorization;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum DriverError {
    #[error("{0}")]
    Link(#[from] LinkError),
    #[error("the adapter did not answer \"{0}\" in time")]
    Timeout(String),
    #[error("no answer from the vehicle (NO DATA)")]
    NoData,
    #[error(
        "the adapter could not connect to the vehicle (UNABLE TO CONNECT). Is the ignition on?"
    )]
    UnableToConnect,
    #[error("vehicle bus error: {0}")]
    Bus(String),
    #[error("the adapter does not support \"{0}\"")]
    Unsupported(String),
    #[error("{0}")]
    Blocked(String),
    #[error("the answer could not be understood: {0}")]
    Garbled(String),
    #[error("this does not look like an ELM327-compatible adapter (it answered \"{0}\")")]
    NotElm(String),
    #[error("the vehicle did not answer the K-line wake-up: {0}")]
    InitFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Tx,
    Rx,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub ms: u64,
    pub dir: Direction,
    pub text: String,
}

pub trait Driver: Send {
    fn capabilities(&self) -> &AdapterCapabilities;
    fn protocol(&self) -> Option<ObdProtocol>;
    fn is_can(&self) -> bool;
    fn link_info(&self) -> LinkInfo;
    /// Battery voltage at the diagnostic socket, if the hardware can measure it.
    fn read_voltage(&mut self) -> Option<f64>;
    /// Wakes the vehicle up (`None` = try each protocol the driver supports)
    /// and returns the answers to the read-only request 01 00.
    fn connect_vehicle(
        &mut self,
        protocol: Option<ObdProtocol>,
    ) -> Result<(ObdProtocol, Vec<EcuMessage>), DriverError>;
    /// Sends one diagnostic request; changing requests need an [`Authorization`].
    fn request(
        &mut self,
        payload: &[u8],
        auth: Option<&Authorization>,
    ) -> Result<Vec<EcuMessage>, DriverError>;
    fn log(&self) -> &[LogEntry];
    /// Failed requests in a row.
    fn consecutive_errors(&self) -> u32;
    /// Hide VIN answers in the log.
    fn set_redact_vin(&mut self, on: bool);
    fn close(&mut self);
}

/// A bounded in-memory log shared by drivers.
#[derive(Debug, Default)]
pub struct Log {
    entries: Vec<LogEntry>,
    started: Option<std::time::Instant>,
}

const LOG_LIMIT: usize = 4000;

impl Log {
    pub fn push(&mut self, dir: Direction, text: String) {
        let started = *self.started.get_or_insert_with(std::time::Instant::now);
        if self.entries.len() >= LOG_LIMIT {
            self.entries.drain(..LOG_LIMIT / 4);
        }
        self.entries.push(LogEntry {
            ms: started.elapsed().as_millis() as u64,
            dir,
            text,
        });
    }
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }
}
