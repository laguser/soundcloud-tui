use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum ThemeColor {
    Auto,       // Default terminal accent / cyan
    Catppuccin, // Mocha Mauve (#cba6f7)
    TokyoNight, // Neon Cyan (#7dcfff)
    Nord,       // Frost Blue (#88c0d0)
    Gruvbox,    // Retro Orange (#fe8019)
    Dracula,    // Purple (#bd93f9)
    Amber,      // Vintage Amber (#ffb000)
    White,      // Pure Monochrome
}

impl ThemeColor {
    pub fn name(&self) -> &'static str {
        match self {
            ThemeColor::Auto => "auto (term)",
            ThemeColor::Catppuccin => "catppuccin",
            ThemeColor::TokyoNight => "tokyo night",
            ThemeColor::Nord => "nord",
            ThemeColor::Gruvbox => "gruvbox",
            ThemeColor::Dracula => "dracula",
            ThemeColor::Amber => "amber",
            ThemeColor::White => "white",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum VisualStyle {
    Tape,      // Smooth centered conveyor style like ttype
    Minimal,   // Transparent airy layout
    Blocky,    // Solid blocky cursor and indicators
}

impl VisualStyle {
    pub fn name(&self) -> &'static str {
        match self {
            VisualStyle::Tape => "tape",
            VisualStyle::Minimal => "minimal",
            VisualStyle::Blocky => "blocky",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackItem {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration: f64,
    pub url: String,
    #[serde(default)]
    pub stream_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub track: TrackItem,
    pub played_at: DateTime<Local>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub theme: ThemeColor,
    pub visual: VisualStyle,
    pub default_volume: u32,
    pub cookies_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeColor::Auto,
            visual: VisualStyle::Tape,
            default_volume: 80,
            cookies_path: None,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct DataStore {
    pub config: Config,
    pub history: Vec<HistoryEntry>,
    pub liked: Vec<TrackItem>,
}

impl DataStore {
    fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("soundcloud-tui");
        fs::create_dir_all(&path).ok();
        path.push("config.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(data) = serde_json::from_str(&content) {
                return data;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let tmp = path.with_extension("tmp");
            if fs::write(&tmp, content).is_ok() {
                let _ = fs::rename(tmp, path);
            }
        }
    }

    pub fn add_history(&mut self, track: TrackItem) {
        self.history.retain(|h| h.track.url != track.url);
        self.history.insert(
            0,
            HistoryEntry {
                track,
                played_at: Local::now(),
            },
        );
        if self.history.len() > 200 {
            self.history.truncate(200);
        }
        self.save();
    }

    pub fn toggle_like(&mut self, track: &TrackItem) -> bool {
        if let Some(pos) = self.liked.iter().position(|t| t.url == track.url) {
            self.liked.remove(pos);
            self.save();
            false
        } else {
            self.liked.insert(0, track.clone());
            self.save();
            true
        }
    }

    pub fn is_liked(&self, url: &str) -> bool {
        self.liked.iter().any(|t| t.url == url)
    }
}
