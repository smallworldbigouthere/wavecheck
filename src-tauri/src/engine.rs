//! wavecheck engine: drives the yt-dlp + ffmpeg binaries.
//!
//! yt-dlp is run via `tokio::process` (not the shell plugin) so the engine can
//! transparently prefer a *self-updated* yt-dlp in the app-data dir over the
//! bundled sidecar — see `ytdlp_path`. ffmpeg/ffprobe always come from the bundle.
//!
//! Two commands are exposed:
//!   * `probe`          — resolve a URL into { title, count, kind } before downloading.
//!   * `start_download` — run the tuned download, streaming per-track progress as events.
//!
//! Progress is parsed from yt-dlp's `--progress-template` (machine-readable, tab-separated)
//! plus a few well-known stdout markers, and emitted as `wc://progress`,
//! `wc://log`, and `wc://done` events.

use std::path::PathBuf;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// A unique tab-separated prefix so we can distinguish our progress lines from
/// everything else yt-dlp prints. The fields map 1:1 to `ProgressEvent` below.
const PROGRESS_TEMPLATE: &str = "download:WCPROG\t%(info.playlist_index)s\t%(info.n_entries)s\t%(progress._percent_str)s\t%(progress._speed_str)s\t%(progress._eta_str)s\t%(info.title)s";

#[derive(Debug, Serialize)]
pub struct ProbeResult {
    pub title: String,
    pub count: u32,
    /// "playlist" or "video"
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadOptions {
    pub url: String,
    /// Output directory the user picked.
    pub out_dir: String,
    /// "video" (best mp4) or "audio" (mp3).
    pub format: String,
    /// Browser to pull cookies from, e.g. "chrome"; None to skip cookies.
    pub browser: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProgressEvent {
    index: Option<u32>,
    total: Option<u32>,
    percent: String,
    speed: String,
    eta: String,
    title: String,
}

/// Directory that holds the bundled sidecar binaries.
///  * debug build (`tauri dev`): `<crate>/binaries`
///  * release build: next to the app executable
///
/// Note: the `env!("CARGO_MANIFEST_DIR")` literal embeds an absolute build path,
/// so it is gated behind `#[cfg(debug_assertions)]` and is never compiled into a
/// release/distributed binary.
fn binary_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    #[cfg(not(debug_assertions))]
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    dir
}

/// Path of the bundled yt-dlp (triple-suffixed in dev, bare name in release).
fn bundled_ytdlp() -> PathBuf {
    #[cfg(debug_assertions)]
    let name = format!("yt-dlp-{}", env!("TARGET_TRIPLE"));
    #[cfg(all(not(debug_assertions), windows))]
    let name = "yt-dlp.exe".to_string();
    #[cfg(all(not(debug_assertions), not(windows)))]
    let name = "yt-dlp".to_string();
    binary_dir().join(name)
}

/// The yt-dlp binary to actually run: a self-updated copy in app-data if present,
/// otherwise the bundled sidecar. This is what makes the engine self-healing.
pub fn ytdlp_path(app: &AppHandle) -> PathBuf {
    let updated = crate::yt_update::updated_ytdlp_path(app);
    if updated.exists() {
        updated
    } else {
        bundled_ytdlp()
    }
}

/// Absolute path to the ffmpeg binary, so yt-dlp can be told exactly where it is.
/// yt-dlp derives the ffprobe path from this by name substitution.
fn ffmpeg_location() -> PathBuf {
    #[cfg(debug_assertions)]
    let name = format!("ffmpeg-{}", env!("TARGET_TRIPLE"));
    #[cfg(all(not(debug_assertions), windows))]
    let name = "ffmpeg.exe".to_string();
    #[cfg(all(not(debug_assertions), not(windows)))]
    let name = "ffmpeg".to_string();
    binary_dir().join(name)
}

/// Resolve a URL to a title + item count without downloading anything.
#[tauri::command]
pub async fn probe(
    app: AppHandle,
    url: String,
    browser: Option<String>,
) -> Result<ProbeResult, String> {
    let mut args: Vec<String> = vec![
        "--flat-playlist".into(),
        "--dump-single-json".into(),
        "--no-warnings".into(),
    ];
    if let Some(b) = browser.filter(|b| !b.is_empty()) {
        args.push("--cookies-from-browser".into());
        args.push(b);
    }
    args.push(url);

    let output = Command::new(ytdlp_path(&app))
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("probe failed: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp probe error: {}", err.trim()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("could not parse yt-dlp output: {e}"))?;

    let type_field = json.get("_type").and_then(|v| v.as_str()).unwrap_or("");
    let is_playlist = type_field == "playlist" || json.get("entries").is_some();

    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();

    let count = if is_playlist {
        json.get("playlist_count")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                json.get("entries")
                    .and_then(|e| e.as_array())
                    .map(|a| a.len() as u64)
            })
            .unwrap_or(0) as u32
    } else {
        1
    };

    Ok(ProbeResult {
        title,
        count,
        kind: if is_playlist {
            "playlist".into()
        } else {
            "video".into()
        },
    })
}

