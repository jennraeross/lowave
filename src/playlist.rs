use crate::ytdlp::{LiveEntry, MediaKind};
use std::path::{Path, PathBuf};

/// An imported .m3u playlist: name + source file. Entries are re-parsed
/// from disk on every refresh so edits to the file pick up.
#[derive(Debug, Clone)]
pub struct PlaylistFile {
    pub name: String,
    pub path: PathBuf,
    pub entries: Vec<LiveEntry>,
    pub missing: usize,
}

/// Parse an .m3u/.m3u8 file into entries. Understands `#EXTINF` titles;
/// every other `#` line is a comment. Items may be absolute paths, paths
/// relative to the playlist file, or remote URLs.
pub fn load_m3u(name: &str, path: &Path) -> PlaylistFile {
    let mut entries = Vec::new();
    let mut missing = 0usize;
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut pending_title: Option<String> = None;
    let mut idx = 0usize;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            // "#EXTINF:123,Artist - Title" — keep text after the comma.
            pending_title = rest.split_once(',').map(|(_, t)| t.trim().to_string());
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        idx += 1;
        let title = pending_title
            .take()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| display_name(line));
        if line.starts_with("http://") || line.starts_with("https://") {
            entries.push(LiveEntry {
                id: format!("pl:{name}:{idx}"),
                title,
                url: line.to_string(),
                is_live: false,
                viewers: None,
                thumbnail: None,
                media: MediaKind::Direct(line.to_string()),
                playlist: Some(name.to_string()),
            });
            continue;
        }
        // Local path: absolute, or relative to the playlist file.
        let candidate = PathBuf::from(line);
        let resolved = if candidate.is_absolute() {
            candidate
        } else {
            path.parent()
                .unwrap_or_else(|| Path::new("."))
                .join(&candidate)
        };
        if resolved.is_file() {
            let s = resolved.to_string_lossy().to_string();
            entries.push(LiveEntry {
                id: format!("pl:{name}:{idx}"),
                title,
                url: s.clone(),
                is_live: false,
                viewers: None,
                thumbnail: None,
                media: MediaKind::Direct(s),
                playlist: Some(name.to_string()),
            });
        } else {
            missing += 1;
        }
    }

    PlaylistFile {
        name: name.to_string(),
        path: path.to_path_buf(),
        entries,
        missing,
    }
}

/// One editable builder row: display title + target (local path or URL).
#[derive(Debug, Clone)]
pub struct DraftEntry {
    pub title: String,
    pub target: String,
}

impl DraftEntry {
    pub fn new(title: String, target: String) -> Self {
        Self {
            title: title.lines().next().unwrap_or("").to_string(),
            target: target.lines().next().unwrap_or("").to_string(),
        }
    }
}

/// Write entries as `#EXTM3U` with `#EXTINF` titles. One entry per line, so
/// embedded newlines are stripped on the way in (see `DraftEntry::new`).
pub fn write_m3u(path: &Path, entries: &[DraftEntry]) -> std::io::Result<()> {
    use std::fmt::Write as _;
    let mut out = String::from("#EXTM3U\n");
    for e in entries {
        let _ = writeln!(out, "#EXTINF:-1,{}", e.title);
        let _ = writeln!(out, "{}", e.target);
    }
    std::fs::write(path, out)
}

fn display_name(line: &str) -> String {    // Last path segment, minus query strings and extension.
    let seg = line.rsplit(['/', '\\']).next().unwrap_or(line);
    let seg = seg.split('?').next().unwrap_or(seg);
    if let Some(dot) = seg.rfind('.') {
        if dot > 0 {
            return seg[..dot].to_string();
        }
    }
    seg.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("lowave-test-m3u");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn parses_extinf_titles_urls_and_relative_paths() {
        let dir = std::env::temp_dir().join("lowave-test-m3u");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("track one.mp3"), b"fake").unwrap();
        let pl = write_tmp(
            "mix.m3u",
            "#EXTM3U\n#EXTINF:123,Artist - Title\ntrack one.mp3\n# a comment\nhttps://example.com/stream?a=1\n./missing.flac\n",
        );
        let parsed = load_m3u("mix", &pl);
        assert_eq!(parsed.entries.len(), 2, "want file + URL entries");
        assert_eq!(parsed.missing, 1);
        assert_eq!(parsed.entries[0].title, "Artist - Title");
        assert!(matches!(parsed.entries[0].media, MediaKind::Direct(_)));
        assert!(parsed.entries[0].url.ends_with("track one.mp3"));
        assert!(matches!(parsed.entries[1].media, MediaKind::Direct(_)));
        assert_eq!(
            parsed.entries[1].media,
            MediaKind::Direct("https://example.com/stream?a=1".to_string())
        );
        assert_eq!(parsed.entries[1].title, "stream");
        assert_eq!(parsed.entries[1].playlist.as_deref(), Some("mix"));
    }

    #[test]
    fn write_then_parse_round_trips() {
        let dir = std::env::temp_dir().join("lowave-test-m3u");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("track one.mp3"), b"fake").unwrap();
        let entries = vec![
            DraftEntry::new(
                "Artist - Title, with comma".into(),
                dir.join("track one.mp3").to_string_lossy().to_string(),
            ),
            DraftEntry::new("Remote".into(), "https://example.com/live".into()),
            // Newline in a title is stripped so the file stays line-based.
            DraftEntry::new(
                "Multi\nline title".into(),
                dir.join("track one.mp3").to_string_lossy().to_string(),
            ),
        ];
        let path = dir.join("roundtrip.m3u");
        write_m3u(&path, &entries).unwrap();
        let parsed = load_m3u("rt", &path);
        assert_eq!(parsed.entries.len(), 3, "kept entries only");
        assert_eq!(parsed.missing, 0);
        assert_eq!(parsed.entries[0].title, "Artist - Title, with comma");
        assert_eq!(parsed.entries[1].title, "Remote");
        assert_eq!(parsed.entries[2].title, "Multi");
    }
}
