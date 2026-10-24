use crate::audio::AudioPlayer;
use crate::search::{SearchEngine, SearchResult};
use crate::storage::{DataStore, PitchMode, ThemeColor, TrackItem, VisualStyle};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::time::Instant;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn name(&self) -> &'static str {
        match self {
            RepeatMode::Off => "off",
            RepeatMode::All => "all",
            RepeatMode::One => "one",
        }
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ViewMode {
    Search,
    Queue,
    Playlists,
    PlaylistDetail,
    Favorites,
    History,
    UserProfile,
    Lyrics,
    Settings,
}

impl ViewMode {
    pub fn name(&self) -> &'static str {
        match self {
            ViewMode::Search => "search",
            ViewMode::Queue => "queue",
            ViewMode::Playlists => "playlists",
            ViewMode::PlaylistDetail => "playlist",
            ViewMode::Favorites => "favorites",
            ViewMode::History => "history",
            ViewMode::UserProfile => "profile",
            ViewMode::Lyrics => "lyrics",
            ViewMode::Settings => "settings",
        }
    }
}

#[derive(Debug)]
pub enum StreamResolveEvent {
    Resolved {
        track_id: String,
        url: String,
        source: &'static str,
    },
    Failed {
        track_id: String,
        title: String,
    },
}

pub struct App {
    pub view_mode: ViewMode,
    pub query_chars: Vec<char>,
    pub input_cursor: usize,
    pub is_searching: bool,
    pub is_editing_search: bool,

    // Track lists & indices
    pub search_results: Vec<TrackItem>,
    pub search_cursor: usize,

    pub queue: Vec<TrackItem>,
    pub queue_cursor: usize,
    pub current_track_idx: Option<usize>,

    pub playlists_cursor: usize,
    pub selected_playlist_idx: Option<usize>,
    pub playlist_detail_cursor: usize,

    pub history_cursor: usize,
    pub favorites_cursor: usize,
    pub settings_cursor: usize,

    // User Profile view
    pub current_user_profile: Option<crate::search::UserProfile>,
    pub user_profile_cursor: usize,

    // Modal dialogs: Creating playlist or adding track to playlist
    pub modal_input: Option<Vec<char>>,
    pub modal_target_track: Option<TrackItem>,
    pub modal_playlist_cursor: usize,

    // Audio status & playback modes
    pub player: AudioPlayer,
    pub search_engine: SearchEngine,
    pub discord: crate::discord::DiscordRpc,
    pub mpris: crate::mpris::MprisManager,
    pub lyrics_manager: crate::lyrics::LyricsManager,
    pub current_lyrics: Option<crate::lyrics::TrackLyrics>,
    pub store: DataStore,

    pub is_paused: bool,
    pub current_pos: f64,
    pub total_duration: f64,
    pub volume: u32,
    pub current_speed: f64,
    pub pending_resume_pos: Option<f64>,
    pub last_saved_pos_instant: Option<Instant>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub prefetched_track_id: Option<String>,
    pub related_fetched_for_id: Option<String>,
    pub pending_auto_wave_play: bool,

    // Dynamic waveform samples
    pub current_waveform: Vec<u8>,
    pub loaded_waveform_track_id: Option<String>,

    pub stream_resolve_tx: std::sync::mpsc::Sender<StreamResolveEvent>,
    pub stream_resolve_rx: std::sync::mpsc::Receiver<StreamResolveEvent>,

    pub status_message: Option<(String, Instant)>,
    pub last_esc_press: Option<Instant>,
    pub last_rpc_update: Option<Instant>,
}

impl App {
    pub fn new() -> Self {
        let store = DataStore::load();
        let volume = store.config.default_volume;
        let player = AudioPlayer::new();
        player.set_volume(volume as f64);
        player.set_pitch_correction(store.config.pitch_mode == PitchMode::Keep);
        let discord = crate::discord::DiscordRpc::new(store.config.discord_client_id.clone());
        let mpris = crate::mpris::MprisManager::new();
        let lyrics_manager = crate::lyrics::LyricsManager::new();
        let (stream_resolve_tx, stream_resolve_rx) = std::sync::mpsc::channel();

        Self {
            view_mode: ViewMode::Search,
            query_chars: Vec::new(),
            input_cursor: 0,
            is_searching: false,
            is_editing_search: true,

            search_results: Vec::new(),
            search_cursor: 0,

            queue: Vec::new(),
            queue_cursor: 0,
            current_track_idx: None,

            playlists_cursor: 0,
            selected_playlist_idx: None,
            playlist_detail_cursor: 0,

            history_cursor: 0,
            favorites_cursor: 0,
            settings_cursor: 0,

            current_user_profile: None,
            user_profile_cursor: 0,

            modal_input: None,
            modal_target_track: None,
            modal_playlist_cursor: 0,

            player,
            search_engine: SearchEngine::new(store.config.client_id.clone()),
            discord,
            mpris,
            lyrics_manager,
            current_lyrics: None,
            store,

            stream_resolve_tx,
            stream_resolve_rx,

            is_paused: false,
            current_pos: 0.0,
            total_duration: 0.0,
            volume,
            current_speed: 1.0,
            pending_resume_pos: None,
            last_saved_pos_instant: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            prefetched_track_id: None,
            related_fetched_for_id: None,
            pending_auto_wave_play: false,
            current_waveform: Vec::new(),
            loaded_waveform_track_id: None,
            status_message: Some(("Ready. Type to search SoundCloud".to_string(), Instant::now())),
            last_esc_press: None,
            last_rpc_update: None,
        }
    }

    pub fn set_status(&mut self, msg: &str) {
        self.status_message = Some((msg.to_string(), Instant::now()));
    }

    pub fn theme_accent(&self) -> Color {
        match self.store.config.theme {
            ThemeColor::Auto => Color::Cyan,
            ThemeColor::Catppuccin => Color::Rgb(203, 166, 247),
            ThemeColor::TokyoNight => Color::Rgb(125, 207, 255),
            ThemeColor::Nord => Color::Rgb(136, 192, 208),
            ThemeColor::Gruvbox => Color::Rgb(254, 128, 25),
            ThemeColor::Dracula => Color::Rgb(189, 147, 249),
            ThemeColor::Amber => Color::Rgb(255, 176, 0),
            ThemeColor::White => Color::White,
        }
    }

    pub fn current_track(&self) -> Option<&TrackItem> {
        if let Some(idx) = self.current_track_idx {
            self.queue.get(idx)
        } else {
            None
        }
    }

