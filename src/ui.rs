use crate::app::{App, ViewMode};
use crate::storage::VisualStyle;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();
    if size.width < 30 || size.height < 8 {
        return;
    }

    // Ultra-clean ttype layout: no boxes, pure typography & conveyor lines
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top status bar
            Constraint::Length(1), // Blank separator
            Constraint::Fill(1),   // Main list content
            Constraint::Length(1), // Blank separator
            Constraint::Length(2), // Center conveyor player tape
            Constraint::Length(1), // Bottom hotkey line
        ])
        .split(size);

    if app.store.config.show_top_status {
        render_top_bar(f, app, chunks[0]);
    }

    match app.view_mode {
        ViewMode::Search => render_search_view(f, app, chunks[2]),
        ViewMode::Queue => render_queue_view(f, app, chunks[2]),
        ViewMode::Playlists => render_playlists_view(f, app, chunks[2]),
        ViewMode::PlaylistDetail => render_playlist_detail_view(f, app, chunks[2]),
        ViewMode::Favorites => render_favorites_view(f, app, chunks[2]),
        ViewMode::History => render_history_view(f, app, chunks[2]),
        ViewMode::UserProfile => render_user_profile_view(f, app, chunks[2]),
        ViewMode::Lyrics => render_lyrics_view(f, app, chunks[2]),
        ViewMode::Settings => render_settings_view(f, app, chunks[2]),
    }

    render_player_tape(f, app, chunks[4]);
    if app.store.config.show_bottom_hints {
        render_bottom_bar(f, app, chunks[5]);
    }

    // Modal overlays (Creating playlist or adding to playlist or artwork URL)
    if let Some(chars) = &app.modal_input {
        render_input_modal(f, app, chars, "New Playlist Name:");
    } else if let Some(chars) = &app.modal_import_input {
        render_input_modal(f, app, chars, "Spotify playlist/album link:");
    } else if let Some((_, chars)) = &app.modal_artwork_input {
        render_artwork_input_modal(f, app, chars);
    } else if let Some(target_track) = &app.modal_target_track {
        render_add_to_playlist_modal(f, app, target_track);
    }
}

pub fn format_duration(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    let m = s / 60;
    let rem_s = s % 60;
    format!("{}:{:02}", m, rem_s)
}

fn render_top_bar(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    let current_view = app.view_mode.name().to_lowercase();
    let history_count = app.store.history.len();
    let mut spans = vec![
        Span::styled(format!("  {} ", current_view), Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled("· ", Style::default().fg(Color::Rgb(50, 50, 50))),
        Span::styled(format!("vol {}% ", app.volume), Style::default().fg(Color::Rgb(110, 110, 110))),
        Span::styled("· ", Style::default().fg(Color::Rgb(50, 50, 50))),
    ];

    if app.view_mode == ViewMode::History {
        spans.push(Span::styled(format!("{} tracks ", history_count), Style::default().fg(accent)));
        spans.push(Span::styled("· ", Style::default().fg(Color::Rgb(50, 50, 50))));
    }

    if (app.current_speed - 1.0).abs() > 0.001 {
        let label = if app.current_speed < 1.0 {
            format!("{:.2}x slowed ", app.current_speed)
        } else {
            format!("{:.2}x sped up ", app.current_speed)
        };
        spans.push(Span::styled(label, Style::default().fg(accent).add_modifier(Modifier::BOLD)));
    } else {
        spans.push(Span::styled("1.00x ", Style::default().fg(Color::Rgb(110, 110, 110))));
    }

    if app.shuffle {
        spans.push(Span::styled("· ", Style::default().fg(Color::Rgb(50, 50, 50))));
        spans.push(Span::styled("shuffle ", Style::default().fg(accent)));
    }

    if app.repeat != crate::app::RepeatMode::Off {
        spans.push(Span::styled("· ", Style::default().fg(Color::Rgb(50, 50, 50))));
        spans.push(Span::styled(
            format!("rep:{} ", app.repeat.name().to_lowercase()),
            Style::default().fg(accent),
        ));
    }

    if let Some((msg, _)) = &app.status_message {
        spans.push(Span::styled("· ", Style::default().fg(Color::Rgb(50, 50, 50))));
        spans.push(Span::styled(
            msg.as_str(),
            Style::default().fg(Color::Rgb(120, 120, 120)),
        ));
    }

    let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Left);
    f.render_widget(p, area);
}

