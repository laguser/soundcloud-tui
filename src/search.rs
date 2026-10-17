use crate::storage::TrackItem;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub enum SearchResult {
    Success(Vec<TrackItem>),
    Error(String),
}

pub struct SearchEngine {
    tx: Sender<(String, bool)>, // (query, is_playlist_or_url)
    rx: Receiver<SearchResult>,
    #[allow(dead_code)]
    cached_client_id: Arc<Mutex<Option<String>>>,
}

impl SearchEngine {
    pub fn new() -> Self {
        let (task_tx, task_rx) = channel::<(String, bool)>();
        let (res_tx, res_rx) = channel::<SearchResult>();
        let cached_client_id = Arc::new(Mutex::new(Some("tHVbSbDP4TaoMbPppngv6Tdry83vNbMQ".to_string())));

        let cid_clone = Arc::clone(&cached_client_id);
        thread::spawn(move || {
            while let Ok((query, is_direct_url)) = task_rx.recv() {
                let result = Self::execute_search(&query, is_direct_url, &cid_clone);
                let _ = res_tx.send(result);
            }
        });

        Self {
            tx: task_tx,
            rx: res_rx,
            cached_client_id,
        }
    }

    pub fn search(&self, query: &str) {
        let is_url = query.starts_with("http://")
            || query.starts_with("https://")
            || query.starts_with("soundcloud.com/")
            || query.starts_with("snd.sc/");
        let _ = self.tx.send((query.to_string(), is_url));
    }

    pub fn poll_result(&self) -> Option<SearchResult> {
        self.rx.try_recv().ok()
    }

    #[allow(dead_code)]
    pub fn resolve_stream_url(track: &TrackItem, client_id_holder: &Arc<Mutex<Option<String>>>) -> Option<String> {
        if let Some(stream) = &track.stream_url {
            return Some(stream.clone());
        }

        let cid = {
            let guard = client_id_holder.lock().unwrap();
            guard.clone().unwrap_or_else(|| "tHVbSbDP4TaoMbPppngv6Tdry83vNbMQ".to_string())
        };

        // Try direct transcodings lookup via curl
        let api_url = format!(
            "https://api-v2.soundcloud.com/tracks/{}?client_id={}",
            track.id, cid
        );

        let output = Command::new("curl")
            .arg("-s")
            .arg("-m")
            .arg("5")
            .arg("-H")
            .arg("User-Agent: Mozilla/5.0")
            .arg(&api_url)
            .output()
            .ok()?;

        if output.status.success() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if let Some(transcodings) = v.get("media").and_then(|m| m.get("transcodings")).and_then(|t| t.as_array()) {
                    for tr in transcodings {
                        if let Some(tr_url) = tr.get("url").and_then(|u| u.as_str()) {
                            let stream_lookup = format!("{}?client_id={}", tr_url, cid);
                            let stream_res = Command::new("curl")
                                .arg("-s")
                                .arg("-m")
                                .arg("4")
                                .arg("-H")
                                .arg("User-Agent: Mozilla/5.0")
                                .arg(&stream_lookup)
                                .output()
                                .ok();

                            if let Some(s_out) = stream_res {
                                if s_out.status.success() {
                                    if let Ok(sv) = serde_json::from_slice::<serde_json::Value>(&s_out.stdout) {
                                        if let Some(direct_url) = sv.get("url").and_then(|u| u.as_str()) {
                                            return Some(direct_url.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    fn execute_search(
        query: &str,
        is_direct_url: bool,
        cached_cid: &Arc<Mutex<Option<String>>>,
    ) -> SearchResult {
        // Fast path: if not a direct URL, query SoundCloud api-v2 via curl
        if !is_direct_url {
            let cid = {
                let guard = cached_cid.lock().unwrap();
                guard.clone().unwrap_or_else(|| "tHVbSbDP4TaoMbPppngv6Tdry83vNbMQ".to_string())
            };

            let encoded_q = query.replace(' ', "%20");
            let api_url = format!(
                "https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit=25",
                encoded_q, cid
            );

            let output = Command::new("curl")
                .arg("-s")
                .arg("-m")
                .arg("5")
                .arg("-H")
                .arg("User-Agent: Mozilla/5.0")
                .arg(&api_url)
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                        if let Some(collection) = v.get("collection").and_then(|c| c.as_array()) {
                            let mut tracks = Vec::new();
                            for item in collection {
                                let id = item.get("id").map(|i| i.to_string()).unwrap_or_default();
                                let title = item
                                    .get("title")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("Unknown")
                                    .to_string();
                                let artist = item
                                    .get("user")
                                    .and_then(|u| u.get("username"))
                                    .and_then(|u| u.as_str())
                                    .unwrap_or("SoundCloud")
                                    .to_string();
                                let url = item
                                    .get("permalink_url")
                                    .and_then(|u| u.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let duration = item
                                    .get("duration")
                                    .and_then(|d| d.as_f64())
                                    .map(|ms| ms / 1000.0)
                                    .unwrap_or(0.0);

                                if !url.is_empty() && !title.is_empty() {
                                    tracks.push(TrackItem {
                                        id,
                                        title,
                                        artist,
                                        duration,
                                        url,
                                        stream_url: None,
                                    });
                                }
                            }

                            if !tracks.is_empty() {
                                return SearchResult::Success(tracks);
                            }
                        }
                    }
                }
            }
        }

        // Fallback to yt-dlp flat playlist extraction (for URLs, playlists, or if API failed)
        let target = if is_direct_url {
            if !query.starts_with("http") {
                format!("https://{}", query)
            } else {
                query.to_string()
            }
        } else {
            format!("scsearch15:{}", query)
        };

        let output = Command::new("yt-dlp")
            .arg("--flat-playlist")
            .arg("--skip-download")
            .arg("--print")
            .arg("%(id)s\t%(title)s\t%(uploader)s\t%(webpage_url)s\t%(duration)s")
            .arg("--no-warnings")
            .arg(&target)
            .output();

        let mut tracks = Vec::new();

        if let Ok(out) = output {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 4 {
                        let id = parts[0].trim().to_string();
                        let title = parts[1].trim().to_string();
                        let artist = parts[2].trim().to_string();
                        let mut url = parts[3].trim().to_string();
                        let duration = parts
                            .get(4)
                            .and_then(|d| d.trim().parse::<f64>().ok())
                            .unwrap_or(0.0);

                        if url.is_empty() || url == "NA" {
                            if !id.is_empty() {
                                url = format!("https://soundcloud.com/{}", id);
                            }
                        }

                        if !title.is_empty() && title != "NA" {
                            tracks.push(TrackItem {
                                id,
                                title,
                                artist: if artist == "NA" || artist.is_empty() {
                                    "SoundCloud".to_string()
                                } else {
                                    artist
                                },
                                duration,
                                url,
                                stream_url: None,
                            });
                        }
                    }
                }
            }
        }

        if tracks.is_empty() {
            SearchResult::Error("No tracks found or query failed".to_string())
        } else {
            SearchResult::Success(tracks)
        }
    }
}