/// Start a download, streaming progress to the frontend. Returns once the
/// process has been spawned; completion is signalled via the `wc://done` event.
#[tauri::command]
pub async fn start_download(app: AppHandle, opts: DownloadOptions) -> Result<(), String> {
    let mut args: Vec<String> = Vec::new();

    // Cookies (the bot-check fix) — authenticate as the user's browser session.
    if let Some(b) = opts.browser.clone().filter(|b| !b.is_empty()) {
        args.push("--cookies-from-browser".into());
        args.push(b);
    }

    // Format selection.
    if opts.format == "audio" {
        args.extend([
            "-x".into(),
            "--audio-format".into(),
            "mp3".into(),
            "--audio-quality".into(),
            "0".into(),
            "--embed-metadata".into(),
            "--embed-thumbnail".into(),
        ]);
    } else {
        args.extend([
            "-f".into(),
            "bestvideo+bestaudio/best".into(),
            "--merge-output-format".into(),
            "mp4".into(),
        ]);
    }

    // Robustness + the tuned flags from the working command.
    args.extend([
        "--ignore-errors".into(),
        "--no-abort-on-error".into(),
        "--concurrent-fragments".into(),
        "4".into(),
        "--retries".into(),
        "10".into(),
        "--newline".into(),
        "--ffmpeg-location".into(),
        ffmpeg_location().to_string_lossy().to_string(),
        "--progress-template".into(),
        PROGRESS_TEMPLATE.into(),
        "-o".into(),
        format!(
            "{}/%(playlist_title)s/%(playlist_index)s - %(title)s.%(ext)s",
            opts.out_dir.trim_end_matches('/')
        ),
        opts.url.clone(),
    ]);

    let mut child = Command::new(ytdlp_path(&app))
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start download: {e}"))?;

    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;

    // Stdout carries progress-template lines.
    let h_out = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            handle_line(&h_out, line.trim_end());
        }
    });

    // Stderr carries "ERROR:" and "has already been downloaded".
    let h_err = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            handle_line(&h_err, line.trim_end());
        }
    });

    // Signal completion with the exit code.
    let h_done = app.clone();
    tauri::async_runtime::spawn(async move {
        let code = child.wait().await.ok().and_then(|s| s.code()).unwrap_or(-1);
        let _ = h_done.emit("wc://done", code);
    });

    Ok(())
}

/// Parse one line of yt-dlp output and emit the appropriate event.
fn handle_line(app: &AppHandle, line: &str) {
    if let Some(rest) = line.strip_prefix("WCPROG\t") {
        let f: Vec<&str> = rest.split('\t').collect();
        let parse_u32 = |s: Option<&&str>| s.and_then(|v| v.trim().parse::<u32>().ok());
        let ev = ProgressEvent {
            index: parse_u32(f.get(0)),
            total: parse_u32(f.get(1)),
            percent: f.get(2).unwrap_or(&"").trim().to_string(),
            speed: f.get(3).unwrap_or(&"").trim().to_string(),
            eta: f.get(4).unwrap_or(&"").trim().to_string(),
            title: f.get(5).unwrap_or(&"").trim().to_string(),
        };
        let _ = app.emit("wc://progress", ev);
        return;
    }

    // Surface meaningful status lines to the UI log.
    if line.contains("has already been downloaded")
        || line.starts_with("ERROR")
        || line.contains("[download] Downloading item")
        || line.contains("Deleting original")
        || line.contains("[ExtractAudio]")
        || line.contains("[Merger]")
    {
        let _ = app.emit("wc://log", line.to_string());
    }
}
