//! Update check: asks GitHub whether a newer OpenDiag build exists. This is the
//! app's only internet access; it sends nothing about the user or the vehicle
//! and can be switched off in Settings (setting `update_check`).

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;

const REPO: &str = "MadsBjordal/OpenDiag";
const DOWNLOAD_PREFIX: &str = "https://github.com/MadsBjordal/OpenDiag/releases/download/";
const MAX_DOWNLOAD: u64 = 400 * 1024 * 1024;

/// The GitHub release this build belongs to (set by the release workflow).
pub fn current_release() -> Option<&'static str> {
    option_env!("OPENDIAG_RELEASE").filter(|s| !s.is_empty())
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    /// "sha256:…" (GitHub adds it for newer uploads).
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub draft: bool,
    pub published_at: Option<String>,
    pub html_url: String,
    pub assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    /// `None` = a development build: updates are not checked.
    pub current: Option<String>,
    pub available: bool,
    pub tag: Option<String>,
    pub name: Option<String>,
    pub published_at: Option<String>,
    pub page: Option<String>,
    pub url: Option<String>,
    pub size: Option<u64>,
    pub sha256: Option<String>,
}

fn asset_for<'a>(r: &'a GhRelease, platform: &str) -> Option<&'a GhAsset> {
    r.assets.iter().find(|a| match platform {
        "windows" => a.name.ends_with("-setup.exe"),
        "android" => a.name.ends_with(".apk"),
        _ => false,
    })
}

/// Picks the newest release with a file for this platform (GitHub lists the
/// newest first) and says whether it differs from the running build.
pub fn choose(releases: &[GhRelease], current: Option<&str>, platform: &str) -> UpdateInfo {
    let newest = releases
        .iter()
        .filter(|r| !r.draft)
        .find_map(|r| asset_for(r, platform).map(|a| (r, a)));
    let mut info = UpdateInfo {
        current: current.map(str::to_string),
        available: false,
        tag: None,
        name: None,
        published_at: None,
        page: None,
        url: None,
        size: None,
        sha256: None,
    };
    let Some((r, a)) = newest else { return info };
    info.available = current.is_some() && current != Some(r.tag_name.as_str());
    info.tag = Some(r.tag_name.clone());
    info.name = r.name.clone();
    info.published_at = r.published_at.clone();
    info.page = Some(r.html_url.clone());
    info.url = Some(a.browser_download_url.clone());
    info.size = Some(a.size);
    info.sha256 = a
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .map(str::to_string);
    info
}

pub(crate) fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(concat!("OpenDiag/", env!("CARGO_PKG_VERSION")))
        .build()
}

pub fn check(platform: &str) -> Result<UpdateInfo, String> {
    let current = current_release();
    let releases: Vec<GhRelease> = agent()
        .get(&format!(
            "https://api.github.com/repos/{REPO}/releases?per_page=10"
        ))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Could not reach GitHub: {e}"))?
        .into_json()
        .map_err(|e| format!("Unexpected answer from GitHub: {e}"))?;
    Ok(choose(&releases, current, platform))
}

/// Only files from this project's GitHub releases are ever downloaded or opened.
pub fn allowed_url(url: &str) -> bool {
    url.starts_with(DOWNLOAD_PREFIX) && !url.contains("..")
}

/// Downloads the Windows installer to the temp folder and checks its SHA-256.
pub fn download(url: &str, sha256: Option<&str>) -> Result<std::path::PathBuf, String> {
    use sha2::{Digest, Sha256};
    if !allowed_url(url) {
        return Err("Refused: not an OpenDiag release file.".into());
    }
    let name: String = url
        .rsplit('/')
        .next()
        .unwrap_or("OpenDiag-setup.exe")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    let resp = agent()
        .get(url)
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(MAX_DOWNLOAD)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Download failed: {e}"))?;
    if let Some(want) = sha256 {
        let got: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if !got.eq_ignore_ascii_case(want) {
            return Err(
                "The downloaded file does not match GitHub's checksum. Nothing was installed."
                    .into(),
            );
        }
    }
    let dir = std::env::temp_dir().join("OpenDiag-update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(if name.is_empty() {
        "OpenDiag-setup.exe".into()
    } else {
        name
    });
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Windows: runs the downloaded installer silently (/S: no windows, no
/// questions; OpenDiag installs per user, so no administrator prompt), then
/// starts the new OpenDiag. Runs in a hidden helper so this app can close first.
#[cfg(windows)]
pub fn install_silently(installer: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::current_exe()?;
    // ping = a two-second pause without a console, so OpenDiag has closed before the installer runs.
    let line = format!(
        "/C \"ping -n 3 127.0.0.1 >nul & \"{}\" /S & start \"\" \"{}\"\"",
        installer.display(),
        exe.display()
    );
    std::process::Command::new("cmd")
        .raw_arg(line)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(tag: &str, files: &[&str], draft: bool) -> GhRelease {
        GhRelease {
            tag_name: tag.into(),
            name: Some(tag.into()),
            draft,
            published_at: None,
            html_url: format!("https://github.com/{REPO}/releases/tag/{tag}"),
            assets: files
                .iter()
                .map(|f| GhAsset {
                    name: f.to_string(),
                    browser_download_url: format!("{DOWNLOAD_PREFIX}{tag}/{f}"),
                    size: 1,
                    digest: Some("sha256:ab".into()),
                })
                .collect(),
        }
    }

    #[test]
    fn newer_build_is_offered_per_platform() {
        let list = vec![
            rel("test-build-3", &["x.apk"], true),
            rel("test-build-2", &["OpenDiag_0.1.0_x64-setup.exe"], false),
            rel(
                "test-build-1",
                &["OpenDiag_0.1.0_x64-setup.exe", "a.apk"],
                false,
            ),
        ];
        let w = choose(&list, Some("test-build-1"), "windows");
        assert!(w.available);
        assert_eq!(w.tag.as_deref(), Some("test-build-2"));
        assert_eq!(w.sha256.as_deref(), Some("ab"));
        // The draft and the release without an APK are skipped on Android.
        let a = choose(&list, Some("test-build-1"), "android");
        assert!(!a.available);
        // Development builds never get an update offered.
        assert!(!choose(&list, None, "windows").available);
        assert!(!choose(&list, Some("test-build-1"), "linux").available);
    }

    #[test]
    fn only_project_release_files() {
        assert!(allowed_url(&format!(
            "{DOWNLOAD_PREFIX}test-build-1/OpenDiag-android-arm64.apk"
        )));
        assert!(!allowed_url("https://example.com/OpenDiag-setup.exe"));
        assert!(!allowed_url(&format!("{DOWNLOAD_PREFIX}../../evil.exe")));
    }
}
