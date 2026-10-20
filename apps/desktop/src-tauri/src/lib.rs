//! The OpenDiag app: one Tauri shell for Windows, Linux, macOS and Android.
//! The UI calls `od_call` with a JSON command; the engine runs locally on a
//! worker thread. Internet access is limited to the optional update check
//! (`update.rs`) and opt-in community sharing (`community.rs`).

mod community;
mod update;

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

/// Asks GitHub whether a newer OpenDiag build exists for this platform.
#[tauri::command]
async fn update_check() -> Result<update::UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(|| update::check(platform()))
        .await
        .map_err(|e| e.to_string())?
}

/// Installs an update. Windows: downloads the installer, checks its checksum,
/// starts it and closes OpenDiag. Android and others: opens the download in the
/// browser, and Android's installer takes over from there.
#[tauri::command]
async fn update_install(app: AppHandle, url: String, sha256: Option<String>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    if !update::allowed_url(&url) {
        return Err("Refused: not an OpenDiag release file.".into());
    }
    if platform() == "windows" {
        let path =
            tauri::async_runtime::spawn_blocking(move || update::download(&url, sha256.as_deref()))
                .await
                .map_err(|e| e.to_string())??;
        // /P: the installer runs without questions, showing its progress.
        std::process::Command::new(&path)
            .arg("/P")
            .spawn()
            .map_err(|e| format!("Could not start the installer: {e}"))?;
        app.exit(0);
        Ok(())
    } else {
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string())
    }
}

/// Sends one anonymous share the user confirmed (see `opendiag_engine::share`).
#[tauri::command]
async fn share_send(share: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || community::send(&share))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn share_delete(id: String, token: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || community::delete(&id, &token))
        .await
        .map_err(|e| e.to_string())?
}

/// Downloads the community summaries (compatibility snapshot + fault code counts).
#[tauri::command]
async fn community_fetch() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(community::fetch)
        .await
        .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opendiag_transport::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let store = Store::open(&dir.join("opendiag.db"))?;
            let links: Box<dyn LinkFactory> =
                tauri_plugin_opendiag_transport::link_factory(app.handle());
            let api = Api::new(Engine::new(store, platform()), links);
            app.manage(AppState(Arc::new(api)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            od_call,
            save_export,
            update_check,
            update_install,
            share_send,
            share_delete,
            community_fetch
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenDiag");
}
