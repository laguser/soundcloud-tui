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

    render_top_bar(f, app, chunks[0]);

    match app.view_mode {
        ViewMode::Search => render_search_view(f, app, chunks[2]),
        ViewMode::Queue => render_queue_view(f, app, chunks[2]),
        ViewMode::Playlists => render_playlists_view(f, app, chunks[2]),
        ViewMode::PlaylistDetail => render_playlist_detail_view(f, app, chunks[2]),
        ViewMode::Favorites => render_favorites_view(f, app, chunks[2]),
        ViewMode::History => render_history_view(f, app, chunks[2]),
        ViewMode::Settings => render_settings_view(f, app, chunks[2]),
    }

    render_player_tape(f, app, chunks[4]);
    render_bottom_bar(f, app, chunks[5]);
}

fn format_duration(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    let m = s / 60;
    let rem_s = s % 60;
    format!("{}:{:02}", m, rem_s)
}

fn render_top_bar(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.theme_accent();
    let current_view = app.view_mode.name();

    let mut spans = vec![
        Span::styled("◈ ", Style::default().fg(accent)),
        Span::styled(current_view, Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled("  •  ", Style::default().fg(Color::Rgb(50, 50, 50))),
        Span::styled(format!("{}%", app.volume), Style::default().fg(Color::Rgb(140, 140, 140))),
    ];

    if let Some((msg, _)) = &app.status_message {
        spans.push(Span::styled("  •  ", Style::default().fg(Color::Rgb(50, 50, 50))));
        spans.push(Span::styled(
            msg.as_str(),
            Style::default().fg(Color::Rgb(100, 100, 100)),
        ));
    }

    let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
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
            "press 'i' or '/' to search SoundCloud...",
            Style::default().fg(Color::Rgb(70, 70, 70)),
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
            " → "
        } else if is_playing {
            " ▶ "
        } else {
            "   "
        };

        let like_icon = if is_liked { "♥ " } else { "  " };

        let title_style = if is_selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else if is_playing {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(150, 150, 150))
        };

        let artist_style = if is_selected {
            Style::default().fg(accent)
        } else {
            Style::default().fg(Color::Rgb(75, 75, 75))
        };

        let dur = if track.duration > 0.0 {
            format_duration(track.duration)
        } else {
            "".to_string()
        };

        lines.push(Line::from(vec![
            Span::styled(cursor_glyph, Style::default().fg(if is_selected { accent } else { Color::Rgb(60, 60, 60) })),
            Span::styled(like_icon, Style::default().fg(if is_liked { Color::Red } else { Color::Rgb(40, 40, 40) })),
            Span::styled(format!("{:<40}", truncate_str(&track.title, 38)), title_style),
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
            Line::from(Span::styled("paste any soundcloud playlist link in search to load", Style::default().fg(Color::Rgb(50, 50, 50)))),
        ])
        .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    let max_lines = area.height as usize;
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

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(if is_selected { accent } else { Color::Rgb(60, 60, 60) })),
            Span::styled("≡ ", Style::default().fg(accent)),
            Span::styled(format!("{:<38}", truncate_str(&pl.title, 36)), title_style),
            Span::styled(format!(" {:<20}", truncate_str(&pl.author, 18)), Style::default().fg(Color::Rgb(80, 80, 80))),
            Span::styled(format!("{:>6} tracks", pl.track_count), Style::default().fg(Color::Rgb(65, 65, 65))),
        ]));
    }

    let p = Paragraph::new(lines);
    f.render_widget(p, area);
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

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header info
            Constraint::Length(1), // Blank
            Constraint::Fill(1),   // Tracks
        ])
        .split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("≡ ", Style::default().fg(accent)),
        Span::styled(&playlist.title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  by {}  •  {} tracks", playlist.author, playlist.track_count), Style::default().fg(Color::Rgb(90, 90, 90))),
        Span::styled("  (esc to back)", Style::default().fg(Color::Rgb(60, 60, 60))),
    ]));
    f.render_widget(header, chunks[0]);

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

