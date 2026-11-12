use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStream {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ViewMode {
    #[default]
    Vinyl,
    Cassette,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Vinyl => "vinyl",
            ViewMode::Cassette => "cassette",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            ViewMode::Vinyl => ViewMode::Cassette,
            ViewMode::Cassette => ViewMode::Vinyl,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_streams")]
    pub streams: Vec<UserStream>,
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Video page URL that was playing last; re-selected on launch.
    #[serde(default)]
    pub last_url: Option<String>,
    /// Whether the disc spins while playing.
    #[serde(default = "default_true")]
    pub spin: bool,
    /// Centerpiece view.
    #[serde(default)]
    pub view: ViewMode,
}

fn default_true() -> bool {
    true
}

fn default_volume() -> f32 {
    0.8
}

fn default_streams() -> Vec<UserStream> {
    vec![
        UserStream {
            name: "Lofi Girl — live streams".to_string(),
            url: "https://www.youtube.com/@LofiGirl/streams".to_string(),
        },
        UserStream {
            name: "Lofi Girl — beats to relax/study to".to_string(),
            url: "https://www.youtube.com/watch?v=rFZHOHl-L8A".to_string(),
        },
    ]
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            streams: default_streams(),
            volume: 0.8,
            last_url: None,
            spin: true,
            view: ViewMode::Vinyl,
        }
    }
}

pub fn config_path() -> PathBuf {
    if let Some(cfg) = dirs::config_dir() {
        cfg.join("lowave").join("streams.toml")
    } else {
        PathBuf::from("streams.toml")
    }
}

pub fn load() -> AppConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save(cfg: &AppConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create config dir")?;
    }
    let text = toml::to_string_pretty(cfg)?;
    fs::write(&path, text)?;
    Ok(())
}
