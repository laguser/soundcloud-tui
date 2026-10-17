use crate::storage::TrackItem;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    pub permalink: String,
    pub avatar_url: Option<String>,
    pub description: Option<String>,
    pub followers_count: u64,
    pub track_count: usize,
    pub tracks: Vec<TrackItem>,
}

pub enum SearchResult {
    Success(Vec<TrackItem>),
    PlaylistLoaded(crate::storage::PlaylistItem),
    UserProfileLoaded(UserProfile),
    Error(String),
}

pub enum SearchTask {
    Query(String, bool),
    FetchUser(String), // username, permalink or user_id
}

pub struct SearchEngine {
    tx: Sender<SearchTask>,
    rx: Receiver<SearchResult>,
    pub cached_client_id: Arc<Mutex<Option<String>>>,
}

impl SearchEngine {
    pub fn new(initial_cid: Option<String>) -> Self {
        let (task_tx, task_rx) = channel::<SearchTask>();
        let (res_tx, res_rx) = channel::<SearchResult>();
        let cid_val = initial_cid.unwrap_or_else(|| "tHVbSbDP4TaoMbPppngv6Tdry83vNbMQ".to_string());
        let cached_client_id = Arc::new(Mutex::new(Some(cid_val)));

        let cid_clone = Arc::clone(&cached_client_id);
        thread::spawn(move || {
            while let Ok(task) = task_rx.recv() {
                match task {
                    SearchTask::Query(query, is_direct_url) => {
                        let result = Self::execute_search(&query, is_direct_url, &cid_clone);
                        let _ = res_tx.send(result);
                    }
                    SearchTask::FetchUser(user_query) => {
                        let result = Self::execute_fetch_user(&user_query, &cid_clone);
                        let _ = res_tx.send(result);
                    }
                }
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
        let _ = self.tx.send(SearchTask::Query(query.to_string(), is_url));
    }

    pub fn fetch_user(&self, user_query: &str) {
        let _ = self.tx.send(SearchTask::FetchUser(user_query.to_string()));
    }

    pub fn prefetch_track(track: &TrackItem, client_id_holder: &Arc<Mutex<Option<String>>>) {
        if crate::storage::DataStore::get_cached_track_path(&track.id).is_some() {
            return;
        }
        let track_clone = track.clone();
        let holder = Arc::clone(client_id_holder);
        thread::spawn(move || {
            let _ = Self::resolve_stream_url(&track_clone, &holder);
        });
    }

    pub fn poll_result(&self) -> Option<SearchResult> {
        self.rx.try_recv().ok()
    }

    pub fn resolve_stream_url(track: &TrackItem, client_id_holder: &Arc<Mutex<Option<String>>>) -> Option<String> {
        // 1. Check local on-disk cache first
        if let Some(cached_file) = crate::storage::DataStore::get_cached_track_path(&track.id) {
            return Some(cached_file.to_string_lossy().to_string());
        }

        let cid = {
            let guard = client_id_holder.lock().unwrap();
            guard.clone().unwrap_or_else(|| "tHVbSbDP4TaoMbPppngv6Tdry83vNbMQ".to_string())
        };

        // If stream_url already contains transcoding endpoint, query direct stream URL in 1 jump!
        if let Some(trans_url) = &track.stream_url {
            if trans_url.contains("api-v2.soundcloud.com/media") {
                let stream_lookup = format!("{}?client_id={}", trans_url, cid);
                let stream_res = Command::new("curl")
                    .arg("-s")
                    .arg("-m")
                    .arg("3")
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
            } else if trans_url.starts_with("http") {
                return Some(trans_url.clone());
            }
        }

        // Try direct transcodings lookup via curl
        let api_url = format!(
            "https://api-v2.soundcloud.com/tracks/{}?client_id={}",
            track.id, cid
        );

        let output = Command::new("curl")
            .arg("-s")
            .arg("-m")
            .arg("4")
            .arg("-H")
            .arg("User-Agent: Mozilla/5.0")
            .arg(&api_url)
            .output()
            .ok()?;

        if output.status.success() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if let Some(transcodings) = v.get("media").and_then(|m| m.get("transcodings")).and_then(|t| t.as_array()) {
                    // Prefer progressive format for instant seek and fast playback
                    let mut chosen_url = None;
                    for tr in transcodings {
                        if tr.get("format").and_then(|f| f.get("protocol")).and_then(|p| p.as_str()) == Some("progressive") {
                            chosen_url = tr.get("url").and_then(|u| u.as_str());
                            break;
                        }
                    }
                    if chosen_url.is_none() && !transcodings.is_empty() {
                        chosen_url = transcodings[0].get("url").and_then(|u| u.as_str());
                    }

                    if let Some(tr_url) = chosen_url {
                        let stream_lookup = format!("{}?client_id={}", tr_url, cid);
                        let stream_res = Command::new("curl")
                            .arg("-s")
                            .arg("-m")
                            .arg("3")
                            .arg("-H")
                            .arg("User-Agent: Mozilla/5.0")
                            .arg(&stream_lookup)
                            .output()
                            .ok();

                        if let Some(s_out) = stream_res {
                            if s_out.status.success() {
                                if let Ok(sv) = serde_json::from_slice::<serde_json::Value>(&s_out.stdout) {
                                    if let Some(direct_url) = sv.get("url").and_then(|u| u.as_str()) {
                                        let resolved = direct_url.to_string();

                                        // Spawn background cache downloader
                                        let track_id_clean = track.id.clone();
                                        let stream_copy = resolved.clone();
                                        thread::spawn(move || {
                                            Self::download_to_cache(&track_id_clean, &stream_copy);
                                        });

                                        return Some(resolved);
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

    fn download_to_cache(track_id: &str, stream_url: &str) {
        if track_id.is_empty() {
            return;
        }
        let safe_name: String = track_id.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
        let cache_dir = crate::storage::DataStore::cache_dir();

        // If it is an HLS m3u8 stream, use ffmpeg -c copy into .m4a container for 1:1 lossless instant audio
        if stream_url.contains(".m3u8") || stream_url.contains("stream/hls") {
            let m4a_file = cache_dir.join(format!("{}.m4a", safe_name));
            let tmp_file = cache_dir.join(format!("{}.m4a.tmp", safe_name));

            let _ = Command::new("ffmpeg")
                .arg("-y")
                .arg("-v")
                .arg("error")
                .arg("-headers")
                .arg("User-Agent: Mozilla/5.0\r\n")
                .arg("-i")
                .arg(stream_url)
                .arg("-c")
                .arg("copy")
                .arg(&tmp_file)
                .status();

            if tmp_file.exists() && std::fs::metadata(&tmp_file).map(|m| m.len() > 100 * 1024).unwrap_or(false) {
                let _ = std::fs::rename(tmp_file, m4a_file);
            } else {
                let _ = std::fs::remove_file(tmp_file);
            }
        } else {
            // Direct progressive MP3 download via curl
            let mp3_file = cache_dir.join(format!("{}.mp3", safe_name));
            let tmp_file = cache_dir.join(format!("{}.mp3.tmp", safe_name));

            let _ = Command::new("curl")
                .arg("-s")
                .arg("-m")
                .arg("180")
                .arg("-H")
                .arg("User-Agent: Mozilla/5.0")
                .arg(stream_url)
                .arg("-o")
                .arg(&tmp_file)
                .status();

            if tmp_file.exists() && std::fs::metadata(&tmp_file).map(|m| m.len() > 100 * 1024).unwrap_or(false) {
                let _ = std::fs::rename(tmp_file, mp3_file);
            } else {
                let _ = std::fs::remove_file(tmp_file);
            }
        }
    }

    fn execute_search(
        query: &str,
        is_direct_url: bool,
        cached_cid: &Arc<Mutex<Option<String>>>,
    ) -> SearchResult {
        let cid = {
            let guard = cached_cid.lock().unwrap();
            guard.clone().unwrap_or_else(|| "tHVbSbDP4TaoMbPppngv6Tdry83vNbMQ".to_string())
        };

        // If direct URL, check if it's a playlist or track via api-v2/resolve
        if is_direct_url {
            let full_url = if !query.starts_with("http") {
                format!("https://{}", query)
            } else {
                query.to_string()
            };

            let resolve_endpoint = format!(
                "https://api-v2.soundcloud.com/resolve?url={}&client_id={}",
                full_url, cid
            );

            let output = Command::new("curl")
                .arg("-s")
                .arg("-m")
                .arg("6")
                .arg("-H")
                .arg("User-Agent: Mozilla/5.0")
                .arg(&resolve_endpoint)
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                        let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                        if kind == "playlist" {
                            let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("Untitled Playlist").to_string();
                            let author = v.get("user").and_then(|u| u.get("username")).and_then(|u| u.as_str()).unwrap_or("Unknown").to_string();
                            let playlist_id = v.get("id").map(|i| i.to_string()).unwrap_or_default();
                            let mut playlist_tracks = Vec::new();

                            if let Some(track_array) = v.get("tracks").and_then(|t| t.as_array()) {
                                let mut stub_ids = Vec::new();
                                for t in track_array {
                                    let t_id = t.get("id").map(|i| i.to_string()).unwrap_or_default();
                                    let t_title = t.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    if !t_id.is_empty() {
                                        if t_title.is_empty() || t_title == "Unknown" {
                                            stub_ids.push(t_id.clone());
                                        }
                                        let t_artist = t.get("user").and_then(|u| u.get("username")).and_then(|u| u.as_str()).unwrap_or(&author).to_string();
                                        let t_url = t.get("permalink_url").and_then(|u| u.as_str()).unwrap_or("").to_string();
                                        let t_duration = t.get("duration").and_then(|d| d.as_f64()).map(|ms| ms / 1000.0).unwrap_or(0.0);
                                        let t_artwork = t.get("artwork_url").and_then(|a| a.as_str()).map(|s| s.to_string());
                                        let t_waveform = t.get("waveform_url").and_then(|w| w.as_str()).map(|s| s.to_string());
                                        let t_u_permalink = t.get("user").and_then(|u| u.get("permalink")).and_then(|p| p.as_str()).map(|s| s.to_string());
                                        let t_u_id = t.get("user").and_then(|u| u.get("id")).map(|i| i.to_string());

                                        playlist_tracks.push(TrackItem {
                                            id: t_id,
                                            title: if t_title.is_empty() { "Loading...".to_string() } else { t_title },
                                            artist: t_artist,
                                            duration: t_duration,
                                            url: if t_url.is_empty() { format!("https://soundcloud.com/tracks/{}", playlist_tracks.len()) } else { t_url },
                                            stream_url: None,
                                            artwork_url: t_artwork,
                                            waveform_url: t_waveform,
                                            user_permalink: t_u_permalink,
                                            user_id: t_u_id,
                                        });
                                    }
                                }

                                // Hydrate track stubs in chunks of 50
                                for chunk in stub_ids.chunks(50) {
                                    let ids_param = chunk.join(",");
                                    let bulk_url = format!("https://api-v2.soundcloud.com/tracks?ids={}&client_id={}", ids_param, cid);
                                    let b_out = Command::new("curl")
                                        .arg("-s")
                                        .arg("-m")
                                        .arg("6")
                                        .arg("-H")
                                        .arg("User-Agent: Mozilla/5.0")
                                        .arg(&bulk_url)
                                        .output();
                                    if let Ok(res) = b_out {
                                        if res.status.success() {
                                            if let Ok(full_tracks) = serde_json::from_slice::<Vec<serde_json::Value>>(&res.stdout) {
                                                for ft in full_tracks {
                                                    let fid = ft.get("id").map(|i| i.to_string()).unwrap_or_default();
                                                    if let Some(target) = playlist_tracks.iter_mut().find(|pt| pt.id == fid) {
                                                        if let Some(ftitle) = ft.get("title").and_then(|x| x.as_str()) {
                                                            target.title = ftitle.to_string();
                                                        }
                                                        if let Some(fuser) = ft.get("user").and_then(|u| u.get("username")).and_then(|u| u.as_str()) {
                                                            target.artist = fuser.to_string();
                                                        }
                                                        if let Some(furl) = ft.get("permalink_url").and_then(|u| u.as_str()) {
                                                            target.url = furl.to_string();
                                                        }
                                                        if let Some(fdur) = ft.get("duration").and_then(|d| d.as_f64()) {
                                                            target.duration = fdur / 1000.0;
                                                        }
                                                        target.artwork_url = ft.get("artwork_url").and_then(|a| a.as_str()).map(|s| s.to_string());
                                                        target.waveform_url = ft.get("waveform_url").and_then(|w| w.as_str()).map(|s| s.to_string());
                                                        target.user_permalink = ft.get("user").and_then(|u| u.get("permalink")).and_then(|p| p.as_str()).map(|s| s.to_string());
                                                        target.user_id = ft.get("user").and_then(|u| u.get("id")).map(|i| i.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let track_count = playlist_tracks.len();
                            return SearchResult::PlaylistLoaded(crate::storage::PlaylistItem {
                                id: playlist_id,
                                title,
                                author,
                                track_count,
                                url: full_url,
                                tracks: playlist_tracks,
                            });
                        } else if kind == "track" {
                            let id = v.get("id").map(|i| i.to_string()).unwrap_or_default();
                            let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("Unknown").to_string();
                            let artist = v.get("user").and_then(|u| u.get("username")).and_then(|u| u.as_str()).unwrap_or("SoundCloud").to_string();
                            let url = v.get("permalink_url").and_then(|u| u.as_str()).unwrap_or(&full_url).to_string();
                            let duration = v.get("duration").and_then(|d| d.as_f64()).map(|ms| ms / 1000.0).unwrap_or(0.0);
                            let artwork_url = v.get("artwork_url").and_then(|a| a.as_str()).map(|s| s.to_string());
                            let waveform_url = v.get("waveform_url").and_then(|w| w.as_str()).map(|s| s.to_string());
                            let user_permalink = v.get("user").and_then(|u| u.get("permalink")).and_then(|p| p.as_str()).map(|s| s.to_string());
                            let user_id = v.get("user").and_then(|u| u.get("id")).map(|i| i.to_string());

                            return SearchResult::Success(vec![TrackItem {
                                id,
                                title,
                                artist,
                                duration,
                                url,
                                stream_url: None,
                                artwork_url,
                                waveform_url,
                                user_permalink,
                                user_id,
                            }]);
                        }
                    }
                }
            }
        }

        // Fast path: if not a direct URL, query SoundCloud api-v2 via curl
        if !is_direct_url {
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
                                    .get("publisher_metadata")
                                    .and_then(|pm| pm.get("artist"))
                                    .and_then(|a| a.as_str())
                                    .filter(|s| !s.trim().is_empty())
                                    .or_else(|| item.get("user").and_then(|u| u.get("username")).and_then(|u| u.as_str()))
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
                                let artwork_url = item
                                    .get("artwork_url")
                                    .and_then(|a| a.as_str())
                                    .map(|s| s.to_string());
                                let waveform_url = item
                                    .get("waveform_url")
                                    .and_then(|w| w.as_str())
                                    .map(|s| s.to_string());
                                let user_permalink = item
                                    .get("user")
                                    .and_then(|u| u.get("permalink"))
                                    .and_then(|p| p.as_str())
                                    .map(|s| s.to_string());
                                let user_id = item
                                    .get("user")
                                    .and_then(|u| u.get("id"))
                                    .map(|i| i.to_string());

                                let transcoding_url = item
                                    .get("media")
                                    .and_then(|m| m.get("transcodings"))
                                    .and_then(|t| t.as_array())
                                    .and_then(|arr| arr.first())
                                    .and_then(|t| t.get("url"))
                                    .and_then(|u| u.as_str())
                                    .map(|s| s.to_string());

                                if !url.is_empty() && !title.is_empty() {
                                    tracks.push(TrackItem {
                                        id,
                                        title,
                                        artist,
                                        duration,
                                        url,
                                        stream_url: transcoding_url,
                                        artwork_url,
                                        waveform_url,
                                        user_permalink,
                                        user_id,
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
                                artwork_url: None,
                                waveform_url: None,
                                user_permalink: None,
                                user_id: None,
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

    fn execute_fetch_user(user_query: &str, cached_cid: &Arc<Mutex<Option<String>>>) -> SearchResult {
        let cid = {
            let guard = cached_cid.lock().unwrap();
            guard.clone().unwrap_or_else(|| "tHVbSbDP4TaoMbPppngv6Tdry83vNbMQ".to_string())
        };

        // If user_query is a permalink or URL or user ID
        let resolve_url = if user_query.starts_with("http") {
            format!("https://api-v2.soundcloud.com/resolve?url={}&client_id={}", user_query, cid)
        } else if user_query.chars().all(|c| c.is_ascii_digit()) {
            format!("https://api-v2.soundcloud.com/users/{}?client_id={}", user_query, cid)
        } else {
            format!("https://api-v2.soundcloud.com/resolve?url=https://soundcloud.com/{}&client_id={}", user_query, cid)
        };

        let output = Command::new("curl")
            .arg("-s")
            .arg("-m")
            .arg("6")
            .arg("-H")
            .arg("User-Agent: Mozilla/5.0")
            .arg(&resolve_url)
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    let user_id = v.get("id").map(|i| i.to_string()).unwrap_or_default();
                    let username = v.get("username").and_then(|u| u.as_str()).unwrap_or("Unknown").to_string();
                    let permalink = v.get("permalink").and_then(|p| p.as_str()).unwrap_or("").to_string();
                    let avatar_url = v.get("avatar_url").and_then(|a| a.as_str()).map(|s| s.to_string());
                    let description = v.get("description").and_then(|d| d.as_str()).map(|s| s.to_string());
                    let followers_count = v.get("followers_count").and_then(|f| f.as_u64()).unwrap_or(0);
                    let track_count = v.get("track_count").and_then(|t| t.as_u64()).unwrap_or(0) as usize;

                    if !user_id.is_empty() {
                        // Fetch user's tracks
                        let tracks_url = format!("https://api-v2.soundcloud.com/users/{}/tracks?client_id={}&limit=30", user_id, cid);
                        let tracks_out = Command::new("curl")
                            .arg("-s")
                            .arg("-m")
                            .arg("6")
                            .arg("-H")
                            .arg("User-Agent: Mozilla/5.0")
                            .arg(&tracks_url)
                            .output();

                        let mut user_tracks = Vec::new();
                        if let Ok(t_out) = tracks_out {
                            if t_out.status.success() {
                                if let Ok(tv) = serde_json::from_slice::<serde_json::Value>(&t_out.stdout) {
                                    if let Some(coll) = tv.get("collection").and_then(|c| c.as_array()) {
                                        for t in coll {
                                            let tid = t.get("id").map(|i| i.to_string()).unwrap_or_default();
                                            let title = t.get("title").and_then(|x| x.as_str()).unwrap_or("Unknown").to_string();
                                            let artist = t.get("user").and_then(|u| u.get("username")).and_then(|u| u.as_str()).unwrap_or(&username).to_string();
                                            let url = t.get("permalink_url").and_then(|u| u.as_str()).unwrap_or("").to_string();
                                            let duration = t.get("duration").and_then(|d| d.as_f64()).map(|ms| ms / 1000.0).unwrap_or(0.0);
                                            let artwork = t.get("artwork_url").and_then(|a| a.as_str()).map(|s| s.to_string());
                                            let waveform = t.get("waveform_url").and_then(|w| w.as_str()).map(|s| s.to_string());

                                            if !tid.is_empty() && !title.is_empty() {
                                                user_tracks.push(TrackItem {
                                                    id: tid,
                                                    title,
                                                    artist,
                                                    duration,
                                                    url,
                                                    stream_url: None,
                                                    artwork_url: artwork,
                                                    waveform_url: waveform,
                                                    user_permalink: Some(permalink.clone()),
                                                    user_id: Some(user_id.clone()),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        return SearchResult::UserProfileLoaded(UserProfile {
                            id: user_id,
                            username,
                            permalink,
                            avatar_url,
                            description,
                            followers_count,
                            track_count: if user_tracks.is_empty() { track_count } else { user_tracks.len() },
                            tracks: user_tracks,
                        });
                    }
                }
            }
        }

        SearchResult::Error(format!("Could not load user profile for '{}'", user_query))
    }
}
