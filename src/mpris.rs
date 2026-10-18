use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

pub enum MprisEvent {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
}

pub struct MprisManager {
    controls: Option<MediaControls>,
    event_rx: Receiver<MprisEvent>,
}

impl MprisManager {
    pub fn new() -> Self {
        let (tx, rx) = channel::<MprisEvent>();

        #[cfg(not(target_os = "windows"))]
        let hwnd = None;
        #[cfg(target_os = "windows")]
        let hwnd = None;

        let config = PlatformConfig {
            dbus_name: "soundcloud-tui",
            display_name: "SoundCloud TUI",
            hwnd,
        };

        let mut controls = MediaControls::new(config).ok();

        if let Some(ref mut c) = controls {
            let tx_clone = tx;
            let res = c.attach(move |event: MediaControlEvent| {
                match event {
                    MediaControlEvent::Play => {
                        let _ = tx_clone.send(MprisEvent::Play);
                    }
                    MediaControlEvent::Pause => {
                        let _ = tx_clone.send(MprisEvent::Pause);
                    }
                    MediaControlEvent::Toggle => {
                        let _ = tx_clone.send(MprisEvent::Toggle);
                    }
                    MediaControlEvent::Next => {
                        let _ = tx_clone.send(MprisEvent::Next);
                    }
                    MediaControlEvent::Previous => {
                        let _ = tx_clone.send(MprisEvent::Previous);
                    }
                    _ => {}
                }
            });
            if res.is_err() {
                controls = None;
            }
        }

        Self {
            controls,
            event_rx: rx,
        }
    }

    pub fn update(
        &mut self,
        title: &str,
        artist: &str,
        artwork_url: Option<&str>,
        duration: f64,
        is_paused: bool,
    ) {
        if let Some(ref mut c) = self.controls {
            let dur = if duration > 0.0 {
                Some(Duration::from_secs_f64(duration))
            } else {
                None
            };

            let _ = c.set_metadata(MediaMetadata {
                title: Some(title),
                artist: Some(artist),
                album: Some("SoundCloud"),
                cover_url: artwork_url,
                duration: dur,
            });

            let playback = if is_paused {
                MediaPlayback::Paused { progress: None }
            } else {
                MediaPlayback::Playing { progress: None }
            };
            let _ = c.set_playback(playback);
        }
    }

    pub fn poll_event(&self) -> Option<MprisEvent> {
        self.event_rx.try_recv().ok()
    }
}