fn render_settings_view(f: &mut Frame, app: &App, size: Rect) {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(8), Constraint::Fill(1)])
        .split(size);

    let accent = app.theme_accent();
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled("settings", Style::default().fg(accent).add_modifier(Modifier::BOLD))));
    lines.push(Line::from(Span::styled("────────────", Style::default().fg(Color::Rgb(40, 40, 40)))));

    let items = [
        ("theme", app.store.config.theme.name().to_string()),
        ("style", app.store.config.visual.name().to_string()),
        ("volume", format!("{}%", app.store.config.default_volume)),
        ("discord rpc", if app.store.config.discord_rpc { "on".to_string() } else { "off".to_string() }),
    ];

    for (idx, (label, val)) in items.iter().enumerate() {
        let is_selected = idx == app.settings_cursor;
        let prefix = if is_selected { " → " } else { "   " };
        let style_label = if is_selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(100, 100, 100))
        };
        let style_val = if is_selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(120, 120, 120))
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(if is_selected { accent } else { Color::Rgb(50, 50, 50) })),
            Span::styled(format!("{:<16}", label), style_label),
            Span::styled(val.as_str(), style_val),
        ]));
    }

    let p = Paragraph::new(lines).alignment(Alignment::Center);
    f.render_widget(p, v[1]);
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

    // Tape conveyor width
    let bar_width = width.saturating_sub(24).clamp(12, 70);
    let filled_len = (bar_width as f64 * progress_ratio) as usize;

    let bar_str: String = match app.store.config.visual {
        VisualStyle::Tape => {
            let mut s = String::new();
            for i in 0..bar_width {
                if i < filled_len {
                    s.push('━');
                } else if i == filled_len {
                    s.push('●');
                } else {
                    s.push('─');
                }
            }
            s
        }
        VisualStyle::Blocky => {
            let mut s = String::new();
            for i in 0..bar_width {
                if i < filled_len {
                    s.push('█');
                } else if i == filled_len {
                    s.push('▓');
                } else {
                    s.push('░');
                }
            }
            s
        }
        VisualStyle::Minimal => {
            let mut s = String::new();
            for i in 0..bar_width {
                if i < filled_len {
                    s.push('·');
                } else if i == filled_len {
                    s.push('◆');
                } else {
                    s.push(' ');
                }
            }
            s
        }
    };

    let line1 = Line::from(vec![
        Span::styled(format!("{} ", state_icon), Style::default().fg(accent)),
        Span::styled(
            truncate_str(&track_title, width.saturating_sub(10)),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
    ]);

    let line2 = Line::from(vec![
        Span::styled(format!("{:>5} ", cur_str), Style::default().fg(Color::Rgb(90, 90, 90))),
        Span::styled(bar_str, Style::default().fg(accent)),
        Span::styled(format!(" {:<5}", tot_str), Style::default().fg(Color::Rgb(90, 90, 90))),
    ]);

    let p = Paragraph::new(vec![line1, line2]).alignment(Alignment::Center);
    f.render_widget(p, area);
}

fn render_bottom_bar(f: &mut Frame, _app: &App, area: Rect) {
    let hints = vec![
        Span::styled("space ", Style::default().fg(Color::Rgb(100, 100, 100))),
        Span::styled("play", Style::default().fg(Color::Rgb(60, 60, 60))),
        Span::styled("  •  ", Style::default().fg(Color::Rgb(35, 35, 35))),
        Span::styled("n/p ", Style::default().fg(Color::Rgb(100, 100, 100))),
        Span::styled("skip", Style::default().fg(Color::Rgb(60, 60, 60))),
        Span::styled("  •  ", Style::default().fg(Color::Rgb(35, 35, 35))),
        Span::styled("l ", Style::default().fg(Color::Rgb(100, 100, 100))),
        Span::styled("like", Style::default().fg(Color::Rgb(60, 60, 60))),
        Span::styled("  •  ", Style::default().fg(Color::Rgb(35, 35, 35))),
        Span::styled("a ", Style::default().fg(Color::Rgb(100, 100, 100))),
        Span::styled("queue", Style::default().fg(Color::Rgb(60, 60, 60))),
        Span::styled("  •  ", Style::default().fg(Color::Rgb(35, 35, 35))),
        Span::styled("tab ", Style::default().fg(Color::Rgb(100, 100, 100))),
        Span::styled("views", Style::default().fg(Color::Rgb(60, 60, 60))),
        Span::styled("  •  ", Style::default().fg(Color::Rgb(35, 35, 35))),
        Span::styled("esc ", Style::default().fg(Color::Rgb(100, 100, 100))),
        Span::styled("quit (x2)", Style::default().fg(Color::Rgb(60, 60, 60))),
    ];
    let p = Paragraph::new(Line::from(hints)).alignment(Alignment::Center);
    f.render_widget(p, area);
}