fn render_search_view(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();

    let search_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Clean borderless input line
            Constraint::Length(1), // Spacer
            Constraint::Fill(1),   // Results list
        ])
        .split(area);

    // Borderless minimal prompt: "  ⌕  query▍ "
    let prompt_icon = if app.is_editing_search { "⌕ " } else { "· " };
    let prompt_color = if app.is_editing_search { accent } else { Color::Rgb(90, 90, 90) };

    let mut input_spans = vec![
        Span::styled(format!("  {} ", prompt_icon), Style::default().fg(prompt_color)),
    ];

    if app.query_chars.is_empty() && !app.is_editing_search {
        input_spans.push(Span::styled(
            "press '/' or 'i' to search SoundCloud...",
            Style::default().fg(Color::Rgb(85, 85, 85)),
        ));
    } else if app.query_chars.is_empty() && app.is_editing_search {
        input_spans.push(Span::styled(
            " ",
            Style::default().bg(accent),
        ));
        input_spans.push(Span::styled(
            " type song or artist and press Enter...",
            Style::default().fg(Color::Rgb(75, 75, 75)),
        ));
    } else {
        let cursor_pos = app.input_cursor;
        for (idx, &ch) in app.query_chars.iter().enumerate() {
            if app.is_editing_search && idx == cursor_pos {
                input_spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Black).bg(accent).add_modifier(Modifier::BOLD),
                ));
            } else {
                input_spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::White)));
            }
        }
        if app.is_editing_search && cursor_pos == app.query_chars.len() {
            input_spans.push(Span::styled(
                " ",
                Style::default().bg(accent),
            ));
        }
        if !app.is_editing_search {
            input_spans.push(Span::styled(
                "   (/ or i to search · Enter to play)",
                Style::default().fg(Color::Rgb(75, 75, 75)),
            ));
        }
    }

    let input_p = Paragraph::new(Line::from(input_spans));
    f.render_widget(input_p, search_layout[0]);

    // Loading / Results
    if app.is_searching {
        let loading = Paragraph::new(Line::from(vec![
            Span::styled("⚡ ", Style::default().fg(accent)),
            Span::styled("searching tracks...", Style::default().fg(Color::Rgb(120, 120, 120))),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(loading, search_layout[2]);
        return;
    }

    if app.search_results.is_empty() {
        let empty_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "type any song or artist and hit enter",
                Style::default().fg(Color::Rgb(65, 65, 65)),
            )),
        ])
        .alignment(Alignment::Center);
        f.render_widget(empty_msg, search_layout[2]);
        return;
    }

    render_tracks_list(f, &app.search_results, app.search_cursor, accent, search_layout[2], app);
}

fn render_tracks_list(
    f: &mut Frame,
    tracks: &[crate::storage::TrackItem],
    cursor: usize,
    accent: Color,
    area: Rect,
    app: &App,
) {
    let max_lines = area.height as usize;
    if max_lines == 0 {
        return;
    }

    let scroll_offset = if cursor >= max_lines {
        cursor - max_lines + 1
    } else {
        0
    };

    let mut lines = Vec::new();

    for (idx, track) in tracks.iter().enumerate().skip(scroll_offset).take(max_lines) {
        let is_selected = idx == cursor;
        let is_playing = app
            .current_track()
            .map(|t| t.url == track.url)
            .unwrap_or(false);
        let is_liked = app.store.is_liked(&track.url);

        let cursor_glyph = if is_selected {
            match app.store.config.visual {
                VisualStyle::Blocky => "▊ ",
                VisualStyle::Minimal => "· ",
                VisualStyle::Braille => "⣿ ",
                VisualStyle::Neon => "◈ ",
                VisualStyle::Tape => "▸ ",
                VisualStyle::Waveform => "› ",
            }
        } else if is_playing {
            "▶ "
        } else {
            "  "
        };

        let like_icon = if !app.store.config.show_hearts {
            "  "
        } else if is_liked {
            "♥ "
        } else {
            "  "
        };

        let title_style = if is_selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else if is_playing {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(140, 140, 140))
        };

        let artist_style = if is_selected {
            Style::default().fg(accent)
        } else {
            Style::default().fg(Color::Rgb(70, 70, 70))
        };

        let dur = if app.store.config.show_durations && track.duration > 0.0 {
            format_duration(track.duration)
        } else {
            "".to_string()
        };

        lines.push(Line::from(vec![
            Span::styled(format!(" {}", cursor_glyph), Style::default().fg(if is_selected { accent } else { Color::Rgb(80, 80, 80) })),
            Span::styled(like_icon, Style::default().fg(if is_liked { Color::Rgb(220, 70, 70) } else { Color::Rgb(40, 40, 40) })),
            Span::styled(format!("{:<42}", truncate_str(&track.title, 40)), title_style),
            Span::styled(format!(" {:<24}", truncate_str(&track.artist, 22)), artist_style),
            Span::styled(format!("{:>7}", dur), Style::default().fg(Color::Rgb(65, 65, 65))),
        ]));
    }

    let p = Paragraph::new(lines);
    f.render_widget(p, area);
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}

fn render_queue_view(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    if app.queue.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("queue is empty", Style::default().fg(Color::Rgb(80, 80, 80)))),
            Line::from(Span::styled("press 'a' to enqueue tracks", Style::default().fg(Color::Rgb(50, 50, 50)))),
        ])
        .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    render_tracks_list(f, &app.queue, app.queue_cursor, accent, area, app);
}

