use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub time_secs: f64,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct TrackLyrics {
    pub track_id: String,
    pub lines: Vec<LyricLine>,
    pub plain_lyrics: Option<String>,
}

pub struct LyricsManager {
    task_tx: Sender<(String, String, String)>, // (track_id, track_title, artist)
    res_rx: Receiver<TrackLyrics>,
}

impl LyricsManager {
    pub fn new() -> Self {
        let (task_tx, task_rx) = channel::<(String, String, String)>();
        let (res_tx, res_rx) = channel::<TrackLyrics>();

        thread::spawn(move || {
            while let Ok((track_id, title, artist)) = task_rx.recv() {
                let lyrics = Self::fetch_lyrics(&track_id, &title, &artist);
                let _ = res_tx.send(lyrics);
            }
        });

        Self { task_tx, res_rx }
    }

    pub fn request_lyrics(&self, track_id: &str, title: &str, artist: &str) {
        let _ = self.task_tx.send((track_id.to_string(), title.to_string(), artist.to_string()));
    }

    pub fn poll_lyrics(&self) -> Option<TrackLyrics> {
        self.res_rx.try_recv().ok()
    }

    fn clean_title(title: &str) -> String {
        // Strip common tags like (prod. ...), (slowed), [official video], feat etc
        let mut clean = title.to_string();
        if let Some(pos) = clean.find(" - ") {
            clean = clean[pos + 3..].to_string();
        }
        for open in ['(', '['] {
            if let Some(pos) = clean.find(open) {
                clean = clean[..pos].to_string();
            }
        }
        clean.trim().to_string()
    }

    fn fetch_lyrics(track_id: &str, title: &str, artist: &str) -> TrackLyrics {
        let clean_t = Self::clean_title(title);
        let q = format!("{} {}", artist, clean_t).replace(' ', "%20");
        let search_url = format!("https://lrclib.net/api/search?q={}", q);

        let output = Command::new("curl")
            .arg("-s")
            .arg("-m")
            .arg("5")
            .arg("-H")
            .arg("User-Agent: SoundCloudTUI/0.2.0 (linux)")
            .arg(&search_url)
            .output();

        let mut lines = Vec::new();
        let mut plain_lyrics = None;

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(v) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
                    if let Some(first) = v.first() {
                        if let Some(synced) = first.get("syncedLyrics").and_then(|s| s.as_str()) {
                            lines = Self::parse_lrc(synced);
                        } else if let Some(plain) = first.get("plainLyrics").and_then(|p| p.as_str()) {
                            plain_lyrics = Some(plain.to_string());
                        }
                    }
                }
            }
        }

        TrackLyrics {
            track_id: track_id.to_string(),
            lines,
            plain_lyrics,
        }
    }

    fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
        let mut result = Vec::new();
        for line in lrc.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                if let Some(closing) = line.find(']') {
                    let time_part = &line[1..closing];
                    let text_part = line[closing + 1..].trim();

                    // Parse mm:ss.xx
                    let parts: Vec<&str> = time_part.split(':').collect();
                    if parts.len() == 2 {
                        let mins = parts[0].parse::<f64>().unwrap_or(0.0);
                        let secs = parts[1].parse::<f64>().unwrap_or(0.0);
                        let total = mins * 60.0 + secs;

                        result.push(LyricLine {
                            time_secs: total,
                            text: text_part.to_string(),
                        });
                    }
                }
            }
        }
        result.sort_by(|a, b| a.time_secs.partial_cmp(&b.time_secs).unwrap());
        result
    }
}