    pub fn play_track(&mut self, track: TrackItem) {
        // Remember previous track position and speed before switching
        let prev_id = self.current_track().map(|t| t.id.clone());
        if let Some(id) = prev_id {
            if self.current_pos > 1.0 {
                self.store.save_track_state(&id, self.current_pos, self.current_speed);
            }
        }

        self.store.add_history(track.clone());

        // Put in queue if not present or select it
        if let Some(pos) = self.queue.iter().position(|t| t.url == track.url) {
            self.current_track_idx = Some(pos);
        } else {
            self.queue.push(track.clone());
            self.current_track_idx = Some(self.queue.len() - 1);
        }
        self.is_paused = false;
        self.total_duration = track.duration;
        self.prefetched_track_id = None;

        // Restore saved position and speed for this track if remembered
        if self.store.config.resume_playback {
            if let Some(state) = self.store.get_track_state(&track.id).cloned() {
                self.current_speed = state.speed;
                if state.position > 3.0 && (track.duration <= 0.0 || state.position < track.duration - 5.0) {
                    self.pending_resume_pos = Some(state.position);
                    self.current_pos = state.position;
                } else {
                    self.pending_resume_pos = None;
                    self.current_pos = 0.0;
                }
            } else {
                self.current_speed = 1.0;
                self.pending_resume_pos = None;
                self.current_pos = 0.0;
            }
        } else {
            self.current_speed = 1.0;
            self.pending_resume_pos = None;
            self.current_pos = 0.0;
        }

        // Update MPRIS2 for Caelestia Shell and system media keys immediately
        self.mpris.update(
            &track.title,
            &track.artist,
            track.artwork_url.as_deref(),
            track.duration,
            false,
        );

        // Request lyrics
        self.current_lyrics = None;
        self.lyrics_manager.request_lyrics(&track.id, &track.title, &track.artist);

        // Fetch waveform if available
        self.fetch_waveform_for_track(&track);

        // 1. Check local on-disk cache (instant 0ms playback)
        if let Some(cached_file) = crate::storage::DataStore::get_cached_track_path(&track.id) {
            let path_str = cached_file.to_string_lossy().to_string();
            self.player.play(&path_str);
            if self.current_speed != 1.0 {
                self.player.set_speed(self.current_speed);
            }
            if let Some(pos) = self.pending_resume_pos.take() {
                self.player.seek_to(pos);
                self.set_status(&format!("Resumed at {}: {}", crate::ui::format_duration(pos), track.title));
            } else {
                self.set_status(&format!("Playing (cached): {}", track.title));
            }
            return;
        }

        // 2. Resolve network stream asynchronously in background (UI never freezes!)
        self.set_status(&format!("Loading: {}", track.title));
        let tx = self.stream_resolve_tx.clone();
        let cid_holder = std::sync::Arc::clone(&self.search_engine.cached_client_id);
        let track_clone = track.clone();
        std::thread::spawn(move || {
            if let Some(resolved) = crate::search::SearchEngine::resolve_stream_url(&track_clone, &cid_holder) {
                let _ = tx.send(StreamResolveEvent::Resolved {
                    track_id: track_clone.id,
                    url: resolved,
                    source: "direct",
                });
            } else {
                let _ = tx.send(StreamResolveEvent::Failed {
                    track_id: track_clone.id,
                    title: track_clone.title,
                });
            }
        });
    }