fn render_playlists_view(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    if app.store.playlists.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("no saved playlists", Style::default().fg(Color::Rgb(80, 80, 80)))),
            Line::from(Span::styled("press 'c' to create, 'i' to import from Spotify, or paste any SoundCloud playlist link in search", Style::default().fg(Color::Rgb(50, 50, 50)))),
        ])
        .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top hints
            Constraint::Length(1), // Spacer
            Constraint::Fill(1),   // Playlists list
        ])
        .split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("  ≡ ", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(format!("Your Playlists ({})", app.store.playlists.len()), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("  •  'c' create  •  'i' import Spotify  •  'Shift+C' cover  •  'd' delete", Style::default().fg(Color::Rgb(90, 90, 90))),
    ]));
    f.render_widget(header, chunks[0]);

    let max_lines = chunks[2].height as usize;
    if max_lines == 0 {
        return;
    }

    let cursor = app.playlists_cursor;
    let scroll_offset = if cursor >= max_lines {
        cursor - max_lines + 1
    } else {
        0
    };

    let mut lines = Vec::new();
    for (idx, pl) in app.store.playlists.iter().enumerate().skip(scroll_offset).take(max_lines) {
        let is_selected = idx == cursor;
        let prefix = if is_selected { " → " } else { "   " };
        let title_style = if is_selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(150, 150, 150))
        };

        let cover_icon = if pl.artwork_url.is_some() {
            Span::styled("🖼  ", Style::default().fg(accent))
        } else {
            Span::styled("≡  ", Style::default().fg(Color::Rgb(80, 80, 80)))
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(if is_selected { accent } else { Color::Rgb(60, 60, 60) })),
            cover_icon,
            Span::styled(format!("{:<36}", truncate_str(&pl.title, 34)), title_style),
            Span::styled(format!(" {:<22}", truncate_str(&pl.author, 20)), Style::default().fg(Color::Rgb(80, 80, 80))),
            Span::styled(format!("{:>6} tracks", pl.track_count), Style::default().fg(Color::Rgb(65, 65, 65))),
        ]));
    }

    let p = Paragraph::new(lines);
    f.render_widget(p, chunks[2]);
}

fn render_playlist_detail_view(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    let playlist = match app.selected_playlist_idx.and_then(|i| app.store.playlists.get(i)) {
        Some(p) => p,
        None => {
            let empty = Paragraph::new("Playlist not found").alignment(Alignment::Center);
            f.render_widget(empty, area);
            return;
        }
    };

    let total_secs: f64 = playlist.tracks.iter().map(|t| t.duration).sum();
    let duration_str = format_duration(total_secs);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(14), // Banner with cover
            Constraint::Length(1), // Spacer
            Constraint::Fill(1),   // Tracks
        ])
        .split(area);

    // Beautiful header banner
    let art_status = if let Some(ref art) = playlist.artwork_url {
        { let _ = art; "cover set".to_string() }
    } else {
        "no cover (press 'Shift+C' to set)".to_string()
    };

    let banner_lines = vec![
        Line::from(vec![
            Span::styled("  ╭─ ", Style::default().fg(accent)),
            Span::styled(&playlist.title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  [{} tracks · {}]", playlist.track_count, duration_str), Style::default().fg(accent)),
        ]),
        Line::from(vec![
            Span::styled("  │  ", Style::default().fg(accent)),
            Span::styled(format!("Curated by: @{}", playlist.author), Style::default().fg(Color::Rgb(150, 150, 150))),
            Span::styled(format!("  •  {}", art_status), Style::default().fg(Color::Rgb(95, 95, 95))),
        ]),
        Line::from(vec![
            Span::styled("  │  ", Style::default().fg(accent)),
            Span::styled("Actions: ", Style::default().fg(Color::Rgb(110, 110, 110))),
            Span::styled("Enter ", Style::default().fg(Color::White)),
            Span::styled("play track  •  ", Style::default().fg(Color::Rgb(80, 80, 80))),
            Span::styled("Shift+C ", Style::default().fg(Color::White)),
            Span::styled("set/change cover image URL  •  ", Style::default().fg(Color::Rgb(80, 80, 80))),
            Span::styled("d ", Style::default().fg(Color::White)),
            Span::styled("remove track  •  ", Style::default().fg(Color::Rgb(80, 80, 80))),
            Span::styled("Esc ", Style::default().fg(Color::White)),
            Span::styled("back", Style::default().fg(Color::Rgb(80, 80, 80))),
        ]),
        Line::from(Span::styled("  ╰─────────────────────────────────────────────────────────────────────────────", Style::default().fg(Color::Rgb(45, 45, 45)))),
    ];
    let banner = Paragraph::new(banner_lines);
    if let Some(ref art) = playlist.artwork_url {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(2), Constraint::Length(28), Constraint::Length(2), Constraint::Fill(1)])
            .split(chunks[0]);
        render_cover(f, art, cols[1], accent);
        let text_area = Rect { y: cols[3].y + 4, height: cols[3].height.saturating_sub(4), ..cols[3] };
        f.render_widget(banner, text_area);
    } else {
        f.render_widget(banner, chunks[0]);
    }

    if playlist.tracks.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("This playlist is empty", Style::default().fg(Color::Rgb(80, 80, 80)))),
            Line::from(Span::styled("Press 'm' on any track in Search or Favorites to add it here", Style::default().fg(Color::Rgb(55, 55, 55)))),
        ]).alignment(Alignment::Center);
        f.render_widget(empty, chunks[2]);
        return;
    }

    render_tracks_list(f, &playlist.tracks, app.playlist_detail_cursor, accent, chunks[2], app);
}

