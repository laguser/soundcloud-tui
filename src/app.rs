use crate::audio::AudioPlayer;
use crate::search::{SearchEngine, SearchResult};
use crate::storage::{DataStore, ThemeColor, TrackItem, VisualStyle};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use std::time::Instant;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ViewMode {
    Search,
    Queue,
    Playlists,
    PlaylistDetail,
    Favorites,
    History,
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
            ViewMode::Settings => "settings",
        }
    }
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

    // Audio status
    pub player: AudioPlayer,
    pub search_engine: SearchEngine,
    pub store: DataStore,

    pub is_paused: bool,
    pub current_pos: f64,
    pub total_duration: f64,
    pub volume: u32,
    pub status_message: Option<(String, Instant)>,
    pub last_esc_press: Option<Instant>,
}

impl App {
    pub fn new() -> Self {
        let store = DataStore::load();
        let volume = store.config.default_volume;
        let player = AudioPlayer::new();
        player.set_volume(volume as f64);

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

            player,
            search_engine: SearchEngine::new(),
            store,

            is_paused: false,
            current_pos: 0.0,
            total_duration: 0.0,
            volume,
            status_message: Some(("Ready. Type to search SoundCloud".to_string(), Instant::now())),
            last_esc_press: None,
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
        // 1. Check local on-disk cache
        if let Some(cached_file) = crate::storage::DataStore::get_cached_track_path(&track.id) {
            let path_str = cached_file.to_string_lossy().to_string();
            self.set_status(&format!("Playing (cached): {}", track.title));
            self.player.play(&path_str);
        } else if let Some(resolved) = crate::search::SearchEngine::resolve_stream_url(&track, &self.search_engine.cached_client_id) {
            self.set_status(&format!("Streaming (fast): {}", track.title));
            self.player.play(&resolved);
        } else {
            self.set_status(&format!("Streaming: {}", track.title));
            let play_url = track.stream_url.clone().unwrap_or_else(|| track.url.clone());
            self.player.play(&play_url);
        }

        self.store.add_history(track.clone());

        // Put in queue if not present or select it
        if let Some(pos) = self.queue.iter().position(|t| t.url == track.url) {
            self.current_track_idx = Some(pos);
        } else {
            self.queue.push(track);
            self.current_track_idx = Some(self.queue.len() - 1);
        }
        self.is_paused = false;
        self.current_pos = 0.0;
        self.total_duration = 0.0;
    }

    pub fn play_next(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let next_idx = match self.current_track_idx {
            Some(i) if i + 1 < self.queue.len() => i + 1,
            _ => 0,
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
        self.player.toggle_pause();
        self.is_paused = !self.is_paused;
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
                    ViewMode::History => ViewMode::Settings,
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
                self.toggle_pause();
            }
            KeyCode::Char('n') => {
                self.play_next();
            }
            KeyCode::Char('p') => {
                self.play_prev();
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
            KeyCode::Char('i') | KeyCode::Char('/') => {
                if self.view_mode == ViewMode::Search {
                    self.is_editing_search = true;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => match self.view_mode {
                ViewMode::Search => {
                    if self.search_cursor > 0 {
                        self.search_cursor -= 1;
                    } else {
                        self.is_editing_search = true;
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
                    if self.settings_cursor < 2 {
                        self.settings_cursor += 1;
                    }
                }
            },
            KeyCode::Enter => match self.view_mode {
                ViewMode::Search => {
                    if let Some(track) = self.search_results.get(self.search_cursor).cloned() {
                        self.play_track(track);
                    }
                }
                ViewMode::Queue => {
                    if let Some(track) = self.queue.get(self.queue_cursor).cloned() {
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
                            if let Some(track) = pl.tracks.get(self.playlist_detail_cursor).cloned() {
                                self.play_track(track);
                            }
                        }
                    }
                }
                ViewMode::Favorites => {
                    if let Some(track) = self.store.liked.get(self.favorites_cursor).cloned() {
                        self.play_track(track);
                    }
                }
                ViewMode::History => {
                    if let Some(entry) = self.store.history.get(self.history_cursor).cloned() {
                        self.play_track(entry.track);
                    }
                }
                ViewMode::Settings => {
                    self.cycle_setting();
                }
            },
            KeyCode::Char('a') => {
                // Add to queue
                let track = match self.view_mode {
                    ViewMode::Search => self.search_results.get(self.search_cursor).cloned(),
                    ViewMode::Favorites => self.store.liked.get(self.favorites_cursor).cloned(),
                    ViewMode::History => {
                        self.store.history.get(self.history_cursor).map(|h| h.track.clone())
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
                    let removed = self.store.playlists.remove(self.playlists_cursor);
                    self.store.save();
                    self.set_status(&format!("Removed Playlist: {}", removed.title));
                    if self.playlists_cursor >= self.store.playlists.len() && self.playlists_cursor > 0 {
                        self.playlists_cursor -= 1;
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('b') => {
                self.seek(-5.0);
            }
            KeyCode::Right | KeyCode::Char('f') => {
                self.seek(5.0);
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
                    VisualStyle::Tape => VisualStyle::Minimal,
                    VisualStyle::Minimal => VisualStyle::Blocky,
                    VisualStyle::Blocky => VisualStyle::Tape,
                };
            }
            2 => {
                self.volume = if self.volume >= 100 { 50 } else { self.volume + 25 };
                self.store.config.default_volume = self.volume;
                self.player.set_volume(self.volume as f64);
            }
            _ => {}
        }
        self.store.save();
    }

    pub fn poll_async_events(&mut self) {
        // Audio events
        while let Some(ev) = self.player.poll_event() {
            match ev {
                crate::audio::PlayerEvent::TimePos(pos) => self.current_pos = pos,
                crate::audio::PlayerEvent::Duration(dur) => self.total_duration = dur,
                crate::audio::PlayerEvent::Paused(p) => self.is_paused = p,
                crate::audio::PlayerEvent::Volume(v) => self.volume = v as u32,
                crate::audio::PlayerEvent::PlaybackEnded => self.play_next(),
                _ => {}
            }
        }

        // Search events
        if let Some(res) = self.search_engine.poll_result() {
            self.is_searching = false;
            match res {
                SearchResult::Success(tracks) => {
                    self.set_status(&format!("Found {} tracks", tracks.len()));
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
                SearchResult::Error(err) => {
                    self.set_status(&format!("Error: {}", err));
                }
            }
        }
    }
}
