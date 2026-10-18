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
    event::{self, Event, KeyCode, KeyModifiers},
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
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let tick_rate = Duration::from_millis(50);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::render(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
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
                        break;
                    }

                    app.last_esc_press = Some(now);

                    if app.is_editing_search {
                        app.is_editing_search = false;
                    } else if app.view_mode == app::ViewMode::PlaylistDetail {
                        app.view_mode = app::ViewMode::Playlists;
                    } else {
                        app.view_mode = app::ViewMode::Search;
                    }
                    continue;
                }

                app.handle_key(key);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
            app.poll_async_events();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
