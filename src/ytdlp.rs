use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct LiveEntry {
    pub id: String,
    pub title: String,
    pub url: String,
    pub is_live: bool,
    pub viewers: Option<u64>,
    pub thumbnail: Option<String>,
}

/// YouTube thumbnails are predictable; works even when flat-playlist JSON
/// omits them.
pub fn thumbnail_url(id: &str) -> String {
    format!("https://i.ytimg.com/vi/{id}/maxresdefault.jpg")
}

/// Locate a yt-dlp binary: bundled sidecar next to exe, then PATH.
pub fn find_ytdlp() -> Result<PathBuf> {
    // 1. Same dir as current exe (bundled sidecar).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["yt-dlp", "yt-dlp.exe"] {
                let p = dir.join(name);
                if p.is_file() {
                    return Ok(p);
                }
            }
        }
    }
    // 2. PATH lookup.
    let name = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        let p = dir.join(name);
        if p.is_file() {
            return Ok(p);
        }
    }
    // 3. Bare name — let the OS resolve it at spawn time.
    Ok(PathBuf::from(name))
}

pub fn check_ytdlp() -> Result<String> {
    let bin = find_ytdlp()?;
    let out = Command::new(&bin)
        .arg("--version")
        .output()
        .with_context(|| format!("failed to run {}", bin.display()))?;
    if !out.status.success() {
        anyhow::bail!("yt-dlp exited with {}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Resolve a channel / live-tab URL into individual live entries via
/// `yt-dlp --flat-playlist -J`. Falls back to a single entry on failure.
pub fn list_live(channel_or_video_url: &str) -> Vec<LiveEntry> {
    match list_live_inner(channel_or_video_url) {
        Ok(v) if !v.is_empty() => v,
        _ => vec![LiveEntry {
            id: String::new(),
            title: channel_or_video_url.to_string(),
            url: channel_or_video_url.to_string(),
            is_live: true,
            viewers: None,
            thumbnail: None,
        }],
    }
}

fn list_live_inner(url: &str) -> Result<Vec<LiveEntry>> {
    let bin = find_ytdlp()?;
    let out = Command::new(&bin)
        .args(["--flat-playlist", "-J", "--no-warnings", url])
        .output()?;
    if !out.status.success() {
        anyhow::bail!("yt-dlp flat playlist failed");
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let mut entries = Vec::new();

    // Two shapes: direct video JSON, or playlist JSON with `entries`.
    if v.get("entries").is_none() {
        let (title, id) = (
            v.get("title").and_then(|t| t.as_str()).unwrap_or(url),
            v.get("id").and_then(|t| t.as_str()).unwrap_or(""),
        );
        let is_live = v
            .get("is_live")
            .and_then(|b| b.as_bool())
            .unwrap_or(v.get("live_status").and_then(|s| s.as_str()).map(|s| s == "is_live").unwrap_or(true));
        let page_url = if id.is_empty() {
            url.to_string()
        } else {
            format!("https://www.youtube.com/watch?v={id}")
        };
        entries.push(LiveEntry {
            id: id.to_string(),
            title: title.to_string(),
            url: page_url,
            is_live,
            viewers: v.get("view_count").and_then(|n| n.as_u64()),
            thumbnail: (!id.is_empty()).then(|| thumbnail_url(id)),
        });
        return Ok(entries);
    }

    if let Some(arr) = v.get("entries").and_then(|e| e.as_array()) {
        for e in arr {
            let id = e.get("id").and_then(|s| s.as_str()).unwrap_or("");
            if id.is_empty() {
                continue;
            }
            let title = e
                .get("title")
                .and_then(|s| s.as_str())
                .unwrap_or(id)
                .to_string();
            let live_status = e.get("live_status").and_then(|s| s.as_str()).unwrap_or("");
            let is_live = e
                .get("is_live")
                .and_then(|b| b.as_bool())
                .unwrap_or(live_status == "is_live");
            entries.push(LiveEntry {
                id: id.to_string(),
                title,
                url: format!("https://www.youtube.com/watch?v={id}"),
                is_live,
                viewers: e.get("view_count").and_then(|n| n.as_u64()),
                thumbnail: Some(thumbnail_url(id)),
            });
        }
    }
    Ok(entries)
}

/// Resolve a YouTube URL to a direct media URL (usually an HLS manifest for
/// lives) that ffmpeg can read. Prefers AAC to keep CPU low on the decode side.
pub fn resolve_media_url(url: &str) -> Result<String> {
    let bin = find_ytdlp()?;
    let out = Command::new(&bin)
        .args([
            "--no-playlist",
            "-g",
            "-f",
            "bestaudio[ext=m4a]/bestaudio[ext=mp4]/bestaudio[acodec^=mp4a]/bestaudio",
            "--no-warnings",
            url,
        ])
        .output()
        .with_context(|| format!("resolve {}", bin.display()))?;
    if !out.status.success() {
        anyhow::bail!(
            "yt-dlp -g failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let first = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if first.is_empty() {
        anyhow::bail!("yt-dlp -g returned no URL");
    }
    Ok(first)
}

/// Locate an ffmpeg binary: bundled sidecar next to exe, then PATH.
pub fn find_ffmpeg() -> Result<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["ffmpeg", "ffmpeg.exe"] {
                let p = dir.join(name);
                if p.is_file() {
                    return Ok(p);
                }
            }
        }
    }
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        let p = dir.join(name);
        if p.is_file() {
            return Ok(p);
        }
    }
    Ok(PathBuf::from(name))
}

/// Spawn `ffmpeg -i <media_url> -f f32le -ac 2 -ar 44100 -` with piped stdout.
/// Emits raw interleaved f32 PCM the player appends straight to rodio.
pub fn spawn_pcm_pipe(media_url: &str) -> Result<std::process::Child> {
    let bin = find_ffmpeg()?;
    let child = Command::new(&bin)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-reconnect",
            "1",
            "-reconnect_streamed",
            "1",
            "-reconnect_delay_max",
            "5",
            "-i",
            media_url,
            "-vn",
            "-f",
            "f32le",
            "-ac",
            "2",
            "-ar",
            "44100",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawn {} for HLS audio", bin.display()))?;
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_lofi_streams_tab() {
        let entries = list_live("https://www.youtube.com/@LofiGirl/streams");
        assert!(!entries.is_empty(), "expected at least one entry");
        assert!(
            entries.iter().any(|e| e.is_live),
            "expected at least one live entry"
        );
    }
}
