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
    RosePine,   // Rose Pine (#ebbcba)
    Everforest, // Everforest Green (#a7c080)
    Kanagawa,   // Kanagawa Wave (#7e9cd8)
    BloodRed,   // Crimson (#ff5555)
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
            ThemeColor::RosePine => "rosé pine",
            ThemeColor::Everforest => "everforest",
            ThemeColor::Kanagawa => "kanagawa",
            ThemeColor::BloodRed => "crimson red",
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
    Neon,     // Thin neon line (══◈══)
}

impl VisualStyle {
    pub fn name(&self) -> &'static str {
        match self {
            VisualStyle::Waveform => "waveform (peaks)",
            VisualStyle::Tape => "tape (━━●──)",
            VisualStyle::Minimal => "minimal (··◆)",
            VisualStyle::Blocky => "blocky (██▓░░)",
            VisualStyle::Braille => "braille (⣀⣤⣶⣿)",
            VisualStyle::Neon => "neon (══◈──)",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub enum PitchMode {
    #[default]
    Natural, // Pitch shifts naturally with speed (True Slowed / Nightcore)
    Keep,    // Time-stretch keeping original vocal pitch
}

impl PitchMode {
    pub fn name(&self) -> &'static str {
        match self {
            PitchMode::Natural => "natural (slowed / nightcore)",
            PitchMode::Keep => "keep pitch (time-stretch)",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub enum AudioFilter {
    #[default]
    None,
    BassBoost,
    NightcoreEq,
    VaporwaveReverb,
    SoftCompressor,
}

impl AudioFilter {
    pub fn name(&self) -> &'static str {
        match self {
            AudioFilter::None => "off (flat)",
            AudioFilter::BassBoost => "bass boost (+6dB sub)",
            AudioFilter::NightcoreEq => "bright (treble boost)",
            AudioFilter::VaporwaveReverb => "muffled lo-fi (lowpass)",
            AudioFilter::SoftCompressor => "dynamic normalizer",
        }
    }

    pub fn mpv_af(&self) -> &'static str {
        match self {
            AudioFilter::None => "",
            AudioFilter::BassBoost => "lavfi=[bass=g=6:f=110:w=0.6]",
            AudioFilter::NightcoreEq => "lavfi=[treble=g=5:f=3500]",
            AudioFilter::VaporwaveReverb => "lavfi=[lowpass=f=3200]",
            AudioFilter::SoftCompressor => "lavfi=[dynaudnorm=f=150:g=15]",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub enum RpcActivityType {
    #[default]
    Listening, // type 2 ("Listening to SoundCloud")
    Playing,   // type 0 ("Playing SoundCloud")
}

impl RpcActivityType {
    pub fn name(&self) -> &'static str {
        match self {
            RpcActivityType::Listening => "listening to SoundCloud",
            RpcActivityType::Playing => "playing SoundCloud",
        }
    }

    pub fn code(&self) -> u8 {
        match self {
            RpcActivityType::Listening => 2,
            RpcActivityType::Playing => 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub enum StreamingBufferMode {
    #[default]
    Realtime, // Low-latency direct playback (~2s buffer, instant startup)
    Balanced, // Moderate buffer (~30s buffer, safe for mobile/LTE)
    FullBuffer, // Heavy pre-cache (~300s buffer)
}

impl StreamingBufferMode {
    pub fn name(&self) -> &'static str {
        match self {
            StreamingBufferMode::Realtime => "realtime / low-latency (fast start)",
            StreamingBufferMode::Balanced => "balanced (~30s buffer)",
            StreamingBufferMode::FullBuffer => "full buffer (~300s pre-cache)",
        }
    }

    pub fn readahead_secs(&self) -> f64 {
        match self {
            StreamingBufferMode::Realtime => 2.0,
            StreamingBufferMode::Balanced => 30.0,
            StreamingBufferMode::FullBuffer => 300.0,
        }
    }

    pub fn stream_buffer_size(&self) -> &'static str {
        match self {
            StreamingBufferMode::Realtime => "32k",
            StreamingBufferMode::Balanced => "128k",
            StreamingBufferMode::FullBuffer => "512k",
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
    #[serde(default)]
    pub artwork_url: Option<String>,
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
    #[serde(default = "default_true")]
    pub show_top_status: bool,
    #[serde(default = "default_true")]
    pub show_durations: bool,
    #[serde(default = "default_true")]
    pub show_hearts: bool,
    #[serde(default = "default_bar_width")]
    pub progress_bar_width: usize,
    #[serde(default = "default_true")]
    pub mouse_support: bool,

    pub default_volume: u32,
    #[serde(default = "default_volume_step")]
    pub volume_step: u32,
    #[serde(default)]
    pub pitch_mode: PitchMode,
    #[serde(default = "default_slowed_preset")]
    pub slowed_preset: f64,
    #[serde(default = "default_speedup_preset")]
    pub speedup_preset: f64,
    #[serde(default = "default_speed_step")]
    pub speed_step: f64,
    #[serde(default)]
    pub audio_filter: AudioFilter,
    #[serde(default)]
    pub streaming_mode: StreamingBufferMode,

    #[serde(default = "default_true")]
    pub auto_wave: bool,
    #[serde(default)]
    pub resume_playback: bool,
    #[serde(default = "default_seek_step")]
    pub seek_step: u32,
    #[serde(default)]
    pub default_shuffle: bool,
    #[serde(default = "default_repeat_str")]
    pub default_repeat: String,
    #[serde(default = "default_search_limit")]
    pub search_limit: usize,

    pub discord_rpc: bool,
    #[serde(default)]
    pub rpc_activity_type: RpcActivityType,
    #[serde(default)]
    pub discord_buttons: bool,
    #[serde(default = "default_true")]
    pub discord_timestamps: bool,
    #[serde(default = "default_true")]
    pub mpris_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_lyrics: bool,

    #[serde(default = "default_true")]
    pub gapless_prefetch: bool,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,

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

fn default_volume_step() -> u32 {
    5
}

fn default_bar_width() -> usize {
    60
}

fn default_slowed_preset() -> f64 {
    0.85
}

fn default_speedup_preset() -> f64 {
    1.25
}

fn default_speed_step() -> f64 {
    0.05
}

fn default_repeat_str() -> String {
    "off".to_string()
}

fn default_search_limit() -> usize {
    30
}

fn default_history_limit() -> usize {
    200
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeColor::Auto,
            visual: VisualStyle::Waveform,
            show_bottom_hints: true,
            show_top_status: true,
            show_durations: true,
            show_hearts: true,
            progress_bar_width: 60,
            mouse_support: true,
            default_volume: 80,
            volume_step: 5,
            pitch_mode: PitchMode::Natural,
            slowed_preset: 0.85,
            speedup_preset: 1.25,
            speed_step: 0.05,
            audio_filter: AudioFilter::None,
            streaming_mode: StreamingBufferMode::Realtime,
            auto_wave: true,
            resume_playback: false,
            seek_step: 5,
            default_shuffle: false,
            default_repeat: "off".to_string(),
            search_limit: 30,
            discord_rpc: true,
            rpc_activity_type: RpcActivityType::Listening,
            discord_buttons: false,
            discord_timestamps: true,
            mpris_enabled: true,
            auto_lyrics: true,
            gapless_prefetch: true,
            history_limit: 200,
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
            if let Ok(mut data) = serde_json::from_str::<Self>(&content) {
                // Ensure playback position is never resumed across runs and track_states are cleared
                data.config.resume_playback = false;
                data.track_states.clear();
                data.save();
                return data;
            }
        }
        let bak_path = path.with_extension("json.bak");
        if let Ok(content) = fs::read_to_string(&bak_path) {
            if let Ok(mut data) = serde_json::from_str::<Self>(&content) {
                data.config.resume_playback = false;
                data.track_states.clear();
                data.save();
                return data;
            }
        }
        let default_store = Self::default();
        default_store.save();
        default_store
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
                if fs::write(&tmp, &content).is_ok() {
                    let _ = fs::rename(tmp, path);
                }
            }
            if let Ok(settings_only) = serde_json::to_string_pretty(&self.config) {
                let mut sctui_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
                sctui_dir.push("sctui");
                let _ = fs::create_dir_all(&sctui_dir);
                let _ = fs::write(sctui_dir.join("config.json"), &settings_only);

                let mut sc_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
                sc_dir.push("soundcloud-tui");
                let _ = fs::create_dir_all(&sc_dir);
                let _ = fs::write(sc_dir.join("settings.json"), &settings_only);
            }
        }
    }

    pub fn add_history(&mut self, track: TrackItem) {
        self.history.retain(|h| {
            let same_id = !h.track.id.is_empty() && !track.id.is_empty() && h.track.id == track.id;
            let same_url = !h.track.url.is_empty() && !track.url.is_empty() && h.track.url == track.url;
            !(same_id || same_url)
        });
        self.history.insert(
            0,
            HistoryEntry {
                track,
                played_at: Local::now(),
            },
        );
        let max_hist = self.config.history_limit.max(10);
        if self.history.len() > max_hist {
            self.history.truncate(max_hist);
        }
        self.save();
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn get_track_state(&self, track_id: &str) -> Option<&TrackPlaybackState> {
        self.track_states.get(track_id)
    }

    pub fn toggle_like(&mut self, track: &TrackItem) -> bool {
        if let Some(pos) = self.liked.iter().position(|t| {
            (!t.id.is_empty() && !track.id.is_empty() && t.id == track.id)
                || (!t.url.is_empty() && !track.url.is_empty() && t.url == track.url)
        }) {
            self.liked.remove(pos);
            self.save();
            false
        } else {
            self.liked.insert(0, track.clone());
            self.save();
            true
        }
    }

    #[allow(dead_code)]
    pub fn is_liked(&self, url: &str) -> bool {
        self.liked.iter().any(|t| (!url.is_empty() && t.url == url) || (!t.id.is_empty() && t.id == url))
    }

    pub fn is_track_liked(&self, track: &TrackItem) -> bool {
        self.liked.iter().any(|t| {
            (!t.id.is_empty() && !track.id.is_empty() && t.id == track.id)
                || (!t.url.is_empty() && !track.url.is_empty() && t.url == track.url)
        })
    }

    pub fn save_playlist(&mut self, playlist: PlaylistItem) {
        self.playlists.retain(|p| {
            let same_id = !p.id.is_empty() && !playlist.id.is_empty() && p.id == playlist.id;
            let same_url = !p.url.is_empty() && !playlist.url.is_empty() && p.url == playlist.url;
            !(same_id || same_url)
        });
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
            artwork_url: None,
        };
        self.playlists.push(pl);
        self.save();
        id
    }

    pub fn set_playlist_artwork(&mut self, playlist_idx: usize, url: Option<String>) -> bool {
        if let Some(pl) = self.playlists.get_mut(playlist_idx) {
            pl.artwork_url = url;
            self.save();
            return true;
        }
        false
    }

    pub fn add_track_to_playlist(&mut self, playlist_idx: usize, track: TrackItem) -> bool {
        if let Some(pl) = self.playlists.get_mut(playlist_idx) {
            let exists = pl.tracks.iter().any(|t| {
                (!t.id.is_empty() && !track.id.is_empty() && t.id == track.id)
                    || (!t.url.is_empty() && !track.url.is_empty() && t.url == track.url)
            });
            if !exists {
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

    #[test]
    fn test_track_like_and_history_matching() {
        let mut store = DataStore::default();
        let track1 = TrackItem {
            id: "sc_12345".to_string(),
            title: "Test Track".to_string(),
            artist: "Test Artist".to_string(),
            duration: 180.0,
            url: "https://soundcloud.com/test/track1".to_string(),
            stream_url: None,
            artwork_url: None,
            waveform_url: None,
            user_permalink: None,
            user_id: None,
        };

        // Same ID but different or resolved URL (e.g. YouTube fallback or Spotify)
        let track1_alt_url = TrackItem {
            id: "sc_12345".to_string(),
            title: "Test Track".to_string(),
            artist: "Test Artist".to_string(),
            duration: 180.0,
            url: "https://youtube.com/watch?v=different".to_string(),
            stream_url: None,
            artwork_url: None,
            waveform_url: None,
            user_permalink: None,
            user_id: None,
        };

        assert!(!store.is_track_liked(&track1));
        assert!(store.toggle_like(&track1));
        assert!(store.is_track_liked(&track1));
        assert!(store.is_track_liked(&track1_alt_url));

        // Toggling with alt url removes it because ID matches
        assert!(!store.toggle_like(&track1_alt_url));
        assert!(!store.is_track_liked(&track1));

        // Test history deduplication with matching ID
        store.add_history(track1.clone());
        assert_eq!(store.history.len(), 1);
        store.add_history(track1_alt_url);
        assert_eq!(store.history.len(), 1);
    }
}
