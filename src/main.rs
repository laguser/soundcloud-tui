mod app;
mod audio;
mod discord;
mod lyrics;
mod mpris;
mod search;
mod storage;
mod ui;

use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn Error>> {
    // Check prerequisites
    if std::process::Command::new("yt-dlp").arg("--version").output().is_err() {
        eprintln!("Error: yt-dlp is required. Install it via 'pacman -S yt-dlp' or package manager.");
        std::process::exit(1);
    }
    if std::process::Command::new("mpv").arg("--version").output().is_err() {
        eprintln!("Error: mpv is required. Install it via 'pacman -S mpv' or package manager.");
        std::process::exit(1);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let tick_rate = Duration::from_millis(30);
    let mut last_tick = Instant::now();

    loop {
        app.poll_async_events();
        terminal.draw(|f| ui::render(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                        app.save_current_state();
                        break;
                    }

                    let in_any_modal_or_input = app.is_editing_search
                        || app.modal_input.is_some()
                        || app.modal_target_track.is_some()
                        || app.modal_artwork_input.is_some()
                        || app.modal_import_input.is_some();

                    if (key.code == KeyCode::Char('q') || key.code == KeyCode::Char('й')) && !in_any_modal_or_input {
                        app.save_current_state();
                        break;
                    }

                    // Double ESC to quit, single ESC to exit search input / return
                    if key.code == KeyCode::Esc {
                        let now = Instant::now();
                        let is_double_esc = if let Some(last) = app.last_esc_press {
                            now.duration_since(last) <= Duration::from_millis(500)
                        } else {
                            false
                        };

                        if is_double_esc {
                            app.save_current_state();
                            break;
                        }

                        app.last_esc_press = Some(now);

                        if app.modal_artwork_input.is_some() {
                            app.modal_artwork_input = None;
                        } else if app.modal_import_input.is_some() {
                            app.modal_import_input = None;
                        } else if app.modal_input.is_some() {
                            app.modal_input = None;
                        } else if app.modal_target_track.is_some() {
                            app.modal_target_track = None;
                        } else if app.is_editing_search {
                            app.is_editing_search = false;
                        } else if app.view_mode == app::ViewMode::PlaylistDetail {
                            app.view_mode = app::ViewMode::Playlists;
                        } else if app.view_mode == app::ViewMode::Settings && app.selected_settings_category.is_some() {
                            app.selected_settings_category = None;
                        } else {
                            app.view_mode = app::ViewMode::Search;
                        }
                        continue;
                    }

                    app.handle_key(key);
                    app.poll_async_events();
                }
                Event::Mouse(mouse) => {
                    if let Ok(size) = terminal.size() {
                        let rect = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        app.handle_mouse(mouse, rect);
                        app.poll_async_events();
                    }
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}