    fn fetch_waveform_for_track(&mut self, track: &TrackItem) {
        if self.loaded_waveform_track_id.as_deref() == Some(&track.id) {
            return;
        }
        self.loaded_waveform_track_id = Some(track.id.clone());
        self.current_waveform.clear();

        if let Some(wf_url) = &track.waveform_url {
            let url_clone = wf_url.clone();
            let track_id_clone = track.id.clone();
            std::thread::spawn(move || {
                let out = std::process::Command::new("curl")
                    .arg("-s")
                    .arg("-m")
                    .arg("4")
                    .arg("-H")
                    .arg("User-Agent: Mozilla/5.0")
                    .arg(&url_clone)
                    .output();
                if let Ok(res) = out {
                    if res.status.success() {
                        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&res.stdout) {
                            if let Some(samples) = v.get("samples").and_then(|s| s.as_array()) {
                                let bytes: Vec<u8> = samples.iter().filter_map(|n| n.as_u64().map(|u| (u.min(100)) as u8)).collect();
                                let path = crate::storage::DataStore::cache_dir().join(format!("{}.wave", track_id_clone));
                                let _ = std::fs::write(path, bytes);
                            }
                        }
                    }
                }
            });
        }
    }

    pub fn play_selected_in_current_view(&mut self) {
        match self.view_mode {
            ViewMode::Search => {
                if !self.search_results.is_empty() && self.search_cursor < self.search_results.len() {
                    self.queue = self.search_results.clone();
                    self.current_track_idx = Some(self.search_cursor);
                    let track = self.queue[self.search_cursor].clone();
                    self.play_track(track);
                }
            }
            ViewMode::Queue => {
                if !self.queue.is_empty() && self.queue_cursor < self.queue.len() {
                    self.current_track_idx = Some(self.queue_cursor);
                    let track = self.queue[self.queue_cursor].clone();
                    self.play_track(track);
                }
            }
            ViewMode::Playlists => {
                if !self.store.playlists.is_empty() && self.playlists_cursor < self.store.playlists.len() {
                    self.selected_playlist_idx = Some(self.playlists_cursor);
                    self.playlist_detail_cursor = 0;
                    self.view_mode = ViewMode::PlaylistDetail;
                }
            }
            ViewMode::PlaylistDetail => {
                if let Some(idx) = self.selected_playlist_idx {
                    if let Some(pl) = self.store.playlists.get(idx) {
                        if !pl.tracks.is_empty() && self.playlist_detail_cursor < pl.tracks.len() {
                            self.queue = pl.tracks.clone();
                            self.current_track_idx = Some(self.playlist_detail_cursor);
                            let track = self.queue[self.playlist_detail_cursor].clone();
                            self.play_track(track);
                        }
                    }
                }
            }
            ViewMode::UserProfile => {
                if let Some(p) = &self.current_user_profile {
                    if !p.tracks.is_empty() && self.user_profile_cursor < p.tracks.len() {
                        self.queue = p.tracks.clone();
                        self.current_track_idx = Some(self.user_profile_cursor);
                        let track = self.queue[self.user_profile_cursor].clone();
                        self.play_track(track);
                    }
                }
            }
            ViewMode::Favorites => {
                if !self.store.liked.is_empty() && self.favorites_cursor < self.store.liked.len() {
                    self.queue = self.store.liked.clone();
                    self.current_track_idx = Some(self.favorites_cursor);
                    let track = self.queue[self.favorites_cursor].clone();
                    self.play_track(track);
                }
            }
            ViewMode::History => {
                if !self.store.history.is_empty() && self.history_cursor < self.store.history.len() {
                    self.queue = self.store.history.iter().map(|e| e.track.clone()).collect();
                    self.current_track_idx = Some(self.history_cursor);
                    let track = self.queue[self.history_cursor].clone();
                    self.play_track(track);
                }
            }
            ViewMode::Settings => {
                self.cycle_setting();
            }
            ViewMode::Lyrics => {
                self.toggle_pause();
            }
        }
    }

    pub fn play_next(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        if self.repeat == RepeatMode::One {
            if let Some(idx) = self.current_track_idx {
                if let Some(track) = self.queue.get(idx).cloned() {
                    self.play_track(track);
                    return;
                }
            }
        }

        if self.shuffle && self.queue.len() > 1 {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let mut next_idx = rng.gen_range(0..self.queue.len());
            if Some(next_idx) == self.current_track_idx {
                next_idx = (next_idx + 1) % self.queue.len();
            }
            let track = self.queue[next_idx].clone();
            self.current_track_idx = Some(next_idx);
            self.play_track(track);
            return;
        }

        let next_idx = match self.current_track_idx {
            Some(i) if i + 1 < self.queue.len() => i + 1,
            _ => {
                if self.repeat == RepeatMode::Off && self.store.config.auto_wave {
                    if let Some(cur) = self.current_track().cloned() {
                        self.pending_auto_wave_play = true;
                        self.related_fetched_for_id = Some(cur.id.clone());
                        self.set_status("Auto-Wave: loading related tracks...");
                        self.search_engine.fetch_related(&cur.id);
                        return;
                    }
                    0
                } else if self.repeat == RepeatMode::All || self.repeat == RepeatMode::Off {
                    0
                } else {
                    return;
                }
            }
        };
        let track = self.queue[next_idx].clone();
        self.current_track_idx = Some(next_idx);
        self.play_track(track);
    }

    pub fn play_prev(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let prev_idx = match self.current_track_idx {
            Some(i) if i > 0 => i - 1,
            _ => self.queue.len().saturating_sub(1),
        };
        let track = self.queue[prev_idx].clone();
        self.current_track_idx = Some(prev_idx);
        self.play_track(track);
    }

    pub fn toggle_pause(&mut self) {
        if self.current_track().is_none() {
            return;
        }
        self.is_paused = !self.is_paused;
        self.player.set_pause(self.is_paused);
        self.set_status(if self.is_paused { "Paused" } else { "Playing" });
        if let Some(track) = self.current_track().cloned() {
            self.mpris.update(
                &track.title,
                &track.artist,
                track.artwork_url.as_deref(),
                track.duration,
                self.is_paused,
            );
        }
    }

    pub fn adjust_volume(&mut self, delta: i32) {
        let new_vol = (self.volume as i32 + delta).clamp(0, 150) as u32;
        self.volume = new_vol;
        self.player.set_volume(new_vol as f64);
        self.set_status(&format!("Volume: {}%", new_vol));
    }

    pub fn seek(&mut self, seconds: f64) {
        self.player.seek(seconds);
        self.current_pos = (self.current_pos + seconds).max(0.0);
    }

    pub fn set_speed(&mut self, speed: f64) {
        let clamped = ((speed * 100.0).round() / 100.0).clamp(0.25, 3.0);
        self.current_speed = clamped;
        self.player.set_speed(clamped);
        let cur_id = self.current_track().map(|t| t.id.clone());
        if let Some(id) = cur_id {
            self.store.save_track_state(&id, self.current_pos, self.current_speed);
        }
        if (clamped - 1.0).abs() < 0.001 {
            self.set_status("Speed: 1.00x (Normal)");
        } else if clamped < 1.0 {
            self.set_status(&format!("Speed: {:.2}x (Slowed)", clamped));
        } else {
            self.set_status(&format!("Speed: {:.2}x (Speed Up)", clamped));
        }
    }

    pub fn speed_down(&mut self) {
        // If normal 1.0x, drop straight to 0.85x as requested; otherwise step down by 0.05
        if (self.current_speed - 1.0).abs() < 0.01 {
            self.set_speed(0.85);
        } else {
            self.set_speed(self.current_speed - 0.05);
        }
    }

    pub fn speed_up(&mut self) {
        // If normal 1.0x, jump straight to 1.25x as requested; otherwise step up by 0.05
        if (self.current_speed - 1.0).abs() < 0.01 {
            self.set_speed(1.25);
        } else {
            self.set_speed(self.current_speed + 0.05);
        }
    }

    pub fn reset_speed(&mut self) {
        self.set_speed(1.0);
    }

    pub fn save_current_state(&mut self) {
        let cur_id = self.current_track().map(|t| t.id.clone());
        if let Some(id) = cur_id {
            if self.current_pos > 1.0 {
                self.store.save_track_state(&id, self.current_pos, self.current_speed);
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Global media keys with ctrl or alt
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Right => {
                    self.seek(10.0);
                    return;
                }
                KeyCode::Left => {
                    self.seek(-10.0);
                    return;
                }
                KeyCode::Up => {
                    self.adjust_volume(5);
                    return;
                }
                KeyCode::Down => {
                    self.adjust_volume(-5);
                    return;
                }
                _ => {}
            }
        }

        // If modal input is active (creating a playlist)
        if let Some(chars) = &mut self.modal_input {
            match key.code {
                KeyCode::Esc => {
                    self.modal_input = None;
                    self.modal_target_track = None;
                }
                KeyCode::Char(c) => {
                    chars.push(c);
                }
                KeyCode::Backspace => {
                    chars.pop();
                }
                KeyCode::Enter => {
                    let title: String = chars.iter().collect();
                    if !title.trim().is_empty() {
                        self.store.create_playlist(title.trim());
                        self.set_status(&format!("Created playlist: {}", title.trim()));
                    }
                    self.modal_input = None;
                }
                _ => {}
            }
            return;
        }

        // Modal for selecting a playlist to add a track
        if self.modal_target_track.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.modal_target_track = None;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.modal_playlist_cursor > 0 {
                        self.modal_playlist_cursor -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.store.playlists.is_empty() && self.modal_playlist_cursor + 1 < self.store.playlists.len() {
                        self.modal_playlist_cursor += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(track) = self.modal_target_track.take() {
                        if !self.store.playlists.is_empty() && self.modal_playlist_cursor < self.store.playlists.len() {
                            let pl_title = self.store.playlists[self.modal_playlist_cursor].title.clone();
                            if self.store.add_track_to_playlist(self.modal_playlist_cursor, track) {
                                self.set_status(&format!("Added to '{}'", pl_title));
                            } else {
                                self.set_status(&format!("Track already in '{}'", pl_title));
                            }
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        // Functional view switches: F1-F6
        match key.code {
            KeyCode::F(1) => {
                self.view_mode = ViewMode::Search;
                self.is_editing_search = true;
                return;
            }
            KeyCode::F(2) => {
                self.view_mode = ViewMode::Queue;
                self.is_editing_search = false;
                return;
            }
            KeyCode::F(3) => {
                self.view_mode = ViewMode::Playlists;
                self.is_editing_search = false;
                return;
            }
            KeyCode::F(4) => {
                self.view_mode = ViewMode::Favorites;
                self.is_editing_search = false;
                return;
            }
            KeyCode::F(5) => {
                self.view_mode = ViewMode::History;
                self.is_editing_search = false;
                return;
            }
            KeyCode::F(6) => {
                self.view_mode = ViewMode::Settings;
                self.is_editing_search = false;
                return;
            }
            KeyCode::Tab => {
                // Tab cycles views
                self.view_mode = match self.view_mode {
                    ViewMode::Search => ViewMode::Queue,
                    ViewMode::Queue => ViewMode::Playlists,
                    ViewMode::Playlists => ViewMode::Favorites,
                    ViewMode::PlaylistDetail => ViewMode::Playlists,
                    ViewMode::Favorites => ViewMode::History,
                    ViewMode::History => ViewMode::Lyrics,
                    ViewMode::Lyrics => ViewMode::Settings,
                    ViewMode::UserProfile => ViewMode::Search,
                    ViewMode::Settings => ViewMode::Search,
                };
                if self.view_mode != ViewMode::Search {
                    self.is_editing_search = false;
                }
                return;
            }
            _ => {}
        }

        // Search edit mode vs navigation mode
        if self.view_mode == ViewMode::Search && self.is_editing_search {
            match key.code {
                KeyCode::Esc => {
                    self.is_editing_search = false;
                    self.set_status("Navigation mode (Space to Play/Pause, Enter to Play)");
                    return;
                }
                KeyCode::Char(' ') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    self.toggle_pause();
                    return;
                }
                KeyCode::Char('p') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    self.toggle_pause();
                    return;
                }
                KeyCode::Char('[') | KeyCode::Char('<') | KeyCode::Char(',')
                    if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
                {
                    self.speed_down();
                    return;
                }
                KeyCode::Char(']') | KeyCode::Char('>') | KeyCode::Char('.')
                    if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
                {
                    self.speed_up();
                    return;
                }
                KeyCode::Char('0') | KeyCode::Char('\\')
                    if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
                {
                    self.reset_speed();
                    return;
                }
                KeyCode::Char(c) => {
                    self.query_chars.insert(self.input_cursor, c);
                    self.input_cursor += 1;
                }
                KeyCode::Backspace => {
                    if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                        self.query_chars.remove(self.input_cursor);
                    }
                }
                KeyCode::Left => {
                    if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.input_cursor < self.query_chars.len() {
                        self.input_cursor += 1;
                    }
                }
                KeyCode::Enter => {
                    let query: String = self.query_chars.iter().collect();
                    if !query.trim().is_empty() {
                        self.is_searching = true;
                        self.set_status(&format!("Searching for '{}'...", query.trim()));
                        self.search_engine.search(&query);
                        self.is_editing_search = false;
                    }
                }
                KeyCode::Down => {
                    self.is_editing_search = false;
                }
                _ => {}
            }
            return;
        }

        // Navigation mode across screens
        match key.code {
            KeyCode::Char(' ') => {
                if self.current_track().is_some() {
                    self.toggle_pause();
                } else {
                    self.play_selected_in_current_view();
                }
            }
            KeyCode::Char('/') | KeyCode::Char('?') => {
                self.view_mode = ViewMode::Search;
                self.is_editing_search = true;
                self.set_status("Search mode (type query and press Enter)");
            }
            KeyCode::Char('<')
            | KeyCode::Char('[')
            | KeyCode::Char('б')
            | KeyCode::Char('Б')
            | KeyCode::Char('х')
            | KeyCode::Char('Х') => {
                self.speed_down();
            }
            KeyCode::Char(',') => {
                self.speed_down();
            }
            KeyCode::Char('>')
            | KeyCode::Char(']')
            | KeyCode::Char('ю')
            | KeyCode::Char('Ю')
            | KeyCode::Char('ъ')
            | KeyCode::Char('Ъ') => {
                self.speed_up();
            }
            KeyCode::Char('.') => {
                self.speed_up();
            }
            KeyCode::Backspace => {
                if self.view_mode == ViewMode::Search {
                    self.is_editing_search = true;
                    if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                        self.query_chars.remove(self.input_cursor);
                    }
                    self.set_status("Search mode (type query and press Enter)");
                } else {
                    self.reset_speed();
                }
            }
            KeyCode::Char('\\') | KeyCode::Char('0') => {
                self.reset_speed();
            }
            KeyCode::Char('n') => {
                self.play_next();
            }
            KeyCode::Char('p') => {
                self.play_prev();
            }
            KeyCode::Char('y') => {
                // Toggle Lyrics view
                self.view_mode = if self.view_mode == ViewMode::Lyrics {
                    ViewMode::Queue
                } else {
                    ViewMode::Lyrics
                };
                self.is_editing_search = false;
            }
            KeyCode::Char('H') => {
                // Jump to History view
                self.view_mode = ViewMode::History;
                self.is_editing_search = false;
                self.set_status("Switched to History");
            }
            KeyCode::Char('F') => {
                // Jump to Favorites view
                self.view_mode = ViewMode::Favorites;
                self.is_editing_search = false;
                self.set_status("Switched to Favorites");
            }
            KeyCode::Char('Q') => {
                // Jump to Queue view
                self.view_mode = ViewMode::Queue;
                self.is_editing_search = false;
                self.set_status("Switched to Queue");
            }
            KeyCode::Char('1') => {
                self.view_mode = ViewMode::Search;
                self.is_editing_search = true;
            }
            KeyCode::Char('2') => {
                self.view_mode = ViewMode::Queue;
                self.is_editing_search = false;
            }
            KeyCode::Char('3') => {
                self.view_mode = ViewMode::Playlists;
                self.is_editing_search = false;
            }
            KeyCode::Char('4') => {
                self.view_mode = ViewMode::Favorites;
                self.is_editing_search = false;
            }
            KeyCode::Char('5') => {
                self.view_mode = ViewMode::History;
                self.is_editing_search = false;
            }
            KeyCode::Char('6') => {
                self.view_mode = ViewMode::Lyrics;
                self.is_editing_search = false;
            }
            KeyCode::Char('7') => {
                self.view_mode = ViewMode::Settings;
                self.is_editing_search = false;
            }
            KeyCode::Char('S') => {
                // Sync library if oauth_token is set
                if let Some(token) = self.store.config.oauth_token.clone() {
                    self.set_status("Syncing your SoundCloud library...");
                    self.search_engine.sync_library(&token);
                } else {
                    self.set_status("Set 'oauth_token' in ~/.config/soundcloud-tui/config.json first");
                }
            }
            KeyCode::Char('s') => {
                self.shuffle = !self.shuffle;
                self.set_status(&format!("Shuffle: {}", if self.shuffle { "ON" } else { "OFF" }));
            }
            KeyCode::Char('r') => {
                self.repeat = match self.repeat {
                    RepeatMode::Off => RepeatMode::All,
                    RepeatMode::All => RepeatMode::One,
                    RepeatMode::One => RepeatMode::Off,
                };
                self.set_status(&format!("Repeat: {}", self.repeat.name().to_uppercase()));
            }
            KeyCode::Char('u') => {
                // View profile of artist for current/highlighted track
                let track = match self.view_mode {
                    ViewMode::Search => self.search_results.get(self.search_cursor).cloned(),
                    ViewMode::Queue => self.queue.get(self.queue_cursor).cloned(),
                    ViewMode::Favorites => self.store.liked.get(self.favorites_cursor).cloned(),
                    ViewMode::History => self.store.history.get(self.history_cursor).map(|h| h.track.clone()),
                    ViewMode::PlaylistDetail => {
                        self.selected_playlist_idx.and_then(|i| self.store.playlists.get(i)).and_then(|p| p.tracks.get(self.playlist_detail_cursor).cloned())
                    }
                    _ => self.current_track().cloned(),
                };
                if let Some(t) = track {
                    let query = t.user_permalink.as_deref().or(t.user_id.as_deref()).unwrap_or(&t.artist);
                    self.set_status(&format!("Loading profile of '{}'...", t.artist));
                    self.search_engine.fetch_user(query);
                }
            }
            KeyCode::Char('c') => {
                // Create local playlist dialog
                if self.view_mode == ViewMode::Playlists {
                    self.modal_input = Some(Vec::new());
                }
            }
            KeyCode::Char('m') => {
                // Add highlighted track to one of saved playlists
                let track = match self.view_mode {
                    ViewMode::Search => self.search_results.get(self.search_cursor).cloned(),
                    ViewMode::Queue => self.queue.get(self.queue_cursor).cloned(),
                    ViewMode::Favorites => self.store.liked.get(self.favorites_cursor).cloned(),
                    ViewMode::History => self.store.history.get(self.history_cursor).map(|h| h.track.clone()),
                    ViewMode::UserProfile => {
                        self.current_user_profile.as_ref().and_then(|p| p.tracks.get(self.user_profile_cursor).cloned())
                    }
                    _ => self.current_track().cloned(),
                };
                if let Some(t) = track {
                    if self.store.playlists.is_empty() {
                        self.set_status("No playlists yet. Press 'c' in Playlists view to create one");
                    } else {
                        self.modal_target_track = Some(t);
                        self.modal_playlist_cursor = 0;
                    }
                }
            }
            KeyCode::Char('l') => {
                // Like / unlike current track
                if let Some(track) = self.current_track().cloned() {
                    let liked = self.store.toggle_like(&track);
                    if liked {
                        self.set_status(&format!("Added to Favorites: {}", track.title));
                    } else {
                        self.set_status(&format!("Removed from Favorites: {}", track.title));
                    }
                }
            }
            KeyCode::Char('o') | KeyCode::Char('щ') | KeyCode::Char('Щ') => {
                self.copy_track_link();
            }
            KeyCode::Char('w') | KeyCode::Char('ц') | KeyCode::Char('Ц') => {
                if let Some(track) = self.selected_or_current_track() {
                    self.pending_auto_wave_play = false;
                    self.related_fetched_for_id = Some(track.id.clone());
                    self.set_status(&format!("Loading related tracks wave for '{}'...", track.title));
                    self.search_engine.fetch_related(&track.id);
                }
            }
            KeyCode::Char('i') | KeyCode::Char('ш') | KeyCode::Char('Ш') => {
                self.view_mode = ViewMode::Search;
                self.is_editing_search = true;
                self.set_status("Search mode (type query and press Enter)");
            }
            KeyCode::Up | KeyCode::Char('k') => match self.view_mode {
                ViewMode::Search => {
                    if self.search_cursor > 0 {
                        self.search_cursor -= 1;
                    }
                }
                ViewMode::Queue => {
                    if self.queue_cursor > 0 {
                        self.queue_cursor -= 1;
                    }
                }
                ViewMode::Playlists => {
                    if self.playlists_cursor > 0 {
                        self.playlists_cursor -= 1;
                    }
                }
                ViewMode::PlaylistDetail => {
                    if self.playlist_detail_cursor > 0 {
                        self.playlist_detail_cursor -= 1;
                    }
                }
                ViewMode::UserProfile => {
                    if self.user_profile_cursor > 0 {
                        self.user_profile_cursor -= 1;
                    }
                }
                ViewMode::Favorites => {
                    if self.favorites_cursor > 0 {
                        self.favorites_cursor -= 1;
                    }
                }
                ViewMode::History => {
                    if self.history_cursor > 0 {
                        self.history_cursor -= 1;
                    }
                }
                ViewMode::Settings => {
                    if self.settings_cursor > 0 {
                        self.settings_cursor -= 1;
                    }
                }
                ViewMode::Lyrics => {
                    self.seek(-(self.store.config.seek_step as f64));
                }
            },
            KeyCode::Down | KeyCode::Char('j') => match self.view_mode {
                ViewMode::Search => {
                    if !self.search_results.is_empty()
                        && self.search_cursor + 1 < self.search_results.len()
                    {
                        self.search_cursor += 1;
                    }
                }
                ViewMode::Queue => {
                    if !self.queue.is_empty() && self.queue_cursor + 1 < self.queue.len() {
                        self.queue_cursor += 1;
                    }
                }
                ViewMode::Playlists => {
                    if !self.store.playlists.is_empty()
                        && self.playlists_cursor + 1 < self.store.playlists.len()
                    {
                        self.playlists_cursor += 1;
                    }
                }
                ViewMode::PlaylistDetail => {
                    if let Some(idx) = self.selected_playlist_idx {
                        if let Some(pl) = self.store.playlists.get(idx) {
                            if !pl.tracks.is_empty() && self.playlist_detail_cursor + 1 < pl.tracks.len() {
                                self.playlist_detail_cursor += 1;
                            }
                        }
                    }
                }
                ViewMode::UserProfile => {
                    if let Some(p) = &self.current_user_profile {
                        if !p.tracks.is_empty() && self.user_profile_cursor + 1 < p.tracks.len() {
                            self.user_profile_cursor += 1;
                        }
                    }
                }
                ViewMode::Favorites => {
                    if !self.store.liked.is_empty()
                        && self.favorites_cursor + 1 < self.store.liked.len()
                    {
                        self.favorites_cursor += 1;
                    }
                }
                ViewMode::History => {
                    if !self.store.history.is_empty()
                        && self.history_cursor + 1 < self.store.history.len()
                    {
                        self.history_cursor += 1;
                    }
                }
                ViewMode::Settings => {
                    if self.settings_cursor < 11 {
                        self.settings_cursor += 1;
                    }
                }
                ViewMode::Lyrics => {
                    self.seek(self.store.config.seek_step as f64);
                }
            },
            KeyCode::Enter => {
                self.play_selected_in_current_view();
            },
            KeyCode::Char('a') => {
                // Add to queue
                let track = match self.view_mode {
                    ViewMode::Search => self.search_results.get(self.search_cursor).cloned(),
                    ViewMode::Favorites => self.store.liked.get(self.favorites_cursor).cloned(),
                    ViewMode::History => {
                        self.store.history.get(self.history_cursor).map(|h| h.track.clone())
                    }
                    ViewMode::UserProfile => {
                        self.current_user_profile.as_ref().and_then(|p| p.tracks.get(self.user_profile_cursor).cloned())
                    }
                    ViewMode::PlaylistDetail => {
                        if let Some(idx) = self.selected_playlist_idx {
                            self.store.playlists.get(idx).and_then(|p| p.tracks.get(self.playlist_detail_cursor).cloned())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(t) = track {
                    self.set_status(&format!("Added to Queue: {}", t.title));
                    self.queue.push(t);
                }
            }
            KeyCode::Char('d') => {
                // Remove from queue or favorites or playlists
                if self.view_mode == ViewMode::Queue && !self.queue.is_empty() {
                    let removed = self.queue.remove(self.queue_cursor);
                    self.set_status(&format!("Removed from Queue: {}", removed.title));
                    if self.queue_cursor >= self.queue.len() && self.queue_cursor > 0 {
                        self.queue_cursor -= 1;
                    }
                } else if self.view_mode == ViewMode::Favorites && !self.store.liked.is_empty() {
                    let removed = self.store.liked.remove(self.favorites_cursor);
                    self.store.save();
                    self.set_status(&format!("Removed from Favorites: {}", removed.title));
                    if self.favorites_cursor >= self.store.liked.len() && self.favorites_cursor > 0 {
                        self.favorites_cursor -= 1;
                    }
                } else if self.view_mode == ViewMode::Playlists && !self.store.playlists.is_empty() {
                    if let Some(removed) = self.store.remove_playlist(self.playlists_cursor) {
                        self.set_status(&format!("Removed Playlist: {}", removed.title));
                        if self.playlists_cursor >= self.store.playlists.len() && self.playlists_cursor > 0 {
                            self.playlists_cursor -= 1;
                        }
                    }
                } else if self.view_mode == ViewMode::PlaylistDetail {
                    if let Some(pl_idx) = self.selected_playlist_idx {
                        if let Some(removed) = self.store.remove_track_from_playlist(pl_idx, self.playlist_detail_cursor) {
                            self.set_status(&format!("Removed track: {}", removed.title));
                            let track_count = self.store.playlists.get(pl_idx).map(|p| p.tracks.len()).unwrap_or(0);
                            if self.playlist_detail_cursor >= track_count && self.playlist_detail_cursor > 0 {
                                self.playlist_detail_cursor -= 1;
                            }
                        }
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('b') => {
                if self.view_mode == ViewMode::Settings {
                    self.cycle_setting();
                } else {
                    self.seek(-(self.store.config.seek_step as f64));
                }
            }
            KeyCode::Right | KeyCode::Char('f') => {
                if self.view_mode == ViewMode::Settings {
                    self.cycle_setting();
                } else {
                    self.seek(self.store.config.seek_step as f64);
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.adjust_volume(5);
            }
            KeyCode::Char('-') => {
                self.adjust_volume(-5);
            }
            _ => {}
        }
    }

    fn cycle_setting(&mut self) {
        match self.settings_cursor {
            0 => {
                self.store.config.theme = match self.store.config.theme {
                    ThemeColor::Auto => ThemeColor::Catppuccin,
                    ThemeColor::Catppuccin => ThemeColor::TokyoNight,
                    ThemeColor::TokyoNight => ThemeColor::Nord,
                    ThemeColor::Nord => ThemeColor::Gruvbox,
                    ThemeColor::Gruvbox => ThemeColor::Dracula,
                    ThemeColor::Dracula => ThemeColor::Amber,
                    ThemeColor::Amber => ThemeColor::White,
                    ThemeColor::White => ThemeColor::Auto,
                };
            }
            1 => {
                self.store.config.visual = match self.store.config.visual {
                    VisualStyle::Waveform => VisualStyle::Tape,
                    VisualStyle::Tape => VisualStyle::Blocky,
                    VisualStyle::Blocky => VisualStyle::Braille,
                    VisualStyle::Braille => VisualStyle::Minimal,
                    VisualStyle::Minimal => VisualStyle::Waveform,
                };
            }
            2 => {
                self.store.config.show_bottom_hints = !self.store.config.show_bottom_hints;
            }
            3 => {
                self.volume = if self.volume >= 120 { 20 } else { self.volume + 10 };
                self.store.config.default_volume = self.volume;
                self.player.set_volume(self.volume as f64);
            }
            4 => {
                self.store.config.pitch_mode = match self.store.config.pitch_mode {
                    PitchMode::Natural => PitchMode::Keep,
                    PitchMode::Keep => PitchMode::Natural,
                };
                self.player
                    .set_pitch_correction(self.store.config.pitch_mode == PitchMode::Keep);
            }
            5 => {
                self.store.config.auto_wave = !self.store.config.auto_wave;
            }
            6 => {
                self.store.config.resume_playback = !self.store.config.resume_playback;
            }
            7 => {
                self.store.config.seek_step = match self.store.config.seek_step {
                    2 => 5,
                    5 => 10,
                    10 => 15,
                    15 => 30,
                    _ => 2,
                };
            }
            8 => {
                self.store.config.discord_rpc = !self.store.config.discord_rpc;
                if !self.store.config.discord_rpc {
                    self.discord.clear();
                } else {
                    self.last_rpc_update = None;
                }
            }
            9 => {
                self.store.config.discord_buttons = !self.store.config.discord_buttons;
                self.last_rpc_update = None;
            }
            10 => {
                self.store.config.gapless_prefetch = !self.store.config.gapless_prefetch;
            }
            11 => {
                let count = DataStore::clear_audio_cache();
                self.set_status(&format!("Cleared {} files from local audio cache", count));
            }
            _ => {}
        }
        self.store.save();
    }

    pub fn poll_async_events(&mut self) {
        // Asynchronous stream resolution events
        while let Ok(res) = self.stream_resolve_rx.try_recv() {
            match res {
                StreamResolveEvent::Resolved { track_id, url, source } => {
                    // Only apply if this is still the active track
                    if self.current_track().map(|t| t.id == track_id).unwrap_or(false) {
                        self.player.play(&url);
                        if self.current_speed != 1.0 {
                            self.player.set_speed(self.current_speed);
                        }
                        if self.is_paused {
                            self.player.set_pause(true);
                        }
                        let title = self.current_track().map(|t| t.title.clone()).unwrap_or_default();
                        if let Some(pos) = self.pending_resume_pos.take() {
                            self.player.seek_to(pos);
                            self.set_status(&format!("Resumed at {}: {}", crate::ui::format_duration(pos), title));
                        } else {
                            let label = if source == "direct" { "Streaming" } else { "Streaming (ytdl)" };
                            self.set_status(&format!("{}: {}", label, title));
                        }
                    }
                }
                StreamResolveEvent::Failed { track_id, title } => {
                    if self.current_track().map(|t| t.id == track_id).unwrap_or(false) {
                        self.set_status(&format!("Track unavailable: {}", title));
                    }
                }
            }
        }

        // Audio events
        while let Some(ev) = self.player.poll_event() {
            match ev {
                crate::audio::PlayerEvent::FileLoaded => {
                    if let Some(pos) = self.pending_resume_pos.take() {
                        self.player.seek_to(pos);
                    }
                    if self.current_speed != 1.0 {
                        self.player.set_speed(self.current_speed);
                    }
                }
                crate::audio::PlayerEvent::TimePos(pos) => {
                    self.current_pos = pos;
                    let should_save = match self.last_saved_pos_instant {
                        Some(last) => last.elapsed() >= std::time::Duration::from_secs(3),
                        None => true,
                    };
                    if should_save {
                        self.last_saved_pos_instant = Some(std::time::Instant::now());
                        let cur_id = self.current_track().map(|t| t.id.clone());
                        if let Some(id) = cur_id {
                            self.store.save_track_state(&id, pos, self.current_speed);
                        }
                    }
                }
                crate::audio::PlayerEvent::Duration(dur) => self.total_duration = dur,
                crate::audio::PlayerEvent::Paused(p) => self.is_paused = p,
                crate::audio::PlayerEvent::Volume(v) => self.volume = v as u32,
                crate::audio::PlayerEvent::PlaybackEnded => {
                    let cur_id = self.current_track().map(|t| t.id.clone());
                    if let Some(id) = cur_id {
                        // Mark track as finished so next time it starts from 0:00
                        self.store.save_track_state(&id, 0.0, self.current_speed);
                    }
                    self.play_next();
                }
                crate::audio::PlayerEvent::Idle(idle) => {
                    if idle && self.current_track().is_some() && self.total_duration > 10.0 && self.current_pos >= (self.total_duration - 4.0).max(1.0) {
                        self.play_next();
                    }
                }
                crate::audio::PlayerEvent::PlaybackError(err) => {
                    if let Some(track) = self.current_track().cloned() {
                        if let Some(cached) = crate::storage::DataStore::get_cached_track_path(&track.id) {
                            let _ = std::fs::remove_file(cached);
                            self.play_track(track);
                            continue;
                        }
                    }
                    self.set_status(&format!("Playback error: {}", err));
                }
            }
        }

        // Gapless background prefetch: when current track reaches 70% duration, prefetch next track into disk cache
        if self.total_duration > 15.0 && (self.current_pos / self.total_duration) >= 0.70 {
            if let Some(cur_idx) = self.current_track_idx {
                if !self.queue.is_empty() {
                    if cur_idx + 1 < self.queue.len() {
                        if self.store.config.gapless_prefetch {
                            let next_idx = cur_idx + 1;
                            if let Some(next_track) = self.queue.get(next_idx) {
                                if self.prefetched_track_id.as_deref() != Some(&next_track.id) {
                                    self.prefetched_track_id = Some(next_track.id.clone());
                                    SearchEngine::prefetch_track(next_track, &self.search_engine.cached_client_id);
                                }
                            }
                        }
                    } else if self.repeat == RepeatMode::Off && !self.shuffle && self.store.config.auto_wave {
                        // Auto-Wave: prefetch related tracks when on the last track of queue at >= 80%
                        if (self.current_pos / self.total_duration) >= 0.80 {
                            if let Some(cur_track) = self.queue.get(cur_idx) {
                                if self.related_fetched_for_id.as_deref() != Some(&cur_track.id) {
                                    self.related_fetched_for_id = Some(cur_track.id.clone());
                                    self.search_engine.fetch_related(&cur_track.id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Load waveform bytes from disk if available and not yet loaded into RAM
        if let Some(track) = self.current_track() {
            if self.current_waveform.is_empty() {
                let wf_file = crate::storage::DataStore::cache_dir().join(format!("{}.wave", track.id));
                if wf_file.exists() {
                    if let Ok(bytes) = std::fs::read(wf_file) {
                        self.current_waveform = bytes;
                    }
                }
            }
        }

        // Discord Rich Presence update (every 1s)
        if self.store.config.discord_rpc {
            let should_update = match self.last_rpc_update {
                Some(last) => last.elapsed() >= std::time::Duration::from_secs(1),
                None => true,
            };

            if should_update {
                self.last_rpc_update = Some(std::time::Instant::now());
                if let Some(track) = self.current_track() {
                    let btn_url = if self.store.config.discord_buttons {
                        Some(track.url.clone())
                    } else {
                        None
                    };
                    self.discord.update(
                        &track.title,
                        &track.artist,
                        btn_url,
                        track.artwork_url.clone(),
                        self.current_pos,
                        self.total_duration,
                        self.is_paused,
                    );
                } else {
                    self.discord.clear();
                }
            }
        }

        // MPRIS events from Caelestia Shell and system media keys
        while let Some(mpris_ev) = self.mpris.poll_event() {
            match mpris_ev {
                crate::mpris::MprisEvent::Play => {
                    if self.is_paused {
                        self.toggle_pause();
                    }
                }
                crate::mpris::MprisEvent::Pause => {
                    if !self.is_paused {
                        self.toggle_pause();
                    }
                }
                crate::mpris::MprisEvent::Toggle => {
                    self.toggle_pause();
                }
                crate::mpris::MprisEvent::Next => {
                    self.play_next();
                }
                crate::mpris::MprisEvent::Previous => {
                    self.play_prev();
                }
            }
        }

        // Lyrics reception
        while let Some(lyrics) = self.lyrics_manager.poll_lyrics() {
            if let Some(track) = self.current_track() {
                if track.id == lyrics.track_id {
                    self.current_lyrics = Some(lyrics);
                }
            }
        }

        // Search events
        if let Some(res) = self.search_engine.poll_result() {
            self.is_searching = false;
            match res {
                SearchResult::Success(tracks) => {
                    self.set_status(&format!("Found {} tracks — press Enter to play", tracks.len()));
                    self.search_results = tracks;
                    self.search_cursor = 0;
                }
                SearchResult::PlaylistLoaded(playlist) => {
                    self.set_status(&format!("Playlist loaded: {} ({} tracks)", playlist.title, playlist.track_count));
                    self.store.save_playlist(playlist);
                    self.selected_playlist_idx = Some(0);
                    self.playlist_detail_cursor = 0;
                    self.view_mode = ViewMode::PlaylistDetail;
                }
                SearchResult::UserProfileLoaded(profile) => {
                    self.set_status(&format!("Loaded profile: @{} ({} tracks)", profile.username, profile.track_count));
                    self.current_user_profile = Some(profile);
                    self.user_profile_cursor = 0;
                    self.view_mode = ViewMode::UserProfile;
                }
                SearchResult::LibraryLoaded { likes, playlists } => {
                    let likes_len = likes.len();
                    let pl_len = playlists.len();
                    for t in likes {
                        if !self.store.is_liked(&t.url) {
                            self.store.liked.push(t);
                        }
                    }
                    for pl in playlists {
                        self.store.save_playlist(pl);
                    }
                    self.store.save();
                    self.set_status(&format!("Synced library: {} likes, {} playlists", likes_len, pl_len));
                }
                SearchResult::RelatedLoaded(tracks) => {
                    let first_new_idx = self.queue.len();
                    let mut added = 0usize;
                    for t in tracks {
                        if !self.queue.iter().any(|existing| existing.url == t.url || existing.id == t.id) {
                            self.queue.push(t);
                            added += 1;
                        }
                    }
                    if self.pending_auto_wave_play {
                        self.pending_auto_wave_play = false;
                        if first_new_idx < self.queue.len() {
                            let track = self.queue[first_new_idx].clone();
                            self.current_track_idx = Some(first_new_idx);
                            self.play_track(track);
                        } else if !self.queue.is_empty() {
                            let track = self.queue[0].clone();
                            self.current_track_idx = Some(0);
                            self.play_track(track);
                        }
                    } else if added > 0 {
                        self.set_status(&format!("Wave: added {} related tracks to queue", added));
                    } else {
                        self.set_status("Wave: no new related tracks found");
                    }
                }
                SearchResult::Error(err) => {
                    self.set_status(&format!("Error: {}", err));
                }
            }
        }
    }

    pub fn selected_or_current_track(&self) -> Option<TrackItem> {
        let from_view = match self.view_mode {
            ViewMode::Search => self.search_results.get(self.search_cursor).cloned(),
            ViewMode::Queue => self.queue.get(self.queue_cursor).cloned(),
            ViewMode::Favorites => self.store.liked.get(self.favorites_cursor).cloned(),
            ViewMode::History => self.store.history.get(self.history_cursor).map(|h| h.track.clone()),
            ViewMode::PlaylistDetail => self
                .selected_playlist_idx
                .and_then(|i| self.store.playlists.get(i))
                .and_then(|p| p.tracks.get(self.playlist_detail_cursor).cloned()),
            ViewMode::UserProfile => self
                .current_user_profile
                .as_ref()
                .and_then(|p| p.tracks.get(self.user_profile_cursor).cloned()),
            _ => None,
        };
        from_view.or_else(|| self.current_track().cloned())
    }

    pub fn copy_track_link(&mut self) {
        let Some(track) = self.selected_or_current_track() else {
            self.set_status("No track selected to copy link");
            return;
        };
        let clean_url = track
            .url
            .split('#')
            .next()
            .unwrap_or(&track.url)
            .split('?')
            .next()
            .unwrap_or(&track.url)
            .to_string();

        let copied = Self::write_to_clipboard(&clean_url);
        if copied {
            self.set_status(&format!("Copied link: {}", clean_url));
        } else {
            self.set_status(&format!("Link: {}", clean_url));
        }
    }

    fn write_to_clipboard(text: &str) -> bool {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let candidates: &[(&str, &[&str])] = &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["-ib"]),
            ("pbcopy", &[]),
        ];

        for (bin, args) in candidates {
            if let Ok(mut child) = Command::new(bin)
                .args(*args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    if stdin.write_all(text.as_bytes()).is_ok() {
                        drop(stdin);
                        if let Ok(status) = child.wait() {
                            if status.success() {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) {
        if area.width < 30 || area.height < 8 {
            return;
        }
        if self.modal_input.is_some() || self.modal_target_track.is_some() {
            return;
        }

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                if mouse.row >= area.height.saturating_sub(3) {
                    self.adjust_volume(5);
                } else {
                    self.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
                }
            }
            MouseEventKind::ScrollDown => {
                if mouse.row >= area.height.saturating_sub(3) {
                    self.adjust_volume(-5);
                } else {
                    self.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let row = mouse.row;
                let col = mouse.column as usize;
                let width = area.width as usize;

                // Row 0: Top status bar -> cycle views
                if row == 0 {
                    self.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
                    return;
                }

                // Player tape line 1 (track title / play-pause icon): toggle pause
                if row == area.height.saturating_sub(3) {
                    self.toggle_pause();
                    return;
                }

                // Player tape line 2 (waveform progress bar): seek to clicked position
                if row == area.height.saturating_sub(2) {
                    if self.total_duration > 0.0 {
                        let bar_width = width.saturating_sub(24).clamp(12, 70);
                        let total_line_width = bar_width + 12;
                        let start_x = width.saturating_sub(total_line_width) / 2;
                        let bar_start_x = start_x + 6;
                        if col >= bar_start_x && col < bar_start_x + bar_width {
                            let ratio = ((col - bar_start_x) as f64 / bar_width as f64).clamp(0.0, 1.0);
                            let target = ratio * self.total_duration;
                            self.player.seek_to(target);
                            self.current_pos = target;
                            self.set_status(&format!("Seek: {}", crate::ui::format_duration(target)));
                        }
                    }
                    return;
                }

                // Main content area: rows 2 .. area.height - 4
                let content_top = 2u16;
                let content_bottom = area.height.saturating_sub(4);
                if row < content_top || row >= content_bottom {
                    return;
                }

                match self.view_mode {
                    ViewMode::Search => {
                        if row == 2 {
                            self.is_editing_search = true;
                            return;
                        }
                        if row >= 4 {
                            self.is_editing_search = false;
                            let list_height = content_bottom.saturating_sub(4) as usize;
                            if list_height == 0 || self.search_results.is_empty() {
                                return;
                            }
                            let scroll_offset = if self.search_cursor >= list_height {
                                self.search_cursor - list_height + 1
                            } else {
                                0
                            };
                            let clicked_idx = scroll_offset + (row - 4) as usize;
                            if clicked_idx < self.search_results.len() {
                                self.search_cursor = clicked_idx;
                                self.play_selected_in_current_view();
                            }
                        }
                    }
                    ViewMode::Queue => {
                        let list_height = (content_bottom - content_top) as usize;
                        if list_height == 0 || self.queue.is_empty() {
                            return;
                        }
                        let scroll_offset = if self.queue_cursor >= list_height {
                            self.queue_cursor - list_height + 1
                        } else {
                            0
                        };
                        let clicked_idx = scroll_offset + (row - content_top) as usize;
                        if clicked_idx < self.queue.len() {
                            self.queue_cursor = clicked_idx;
                            self.play_selected_in_current_view();
                        }
                    }
                    ViewMode::Playlists => {
                        let list_height = (content_bottom - content_top) as usize;
                        if list_height == 0 || self.store.playlists.is_empty() {
                            return;
                        }
                        let scroll_offset = if self.playlists_cursor >= list_height {
                            self.playlists_cursor - list_height + 1
                        } else {
                            0
                        };
                        let clicked_idx = scroll_offset + (row - content_top) as usize;
                        if clicked_idx < self.store.playlists.len() {
                            self.playlists_cursor = clicked_idx;
                            self.play_selected_in_current_view();
                        }
                    }
                    ViewMode::PlaylistDetail => {
                        if row >= 4 {
                            let list_height = content_bottom.saturating_sub(4) as usize;
                            let Some(pl_idx) = self.selected_playlist_idx else { return };
                            let Some(pl_len) = self.store.playlists.get(pl_idx).map(|p| p.tracks.len()) else { return };
                            if list_height == 0 || pl_len == 0 {
                                return;
                            }
                            let scroll_offset = if self.playlist_detail_cursor >= list_height {
                                self.playlist_detail_cursor - list_height + 1
                            } else {
                                0
                            };
                            let clicked_idx = scroll_offset + (row - 4) as usize;
                            if clicked_idx < pl_len {
                                self.playlist_detail_cursor = clicked_idx;
                                self.play_selected_in_current_view();
                            }
                        }
                    }
                    ViewMode::UserProfile => {
                        if row >= 4 {
                            let list_height = content_bottom.saturating_sub(4) as usize;
                            let Some(tr_len) = self.current_user_profile.as_ref().map(|p| p.tracks.len()) else { return };
                            if list_height == 0 || tr_len == 0 {
                                return;
                            }
                            let scroll_offset = if self.user_profile_cursor >= list_height {
                                self.user_profile_cursor - list_height + 1
                            } else {
                                0
                            };
                            let clicked_idx = scroll_offset + (row - 4) as usize;
                            if clicked_idx < tr_len {
                                self.user_profile_cursor = clicked_idx;
                                self.play_selected_in_current_view();
                            }
                        }
                    }
                    ViewMode::Favorites => {
                        let list_height = (content_bottom - content_top) as usize;
                        if list_height == 0 || self.store.liked.is_empty() {
                            return;
                        }
                        let scroll_offset = if self.favorites_cursor >= list_height {
                            self.favorites_cursor - list_height + 1
                        } else {
                            0
                        };
                        let clicked_idx = scroll_offset + (row - content_top) as usize;
                        if clicked_idx < self.store.liked.len() {
                            self.favorites_cursor = clicked_idx;
                            self.play_selected_in_current_view();
                        }
                    }
                    ViewMode::History => {
                        let list_height = (content_bottom - content_top) as usize;
                        if list_height == 0 || self.store.history.is_empty() {
                            return;
                        }
                        let scroll_offset = if self.history_cursor >= list_height {
                            self.history_cursor - list_height + 1
                        } else {
                            0
                        };
                        let clicked_idx = scroll_offset + (row - content_top) as usize;
                        if clicked_idx < self.store.history.len() {
                            self.history_cursor = clicked_idx;
                            self.play_selected_in_current_view();
                        }
                    }
                    ViewMode::Settings => {
                        self.cycle_setting();
                    }
                    ViewMode::Lyrics => {
                        self.toggle_pause();
                    }
                }
            }
            _ => {}
        }
    }
}
