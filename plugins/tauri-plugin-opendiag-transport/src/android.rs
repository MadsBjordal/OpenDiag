use base64::Engine as _;
use opendiag_api::LinkFactory;
use opendiag_core::{ByteLink, KLinePort, LinkError, LinkInfo, LinkKind};
use opendiag_transport::DeviceCandidate;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{plugin::PluginHandle, Runtime};

pub struct Bridge<R: Runtime>(pub PluginHandle<R>);

pub struct AndroidLinks<R: Runtime>(pub PluginHandle<R>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceDto {
    address: String,
    name: String,
    detail: String,
    kind: String,
}

#[derive(Deserialize)]
struct DevicesDto {
    devices: Vec<DeviceDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenArgs<'a> {
    address: &'a str,
}

#[derive(Deserialize)]
struct OpenDto {
    id: i64,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WriteArgs {
    id: i64,
    data: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadArgs {
    id: i64,
    timeout_ms: u64,
    max: usize,
}

#[derive(Deserialize)]
struct ReadDto {
    data: String,
}

#[derive(Serialize)]
struct IdArgs {
    id: i64,
}

#[derive(Deserialize)]
struct Empty {}

fn kind_of(k: &str) -> LinkKind {
    match k {
        "bluetooth" => LinkKind::BluetoothClassic,
        _ => LinkKind::AndroidUsb,
    }
}

fn err(e: impl std::fmt::Display) -> LinkError {
    let m = e.to_string();
    if m.contains("ermission") {
        LinkError::Permission(m)
    } else if m.contains("closed") || m.contains("disconnect") {
        LinkError::Disconnected
    } else {
        LinkError::Io(m)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BreakArgs {
    id: i64,
    low: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BaudArgs {
    id: i64,
    baud: u32,
}

impl<R: Runtime> LinkFactory for AndroidLinks<R> {
    fn open_kline(&self, address: &str) -> Option<Result<Box<dyn KLinePort>, LinkError>> {
        if !address.starts_with("android-") {
            return None;
        }
        Some(
            self.0
                .run_mobile_plugin::<OpenDto>("openKline", OpenArgs { address })
                .map(|o| {
                    Box::new(AndroidLink {
                        handle: self.0.clone(),
                        id: o.id,
                        name: o.name,
                        kind: LinkKind::KLineCable,
                    }) as Box<dyn KLinePort>
                })
                .map_err(|e| LinkError::Open(e.to_string())),
        )
    }

    fn devices(&self) -> Vec<DeviceCandidate> {
        match self.0.run_mobile_plugin::<DevicesDto>("listDevices", ()) {
            Ok(d) => d
                .devices
                .into_iter()
                .map(|d| DeviceCandidate {
                    id: d.address.clone(),
                    kind: kind_of(&d.kind),
                    name: d.name,
                    detail: d.detail,
                    address: d.address,
                    simulated: false,
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn open(&self, address: &str) -> Option<Result<Box<dyn ByteLink>, LinkError>> {
        if !(address.starts_with("android-usb:") || address.starts_with("android-bt:")) {
            return None;
        }
        let kind = if address.starts_with("android-bt:") {
            LinkKind::BluetoothClassic
        } else {
            LinkKind::AndroidUsb
        };
        Some(
            self.0
                .run_mobile_plugin::<OpenDto>("open", OpenArgs { address })
                .map(|o| {
                    Box::new(AndroidLink {
                        handle: self.0.clone(),
                        id: o.id,
                        name: o.name,
                        kind,
                    }) as Box<dyn ByteLink>
                })
                .map_err(|e| LinkError::Open(e.to_string())),
        )
    }
}

struct AndroidLink<R: Runtime> {
    handle: PluginHandle<R>,
    id: i64,
    name: String,
    kind: LinkKind,
}

impl<R: Runtime> ByteLink for AndroidLink<R> {
    fn write_all(&mut self, data: &[u8]) -> Result<(), LinkError> {
        let data = base64::engine::general_purpose::STANDARD.encode(data);
        self.handle
            .run_mobile_plugin::<Empty>("write", WriteArgs { id: self.id, data })
            .map(|_| ())
            .map_err(err)
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, LinkError> {
        let r: ReadDto = self
            .handle
            .run_mobile_plugin(
                "read",
                ReadArgs {
                    id: self.id,
                    timeout_ms: timeout.as_millis().clamp(1, 10_000) as u64,
                    max: buf.len(),
                },
            )
            .map_err(err)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(r.data)
            .map_err(err)?;
        let n = bytes.len().min(buf.len());
        buf[..n].copy_from_slice(&bytes[..n]);
        Ok(n)
    }

    fn info(&self) -> LinkInfo {
        LinkInfo {
            kind: self.kind,
            name: self.name.clone(),
            simulated: false,
        }
    }

    fn close(&mut self) {
        let _ = self
            .handle
            .run_mobile_plugin::<Empty>("close", IdArgs { id: self.id });
    }
}

impl<R: Runtime> KLinePort for AndroidLink<R> {
    fn write(&mut self, data: &[u8]) -> Result<(), LinkError> {
        ByteLink::write_all(self, data)
    }
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, LinkError> {
        ByteLink::read(self, buf, timeout)
    }
    fn set_break(&mut self, low: bool) -> Result<(), LinkError> {
        self.handle
            .run_mobile_plugin::<Empty>("setBreak", BreakArgs { id: self.id, low })
            .map(|_| ())
            .map_err(err)
    }
    fn set_baud(&mut self, baud: u32) -> Result<(), LinkError> {
        self.handle
            .run_mobile_plugin::<Empty>("setBaud", BaudArgs { id: self.id, baud })
            .map(|_| ())
            .map_err(err)
    }
    fn clear_input(&mut self) -> Result<(), LinkError> {
        self.handle
            .run_mobile_plugin::<Empty>("clearInput", IdArgs { id: self.id })
            .map(|_| ())
            .map_err(err)
    }
    fn info(&self) -> LinkInfo {
        ByteLink::info(self)
    }
    fn close(&mut self) {
        ByteLink::close(self)
    }
}
