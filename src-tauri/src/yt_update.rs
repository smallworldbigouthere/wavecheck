//! Runtime self-update of the yt-dlp engine.
//!
//! YouTube changes frequently and breaks yt-dlp every few weeks, so a frozen
//! bundled binary would die within a month for every user. On launch (and via a
//! manual "Update engine" button) we check yt-dlp's latest GitHub release and, if
//! newer than what we have, download the standalone binary into a writable app-data
//! dir. `engine::ytdlp_path` then prefers that copy over the bundled sidecar.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

const LATEST_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";

/// `<app-data>/bin` — writable location for the updated engine.
fn bin_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("bin")
}

/// Path the self-updated yt-dlp is written to (and read from by the engine).
pub fn updated_ytdlp_path(app: &AppHandle) -> PathBuf {
    let name = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    bin_dir(app).join(name)
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatus {
    pub updated: bool,
    pub version: String,
    pub message: String,
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("wavecheck")
        .build()
        .map_err(|e| e.to_string())
}

/// Latest released yt-dlp version tag (e.g. "2026.03.17").
async fn latest_tag() -> Result<String, String> {
    let v: serde_json::Value = http()?
        .get(LATEST_API)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    v.get("tag_name")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "release has no tag_name".into())
}

/// Version reported by the yt-dlp the engine would currently run.
async fn current_version(app: &AppHandle) -> Option<String> {
    let out = tokio::process::Command::new(crate::engine::ytdlp_path(app))
        .arg("--version")
        .output()
        .await
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

/// Download the platform-appropriate standalone yt-dlp into the app-data bin dir.
async fn download_latest(app: &AppHandle) -> Result<(), String> {
    let asset = if cfg!(windows) {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else {
        "yt-dlp"
    };
    let url = format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{asset}");

    let bytes = http()?
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;

    let dir = bin_dir(app);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;
    let dest = updated_ytdlp_path(app);
    tokio::fs::write(&dest, &bytes)
        .await
        .map_err(|e| e.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&dest)
            .await
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&dest, perms)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Check for and apply a yt-dlp update. Invoked manually from the UI and on launch.
#[tauri::command]
pub async fn update_engine(app: AppHandle) -> Result<UpdateStatus, String> {
    let latest = latest_tag().await?;
    let current = current_version(&app).await;

    if current.as_deref() == Some(latest.as_str()) {
        return Ok(UpdateStatus {
            updated: false,
            version: latest,
            message: "Engine already up to date".into(),
        });
    }

    download_latest(&app).await?;
    let now = current_version(&app)
        .await
        .unwrap_or_else(|| latest.clone());
    Ok(UpdateStatus {
        updated: true,
        version: now,
        message: "Engine updated".into(),
    })
}

/// Best-effort background update at startup; emits `wc://engine` on success.
pub fn check_on_launch(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Ok(status) = update_engine(app.clone()).await {
            let _ = app.emit("wc://engine", status);
        }
    });
}
