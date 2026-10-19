//! The OpenDiag app: one Tauri shell for Windows, Linux, macOS and Android.
//! The UI calls `od_call` with a JSON command; the engine runs locally on a
//! worker thread. Nothing here needs the internet.

use base64::Engine as _;
use opendiag_api::{Api, LinkFactory};
use opendiag_engine::Engine;
use opendiag_store::Store;
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState(Arc<Api>);

fn platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else {
        "linux"
    }
}

#[tauri::command]
async fn od_call(
    app: AppHandle,
    state: State<'_, AppState>,
    request: Value,
) -> Result<Value, String> {
    let api = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        api.call(request, &|name, data| {
            let _ = app.emit(name, data);
        })
    })
    .await
    .map_err(|e| e.to_string())
}

/// Writes an exported file to Documents/OpenDiag (desktop) or the app's exports folder (Android).
#[tauri::command]
fn save_export(app: AppHandle, filename: String, base64: String) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64)
        .map_err(|e| e.to_string())?;
    let name: String = filename
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    if name.is_empty() || name.starts_with('.') {
        return Err("bad file name".into());
    }
    let dir = if cfg!(target_os = "android") {
        app.path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("exports")
    } else {
        app.path()
            .document_dir()
            .or_else(|_| app.path().app_data_dir())
            .map_err(|e| e.to_string())?
            .join("OpenDiag")
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opendiag_transport::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let store = Store::open(&dir.join("opendiag.db"))?;
            let links: Box<dyn LinkFactory> =
                tauri_plugin_opendiag_transport::link_factory(app.handle());
            let api = Api::new(Engine::new(store, platform()), links);
            app.manage(AppState(Arc::new(api)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![od_call, save_export])
        .run(tauri::generate_context!())
        .expect("error while running OpenDiag");
}
