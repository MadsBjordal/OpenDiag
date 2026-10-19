//! Bridges Android's USB host and Bluetooth APIs to OpenDiag's engine.
//!
//! On Android the Kotlin side (`android/…/TransportPlugin.kt`) opens the device
//! and moves bytes; the Rust side wraps that as an [`opendiag_core::ByteLink`],
//! so the same ELM327 driver runs on top. On desktop this plugin does nothing:
//! serial ports and TCP are opened directly by the engine.

use opendiag_api::LinkFactory;
use tauri::{plugin::TauriPlugin, AppHandle, Runtime};

#[cfg(target_os = "android")]
mod android;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("opendiag-transport")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle =
                    _api.register_android_plugin("org.opendiag.transport", "TransportPlugin")?;
                _app.manage(android::Bridge(handle));
            }
            Ok(())
        })
        .build()
}

/// The platform's extra links: Android USB/Bluetooth, nothing on desktop.
pub fn link_factory<R: Runtime>(_app: &AppHandle<R>) -> Box<dyn LinkFactory> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let bridge = _app.state::<android::Bridge<R>>();
        return Box::new(android::AndroidLinks(bridge.0.clone()));
    }
    #[cfg(not(target_os = "android"))]
    Box::new(opendiag_api::NoExtraLinks)
}