fn render_favorites_view(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    if app.store.liked.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("no favorites", Style::default().fg(Color::Rgb(80, 80, 80)))),
            Line::from(Span::styled("press 'l' while playing to add", Style::default().fg(Color::Rgb(50, 50, 50)))),
        ])
        .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    render_tracks_list(f, &app.store.liked, app.favorites_cursor, accent, area, app);
}

fn render_history_view(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    if app.store.history.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("no history", Style::default().fg(Color::Rgb(80, 80, 80)))),
        ])
        .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    let tracks: Vec<_> = app.store.history.iter().map(|h| h.track.clone()).collect();
    render_tracks_list(f, &tracks, app.history_cursor, accent, area, app);
}

fn render_settings_view(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    if area.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Length(1), // Blank separator
            Constraint::Fill(1),   // List
        ])
        .split(area);

    let categories = [
        ("1. Appearance & UI", "theme · player style · bar width · top/bottom bars · icons", 7usize),
        ("2. Audio & Speed Engine", "volume · pitch mode · slowed/sped-up presets · DSP filter", 6usize),
        ("3. Playback & Radio", "auto-wave · resume pos · seek step · shuffle · repeat · mouse", 6usize),
        ("4. Discord RPC & System", "rich presence · activity type · button · timestamps · MPRIS", 6usize),
        ("5. Storage & Maintenance", "gapless disk prefetch · history limit · clear cache/history", 4usize),
    ];

    let Some(cat_idx) = app.selected_settings_category else {
        // Render top-level categories list (just like Playlists view)
        let header = Paragraph::new(Line::from(vec![
            Span::styled("  ⚙  ", Style::default().fg(accent)),
            Span::styled("settings categories", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("  (enter / → to open · config: ~/.config/sctui/config.json)", Style::default().fg(Color::Rgb(70, 70, 70))),
        ]));
        f.render_widget(header, chunks[0]);

        let mut lines = Vec::new();
        for (idx, (title, desc, count)) in categories.iter().enumerate() {
            let is_selected = idx == app.settings_category_cursor;
            let prefix = if is_selected { " → " } else { "   " };
            let title_style = if is_selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(150, 150, 150))
            };
            let desc_style = if is_selected {
                Style::default().fg(accent)
            } else {
                Style::default().fg(Color::Rgb(80, 80, 80))
            };

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(if is_selected { accent } else { Color::Rgb(60, 60, 60) })),
                Span::styled("≡ ", Style::default().fg(accent)),
                Span::styled(format!("{:<26}", title), title_style),
                Span::styled(format!(" {:<58}", desc), desc_style),
                Span::styled(format!("{:>2} options", count), Style::default().fg(Color::Rgb(65, 65, 65))),
            ]));
        }

        let p = Paragraph::new(lines);
        f.render_widget(p, chunks[2]);
        return;
    };

    // Inside a specific category
    let (cat_title, _, _) = categories.get(cat_idx).copied().unwrap_or(("Settings", "", 0));
    let header = Paragraph::new(Line::from(vec![
        Span::styled("  ⚙  ", Style::default().fg(accent)),
        Span::styled(cat_title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("  (enter / ← → to change · esc / backspace to back)", Style::default().fg(Color::Rgb(70, 70, 70))),
    ]));
    f.render_widget(header, chunks[0]);

    let items: Vec<(&str, String)> = match cat_idx {
        0 => vec![
            ("color theme", app.store.config.theme.name().to_string()),
            ("player style", app.store.config.visual.name().to_string()),
            ("progress bar width", format!("{} cols", app.store.config.progress_bar_width)),
            (
                "bottom hints bar",
                if app.store.config.show_bottom_hints {
                    "visible".to_string()
                } else {
                    "hidden".to_string()
                },
            ),
            (
                "top status bar",
                if app.store.config.show_top_status {
                    "visible".to_string()
                } else {
                    "hidden".to_string()
                },
            ),
            (
                "track durations",
                if app.store.config.show_durations {
                    "shown".to_string()
                } else {
                    "hidden".to_string()
                },
            ),
            (
                "favorite heart icons",
                if app.store.config.show_hearts {
                    "shown (♥)".to_string()
                } else {
                    "hidden".to_string()
                },
            ),
        ],
        1 => vec![
            ("default volume", format!("{}%", app.store.config.default_volume)),
            ("volume step (+/-)", format!("{}%", app.store.config.volume_step)),
            ("speed pitch mode", app.store.config.pitch_mode.name().to_string()),
            ("slowed preset (< / [)", format!("{:.2}x", app.store.config.slowed_preset)),
            ("sped up preset (> / ])", format!("{:.2}x", app.store.config.speedup_preset)),
            ("audio DSP filter (EQ)", app.store.config.audio_filter.name().to_string()),
        ],
        2 => vec![
            (
                "auto-wave radio",
                if app.store.config.auto_wave {
                    "enabled (infinite related tracks)".to_string()
                } else {
                    "disabled".to_string()
                },
            ),
            (
                "resume position & speed",
                if app.store.config.resume_playback {
                    "remember per track".to_string()
                } else {
                    "always start at 0:00".to_string()
                },
            ),
            ("seek step (←/→)", format!("{} sec", app.store.config.seek_step)),
            (
                "default shuffle on start",
                if app.store.config.default_shuffle {
                    "on".to_string()
                } else {
                    "off".to_string()
                },
            ),
            ("default repeat mode", app.store.config.default_repeat.clone()),
            (
                "terminal mouse support",
                if app.store.config.mouse_support {
                    "enabled (click to play/seek)".to_string()
                } else {
                    "disabled (keyboard only)".to_string()
                },
            ),
        ],
        3 => vec![
            (
                "discord rich presence",
                if app.store.config.discord_rpc {
                    "on".to_string()
                } else {
                    "off".to_string()
                },
            ),
            ("discord activity type", app.store.config.rpc_activity_type.name().to_string()),
            (
                "discord button link",
                if app.store.config.discord_buttons {
                    "on (visible to others)".to_string()
                } else {
                    "off (visible to self & others)".to_string()
                },
            ),
            (
                "discord elapsed timer",
                if app.store.config.discord_timestamps {
                    "shown".to_string()
                } else {
                    "hidden".to_string()
                },
            ),
            (
                "system media keys (MPRIS2)",
                if app.store.config.mpris_enabled {
                    "enabled".to_string()
                } else {
                    "disabled".to_string()
                },
            ),
            (
                "auto-fetch synced lyrics",
                if app.store.config.auto_lyrics {
                    "enabled (LRCLIB)".to_string()
                } else {
                    "disabled".to_string()
                },
            ),
        ],
        4 => vec![
            (
                "gapless disk prefetch",
                if app.store.config.gapless_prefetch {
                    "enabled (at 70% track pos)".to_string()
                } else {
                    "disabled".to_string()
                },
            ),
            ("max history entries", format!("{} tracks", app.store.config.history_limit)),
            ("clear audio cache", crate::storage::DataStore::cache_size_label()),
            ("clear listening history", format!("{} tracks saved", app.store.history.len())),
        ],
        _ => Vec::new(),
    };

    let mut lines = Vec::new();
    for (idx, (label, val)) in items.iter().enumerate() {
        let is_selected = idx == app.settings_cursor;
        let prefix = if is_selected { " → " } else { "   " };
        let style_label = if is_selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(140, 140, 140))
        };
        let style_val = if is_selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(95, 95, 95))
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(if is_selected { accent } else { Color::Rgb(60, 60, 60) })),
            Span::styled(format!("{:<28}", label), style_label),
            Span::styled(format!("‹ {} ›", val), style_val),
        ]));
    }

    let p = Paragraph::new(lines);
    f.render_widget(p, chunks[2]);
}

