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
    Waveform, // Dynamic SoundCloud waveform peaks (▂▃▄▅▆▇█)
    Tape,     // Smooth centered conveyor style (━━●──)
    Minimal,  // Transparent airy layout (··◆  )
    Blocky,   // Solid blocky cursor and indicators (██▓░░)
    Braille,  // High-resolution braille spectrum (⣀⣤⣶⣿)
}

impl VisualStyle {
    pub fn name(&self) -> &'static str {
        match self {
            VisualStyle::Waveform => "waveform (peaks)",
            VisualStyle::Tape => "tape (━━●──)",
            VisualStyle::Minimal => "minimal (··◆)",
            VisualStyle::Blocky => "blocky (██▓░░)",
            VisualStyle::Braille => "braille (⣀⣤⣶⣿)",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum PitchMode {
    Natural, // Pitch shifts naturally with speed (True Slowed / Nightcore)
    Keep,    // Time-stretch keeping original vocal pitch
}

impl Default for PitchMode {
    fn default() -> Self {
        PitchMode::Natural
    }
}

impl PitchMode {
    pub fn name(&self) -> &'static str {
        match self {
            PitchMode::Natural => "natural (slowed / nightcore)",
            PitchMode::Keep => "keep pitch (time-stretch)",
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
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub waveform_url: Option<String>,
    #[serde(default)]
    pub user_permalink: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlaylistItem {
    pub id: String,
    pub title: String,
    pub author: String,
    pub track_count: usize,
    pub url: String,
    pub tracks: Vec<TrackItem>,
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
    #[serde(default = "default_true")]
    pub show_bottom_hints: bool,
    pub default_volume: u32,
    #[serde(default)]
    pub pitch_mode: PitchMode,
    #[serde(default = "default_true")]
    pub auto_wave: bool,
    #[serde(default = "default_true")]
    pub resume_playback: bool,
    #[serde(default = "default_true")]
    pub gapless_prefetch: bool,
    #[serde(default = "default_seek_step")]
    pub seek_step: u32,
    pub discord_rpc: bool,
    #[serde(default)]
    pub discord_buttons: bool,
    pub discord_client_id: Option<String>,
    pub client_id: Option<String>,
    pub oauth_token: Option<String>,
    pub proxy: Option<String>,
    pub cookies_path: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_seek_step() -> u32 {
    5
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeColor::Auto,
            visual: VisualStyle::Waveform,
            show_bottom_hints: true,
            default_volume: 80,
            pitch_mode: PitchMode::Natural,
            auto_wave: true,
            resume_playback: true,
            gapless_prefetch: true,
            seek_step: 5,
            discord_rpc: true,
            discord_buttons: false,
            discord_client_id: None,
            client_id: None,
            oauth_token: None,
            proxy: None,
            cookies_path: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackPlaybackState {
    pub position: f64,
    #[serde(default = "default_track_speed")]
    pub speed: f64,
}

fn default_track_speed() -> f64 {
    1.0
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct DataStore {
    pub config: Config,
    pub history: Vec<HistoryEntry>,
    pub liked: Vec<TrackItem>,
    #[serde(default)]
    pub playlists: Vec<PlaylistItem>,
    #[serde(default)]
    pub track_states: std::collections::HashMap<String, TrackPlaybackState>,
}

impl DataStore {
    pub fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("soundcloud-tui");
        fs::create_dir_all(&path).ok();
        path.push("config.json");
        path
    }

    pub fn cache_dir() -> PathBuf {
        let mut path = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("soundcloud-tui");
        path.push("tracks");
        fs::create_dir_all(&path).ok();
        path
    }

    pub fn get_cached_track_path(id: &str) -> Option<PathBuf> {
        if id.is_empty() {
            return None;
        }
        let safe_name: String = id.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
        let m4a_path = Self::cache_dir().join(format!("{}.m4a", safe_name));
        if m4a_path.exists() && fs::metadata(&m4a_path).map(|m| m.len() > 100 * 1024).unwrap_or(false) {
            return Some(m4a_path);
        }
        let mp3_path = Self::cache_dir().join(format!("{}.mp3", safe_name));
        if mp3_path.exists() && fs::metadata(&mp3_path).map(|m| m.len() > 100 * 1024).unwrap_or(false) {
            return Some(mp3_path);
        }
        None
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(data) = serde_json::from_str(&content) {
                return data;
            }
        }
        let bak_path = path.with_extension("json.bak");
        if let Ok(content) = fs::read_to_string(&bak_path) {
            if let Ok(data) = serde_json::from_str(&content) {
                return data;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        #[cfg(test)]
        {
            return;
        }
        #[cfg(not(test))]
        {
            let path = Self::config_path();
            if let Ok(content) = serde_json::to_string_pretty(self) {
                if path.exists() {
                    let bak = path.with_extension("json.bak");
                    let _ = fs::copy(&path, bak);
                }
                let tmp = path.with_extension("tmp");
                if fs::write(&tmp, content).is_ok() {
                    let _ = fs::rename(tmp, path);
                }
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

    pub fn save_track_state(&mut self, track_id: &str, position: f64, speed: f64) {
        if track_id.is_empty() {
            return;
        }
        self.track_states.insert(
            track_id.to_string(),
            TrackPlaybackState { position, speed },
        );
        if self.track_states.len() > 500 {
            // Keep size bounded
            if let Some(first_key) = self.track_states.keys().next().cloned() {
                self.track_states.remove(&first_key);
            }
        }
        self.save();
    }

    pub fn get_track_state(&self, track_id: &str) -> Option<&TrackPlaybackState> {
        self.track_states.get(track_id)
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

    pub fn save_playlist(&mut self, playlist: PlaylistItem) {
        self.playlists.retain(|p| p.url != playlist.url && p.id != playlist.id);
        self.playlists.insert(0, playlist);
        self.save();
    }

    pub fn create_playlist(&mut self, title: &str) -> String {
        let id = format!("local_{}", Local::now().timestamp_millis());
        let pl = PlaylistItem {
            id: id.clone(),
            title: title.to_string(),
            author: "Local Library".to_string(),
            track_count: 0,
            url: format!("local://{}", id),
            tracks: Vec::new(),
        };
        self.playlists.push(pl);
        self.save();
        id
    }

    pub fn add_track_to_playlist(&mut self, playlist_idx: usize, track: TrackItem) -> bool {
        if let Some(pl) = self.playlists.get_mut(playlist_idx) {
            if !pl.tracks.iter().any(|t| t.url == track.url) {
                pl.tracks.push(track);
                pl.track_count = pl.tracks.len();
                self.save();
                return true;
            }
        }
        false
    }

    pub fn remove_track_from_playlist(&mut self, playlist_idx: usize, track_idx: usize) -> Option<TrackItem> {
        if let Some(pl) = self.playlists.get_mut(playlist_idx) {
            if track_idx < pl.tracks.len() {
                let removed = pl.tracks.remove(track_idx);
                pl.track_count = pl.tracks.len();
                self.save();
                return Some(removed);
            }
        }
        None
    }

    pub fn remove_playlist(&mut self, index: usize) -> Option<PlaylistItem> {
        if index < self.playlists.len() {
            let removed = self.playlists.remove(index);
            self.save();
            Some(removed)
        } else {
            None
        }
    }

    pub fn cache_size_label() -> String {
        let dir = Self::cache_dir();
        let mut total_bytes: u64 = 0;
        let mut count: usize = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        total_bytes += meta.len();
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".m4a") || name.ends_with(".mp3") {
                            count += 1;
                        }
                    }
                }
            }
        }
        let mb = total_bytes as f64 / (1024.0 * 1024.0);
        format!("{:.1} MB ({} tracks)", mb, count)
    }

    pub fn clear_audio_cache() -> usize {
        let dir = Self::cache_dir();
        let mut removed = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && fs::remove_file(path).is_ok() {
                    removed += 1;
                }
            }
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_playback_state_persistence() {
        let mut store = DataStore::default();
        store.save_track_state("track_123", 42.5, 1.25);

        let state = store.get_track_state("track_123").expect("state should exist");
        assert_eq!(state.position, 42.5);
        assert_eq!(state.speed, 1.25);

        // Update with 0.85
        store.save_track_state("track_123", 10.0, 0.85);
        let updated = store.get_track_state("track_123").unwrap();
        assert_eq!(updated.position, 10.0);
        assert_eq!(updated.speed, 0.85);
    }
}