fn render_player_tape(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    let width = area.width as usize;
    if width < 20 {
        return;
    }

    let track_title = app
        .current_track()
        .map(|t| format!("{}  ·  {}", t.title, t.artist))
        .unwrap_or_else(|| "idle".to_string());

    let state_icon = if app.current_track().is_none() {
        "◼"
    } else if app.is_paused {
        "⏸"
    } else {
        "▶"
    };

    let (cur_str, tot_str, progress_ratio) = if app.total_duration > 0.0 {
        let ratio = (app.current_pos / app.total_duration).clamp(0.0, 1.0);
        (
            format_duration(app.current_pos),
            format_duration(app.total_duration),
            ratio,
        )
    } else if app.current_pos > 0.0 {
        (format_duration(app.current_pos), "--:--".to_string(), 0.0)
    } else {
        ("0:00".to_string(), "0:00".to_string(), 0.0)
    };

    // Dynamic waveform / visual style tape rendering
    let max_bar = app.store.config.progress_bar_width.clamp(20, 120);
    let bar_width = width.saturating_sub(24).clamp(12, max_bar);
    let filled_len = (bar_width as f64 * progress_ratio) as usize;

    let mut tape_spans = Vec::new();
    tape_spans.push(Span::styled(format!("{:>5} ", cur_str), Style::default().fg(Color::Rgb(90, 90, 90))));

    match app.store.config.visual {
        VisualStyle::Waveform => {
            let waveform_blocks = ['▂', '▃', '▄', '▅', '▆', '▇', '█'];
            let total_samples = app.current_waveform.len();
            for i in 0..bar_width {
                let glyph = if total_samples > 0 {
                    let sample_idx = (i * total_samples) / bar_width;
                    let val = app.current_waveform.get(sample_idx).copied().unwrap_or(0) as usize;
                    let glyph_idx = (val * (waveform_blocks.len() - 1)) / 100;
                    waveform_blocks[glyph_idx.min(waveform_blocks.len() - 1)]
                } else {
                    waveform_blocks[(i * 3 + 2) % waveform_blocks.len()]
                };
                let style = if i < filled_len {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else if i == filled_len {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Rgb(55, 55, 55))
                };
                tape_spans.push(Span::styled(glyph.to_string(), style));
            }
        }
        VisualStyle::Braille => {
            let braille_blocks = ['⣀', '⣤', '⣶', '⣿'];
            let total_samples = app.current_waveform.len();
            for i in 0..bar_width {
                let glyph = if total_samples > 0 {
                    let sample_idx = (i * total_samples) / bar_width;
                    let val = app.current_waveform.get(sample_idx).copied().unwrap_or(0) as usize;
                    let glyph_idx = (val * (braille_blocks.len() - 1)) / 100;
                    braille_blocks[glyph_idx.min(braille_blocks.len() - 1)]
                } else {
                    braille_blocks[(i * 2 + 1) % braille_blocks.len()]
                };
                let style = if i < filled_len {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else if i == filled_len {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Rgb(55, 55, 55))
                };
                tape_spans.push(Span::styled(glyph.to_string(), style));
            }
        }
        VisualStyle::Tape => {
            for i in 0..bar_width {
                let (ch, style) = if i < filled_len {
                    ('━', Style::default().fg(accent).add_modifier(Modifier::BOLD))
                } else if i == filled_len {
                    ('●', Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                } else {
                    ('─', Style::default().fg(Color::Rgb(55, 55, 55)))
                };
                tape_spans.push(Span::styled(ch.to_string(), style));
            }
        }
        VisualStyle::Blocky => {
            for i in 0..bar_width {
                let (ch, style) = if i < filled_len {
                    ('█', Style::default().fg(accent))
                } else if i == filled_len {
                    ('▓', Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                } else {
                    ('░', Style::default().fg(Color::Rgb(55, 55, 55)))
                };
                tape_spans.push(Span::styled(ch.to_string(), style));
            }
        }
        VisualStyle::Minimal => {
            for i in 0..bar_width {
                let (ch, style) = if i < filled_len {
                    ('·', Style::default().fg(accent).add_modifier(Modifier::BOLD))
                } else if i == filled_len {
                    ('◆', Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                } else {
                    ('·', Style::default().fg(Color::Rgb(45, 45, 45)))
                };
                tape_spans.push(Span::styled(ch.to_string(), style));
            }
        }
        VisualStyle::Neon => {
            for i in 0..bar_width {
                let (ch, style) = if i < filled_len {
                    ('═', Style::default().fg(accent).add_modifier(Modifier::BOLD))
                } else if i == filled_len {
                    ('◈', Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                } else {
                    ('─', Style::default().fg(Color::Rgb(45, 45, 45)))
                };
                tape_spans.push(Span::styled(ch.to_string(), style));
            }
        }
    }

    tape_spans.push(Span::styled(format!(" {:<5}", tot_str), Style::default().fg(Color::Rgb(90, 90, 90))));

    let line1 = Line::from(vec![
        Span::styled(format!("{} ", state_icon), Style::default().fg(accent)),
        Span::styled(
            truncate_str(&track_title, width.saturating_sub(10)),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
    ]);

    let line2 = Line::from(tape_spans);

    let p = Paragraph::new(vec![line1, line2]).alignment(Alignment::Center);
    f.render_widget(p, area);
}

fn render_user_profile_view(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    let profile = match &app.current_user_profile {
        Some(p) => p,
        None => {
            let empty = Paragraph::new("No profile loaded. Press 'u' on any track to view author").alignment(Alignment::Center);
            f.render_widget(empty, area);
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Length(1), // Blank
            Constraint::Fill(1),   // Tracks
        ])
        .split(area);

    let desc = profile.description.as_deref().unwrap_or("SoundCloud Artist");
    let header = Paragraph::new(Line::from(vec![
        Span::styled("@", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(&profile.username, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  •  {} followers  •  {} tracks", profile.followers_count, profile.track_count), Style::default().fg(Color::Rgb(100, 100, 100))),
        Span::styled(format!("  •  {}", truncate_str(desc, 30)), Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("  (esc to back)", Style::default().fg(Color::Rgb(50, 50, 50))),
    ]));
    f.render_widget(header, chunks[0]);

    render_tracks_list(f, &profile.tracks, app.user_profile_cursor, accent, chunks[2], app);
}

fn render_lyrics_view(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    let current_track = match app.current_track() {
        Some(t) => t,
        None => {
            let empty = Paragraph::new("No track playing").alignment(Alignment::Center);
            f.render_widget(empty, area);
            return;
        }
    };

    let lyrics = match &app.current_lyrics {
        Some(l) if l.track_id == current_track.id => l,
        _ => {
            let loading = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("♪ searching lyrics on LRCLIB...", Style::default().fg(Color::Rgb(100, 100, 100)))),
                Line::from(""),
                Line::from(Span::styled(format!("{} - {}", current_track.artist, current_track.title), Style::default().fg(Color::Rgb(60, 60, 60)))),
            ]).alignment(Alignment::Center);
            f.render_widget(loading, area);
            return;
        }
    };

    let cur_time = app.current_pos;
    let max_lines = area.height as usize;
    if max_lines == 0 {
        return;
    }

    if !lyrics.lines.is_empty() {
        // Find current synced line
        let current_line_idx = lyrics
            .lines
            .iter()
            .rposition(|line| line.time_secs <= cur_time)
            .unwrap_or(0);

        let half_screen = max_lines / 2;
        let start_idx = current_line_idx.saturating_sub(half_screen);

        let mut lines = Vec::new();
        for (idx, line) in lyrics.lines.iter().enumerate().skip(start_idx).take(max_lines) {
            let is_current = idx == current_line_idx;
            if is_current {
                lines.push(Line::from(Span::styled(
                    format!("▶  {}", line.text),
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                )));
            } else {
                let diff = (idx as isize - current_line_idx as isize).abs();
                let dim_val = match diff {
                    1 => 130,
                    2 => 100,
                    3 => 70,
                    _ => 45,
                };
                lines.push(Line::from(Span::styled(
                    format!("   {}", line.text),
                    Style::default().fg(Color::Rgb(dim_val, dim_val, dim_val)),
                )));
            }
        }
        let p = Paragraph::new(lines).alignment(Alignment::Center);
        f.render_widget(p, area);
    } else if let Some(plain) = &lyrics.plain_lyrics {
        let lines: Vec<Line> = plain.lines().take(max_lines).map(|l| Line::from(Span::styled(l, Style::default().fg(Color::Rgb(140, 140, 140))))).collect();
        let p = Paragraph::new(lines).alignment(Alignment::Center);
        f.render_widget(p, area);
    } else {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("No lyrics found for this track", Style::default().fg(Color::Rgb(80, 80, 80)))),
        ]).alignment(Alignment::Center);
        f.render_widget(empty, area);
    }
}

fn render_input_modal(f: &mut Frame, app: &App, input_chars: &[char], prompt: &str) {
    let size = f.area();
    let accent = app.theme_accent();
    let text: String = input_chars.iter().collect();

    let area = centered_rect(50, 20, size);
    f.render_widget(ratatui::widgets::Clear, area);

    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(" Create Playlist ", Style::default().fg(accent).add_modifier(Modifier::BOLD)));
    f.render_widget(block, area);

    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let p = Paragraph::new(vec![
        Line::from(Span::styled(prompt, Style::default().fg(Color::Rgb(140, 140, 140)))),
        Line::from(""),
        Line::from(vec![
            Span::styled("▸ ", Style::default().fg(accent)),
            Span::styled(&text, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("▍", Style::default().fg(accent)),
        ]),
        Line::from(""),
        Line::from(Span::styled("Enter: Confirm  •  Esc: Cancel", Style::default().fg(Color::Rgb(80, 80, 80)))),
    ]);
    f.render_widget(p, inner);
}

fn render_artwork_input_modal(f: &mut Frame, app: &App, input_chars: &[char]) {
    let size = f.area();
    let accent = app.theme_accent();
    let text: String = input_chars.iter().collect();

    let area = centered_rect(65, 24, size);
    f.render_widget(ratatui::widgets::Clear, area);

    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(" Set Playlist Cover Image URL ", Style::default().fg(accent).add_modifier(Modifier::BOLD)));
    f.render_widget(block, area);

    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let p = Paragraph::new(vec![
        Line::from(Span::styled("Paste direct image link (e.g. https://.../cover.jpg or i.imgur.com/...):", Style::default().fg(Color::Rgb(140, 140, 140)))),
        Line::from(""),
        Line::from(vec![
            Span::styled("🖼  ", Style::default().fg(accent)),
            Span::styled(if text.is_empty() { "https://" } else { &text }, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("▍", Style::default().fg(accent)),
        ]),
        Line::from(""),
        Line::from(Span::styled("Tip: Leave blank and press Enter to remove cover image", Style::default().fg(Color::Rgb(95, 95, 95)))),
        Line::from(Span::styled("Enter: Save  •  Esc: Cancel", Style::default().fg(Color::Rgb(80, 80, 80)))),
    ]);
    f.render_widget(p, inner);
}

fn render_add_to_playlist_modal(f: &mut Frame, app: &App, target_track: &crate::storage::TrackItem) {
    let size = f.area();
    let accent = app.theme_accent();

    let area = centered_rect(55, 40, size);
    f.render_widget(ratatui::widgets::Clear, area);

    let title_str = format!(" Add to Playlist: {} ", truncate_str(&target_track.title, 25));
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(title_str, Style::default().fg(accent).add_modifier(Modifier::BOLD)));
    f.render_widget(block, area);

    let inner = area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled("Select target playlist:", Style::default().fg(Color::Rgb(120, 120, 120)))));
    lines.push(Line::from(""));

    for (idx, pl) in app.store.playlists.iter().enumerate() {
        let is_sel = idx == app.modal_playlist_cursor;
        let prefix = if is_sel { " ▸ " } else { "   " };
        let style = if is_sel {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(130, 130, 130))
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(if is_sel { accent } else { Color::Rgb(50, 50, 50) })),
            Span::styled(format!("{:<28}", truncate_str(&pl.title, 26)), style),
            Span::styled(format!("({} tracks)", pl.track_count), Style::default().fg(Color::Rgb(70, 70, 70))),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Enter: Add  •  Esc: Cancel", Style::default().fg(Color::Rgb(80, 80, 80)))));

    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_bottom_bar(f: &mut Frame, _app: &App, area: Rect) {
    let hints = vec![
        Span::styled("  / ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("search ", Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("· ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled("enter ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("play ", Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("· ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled("space ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("pause ", Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("· ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled("tab/1-7 ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("views ", Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("· ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled("w ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("wave ", Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("· ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled("o ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("link ", Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("· ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled("< > ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("slow/fast ", Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("· ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled("0 ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("norm ", Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("· ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled("y ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("lyrics ", Style::default().fg(Color::Rgb(70, 70, 70))),
        Span::styled("· ", Style::default().fg(Color::Rgb(40, 40, 40))),
        Span::styled("q ", Style::default().fg(Color::Rgb(120, 120, 120))),
        Span::styled("quit", Style::default().fg(Color::Rgb(70, 70, 70))),
    ];
    let p = Paragraph::new(Line::from(hints)).alignment(Alignment::Left);
    f.render_widget(p, area);
}

// ---- Cover image rendering (half-block truecolor) ----
const N: usize = 128;
type CoverPixels = Vec<Vec<(u8, u8, u8)>>;
enum CoverState {
    Loading,
    Failed,
    Ready(CoverPixels),
}

static COVER_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, CoverState>>> =
    std::sync::OnceLock::new();

fn cover_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, CoverState>> {
    COVER_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn load_cover(url: &str) -> Option<CoverPixels> {
    let out = std::process::Command::new("curl")
        .args(["-s", "-L", "-m", "10", "-A", "Mozilla/5.0", url])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let img = image::load_from_memory(&out.stdout).ok()?.to_rgb8();
    let thumb = image::imageops::resize(&img, N as u32, N as u32, image::imageops::FilterType::Lanczos3);
    let mut rows = Vec::new();
    for y in 0..N as u32 {
        rows.push((0..N as u32).map(|x| { let p = thumb.get_pixel(x, y); (p[0], p[1], p[2]) }).collect());
    }
    Some(rows)
}

fn render_cover(f: &mut Frame, url: &str, area: Rect, accent: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut spawn = false;
    let lines: Vec<Line> = {
        let mut cache = cover_cache().lock().unwrap();
        if !cache.contains_key(url) {
            cache.insert(url.to_string(), CoverState::Loading);
            spawn = true;
        }
        match cache.get(url) {
            Some(CoverState::Ready(px)) => {
                let w = area.width as usize;
                let h = area.height as usize * 2;
                let avg = |px: &CoverPixels, row: usize, col: usize, rows: usize, cols: usize| -> Color {
                    let y0 = row * N / rows;
                    let y1 = ((row + 1) * N / rows).max(y0 + 1).min(N);
                    let x0 = col * N / cols;
                    let x1 = ((col + 1) * N / cols).max(x0 + 1).min(N);
                    let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let p = px[y][x];
                            r += p.0 as u32;
                            g += p.1 as u32;
                            b += p.2 as u32;
                            n += 1;
                        }
                    }
                    let n = n.max(1);
                    Color::Rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
                };
                (0..area.height as usize)
                    .map(|row| {
                        let spans: Vec<Span> = (0..w)
                            .map(|col| {
                                Span::styled(
                                    "▀",
                                    Style::default()
                                        .fg(avg(px, row * 2, col, h, w))
                                        .bg(avg(px, row * 2 + 1, col, h, w)),
                                )
                            })
                            .collect();
                        Line::from(spans)
                    })
                    .collect()
            }
            Some(CoverState::Failed) => vec![Line::from(Span::styled("cover failed", Style::default().fg(Color::Rgb(90, 90, 90))))],
            _ => vec![Line::from(Span::styled("loading cover…", Style::default().fg(accent)))],
        }
    };
    if spawn {
        let u = url.to_string();
        std::thread::spawn(move || {
            let st = match load_cover(&u) {
                Some(p) => CoverState::Ready(p),
                None => CoverState::Failed,
            };
            cover_cache().lock().unwrap().insert(u, st);
        });
    }
    f.render_widget(Paragraph::new(lines), area);
}
