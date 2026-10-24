use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Official SoundCloud Application Client ID in Discord
const DISCORD_CLIENT_ID: &str = "802958833214423081";

pub enum RpcCommand {
    Update {
        title: String,
        artist: String,
        artwork_url: Option<String>,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        is_paused: bool,
    },
    Clear,
}

struct LastSentState {
    title: String,
    artist: String,
    is_paused: bool,
    start_timestamp: Option<u64>,
    end_timestamp: Option<u64>,
    sent_at: Instant,
}

pub struct DiscordRpc {
    tx: Option<Sender<RpcCommand>>,
}

impl DiscordRpc {
    pub fn new(custom_client_id: Option<String>) -> Self {
        let (tx, rx) = channel::<RpcCommand>();
        let client_id = custom_client_id.unwrap_or_else(|| DISCORD_CLIENT_ID.to_string());

        thread::spawn(move || {
            let mut stream: Option<UnixStream> = None;
            let mut last_state: Option<LastSentState> = None;

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    RpcCommand::Update {
                        title,
                        artist,
                        artwork_url,
                        start_timestamp,
                        end_timestamp,
                        is_paused,
                    } => {
                        if stream.is_none() {
                            stream = Self::connect_ipc(&client_id);
                            last_state = None;
                        }

                        // Avoid spamming Discord IPC every 1s (Discord rate-limits SET_ACTIVITY to 5 per 20s).
                        // Only send when track/pause state changes, user seeks (>3s drift), or 15s keepalive elapsed.
                        if let Some(ref prev) = last_state {
                            let same_track = prev.title == title
                                && prev.artist == artist
                                && prev.is_paused == is_paused
                                && prev.end_timestamp.is_some() == end_timestamp.is_some();

                            let time_drift = match (prev.start_timestamp, start_timestamp) {
                                (Some(a), Some(b)) => a.abs_diff(b) > 3,
                                (None, None) => false,
                                _ => true,
                            };

                            if same_track && !time_drift && prev.sent_at.elapsed() < Duration::from_secs(15) {
                                continue;
                            }
                        }

                        if let Some(ref mut s) = stream {
                            let details_str = Self::pad_field(&title);
                            let raw_state = if is_paused {
                                format!("{} (paused)", artist)
                            } else {
                                artist.clone()
                            };
                            let state_str = Self::pad_field(&raw_state);

                            let large_img = artwork_url
                                .unwrap_or_else(|| "https://a-v2.sndcdn.com/assets/images/brand/icon/favicon-8c5e634e.ico".to_string());

                            let mut activity = serde_json::json!({
                                "type": 2,
                                "details": details_str,
                                "state": state_str,
                                "assets": {
                                    "large_image": large_img,
                                    "large_text": "SoundCloud TUI",
                                    "small_image": if is_paused { "pause" } else { "play" },
                                    "small_text": if is_paused { "Paused" } else { "Playing" }
                                }
                            });

                            if !is_paused {
                                if let (Some(start), Some(end)) = (start_timestamp, end_timestamp) {
                                    if end > start {
                                        activity["timestamps"] = serde_json::json!({
                                            "start": start,
                                            "end": end
                                        });
                                    }
                                }
                            }

                            let payload = serde_json::json!({
                                "cmd": "SET_ACTIVITY",
                                "args": {
                                    "pid": std::process::id(),
                                    "activity": activity
                                },
                                "nonce": "sctui-nonce"
                            });

                            if Self::send_ipc(s, 1, &payload).is_err() {
                                stream = None;
                                last_state = None;
                            } else {
                                let _ = Self::read_ipc(s);
                                last_state = Some(LastSentState {
                                    title,
                                    artist,
                                    is_paused,
                                    start_timestamp,
                                    end_timestamp,
                                    sent_at: Instant::now(),
                                });
                            }
                        }
                    }
                    RpcCommand::Clear => {
                        if last_state.is_none() {
                            continue;
                        }
                        if let Some(ref mut s) = stream {
                            let payload = serde_json::json!({
                                "cmd": "SET_ACTIVITY",
                                "args": {
                                    "pid": std::process::id(),
                                    "activity": null
                                },
                                "nonce": "sctui-clear"
                            });
                            if Self::send_ipc(s, 1, &payload).is_ok() {
                                let _ = Self::read_ipc(s);
                            } else {
                                stream = None;
                            }
                        }
                        last_state = None;
                    }
                }
            }
        });

        Self { tx: Some(tx) }
    }

    fn pad_field(s: &str) -> String {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            "  ".to_string()
        } else if trimmed.chars().count() < 2 {
            format!("{} ", trimmed)
        } else {
            trimmed.chars().take(128).collect()
        }
    }

    fn find_socket() -> Option<PathBuf> {
        let run_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| "/run/user/1000".to_string());

        let base_run = PathBuf::from(&run_dir);
        let dirs = [
            base_run.clone(),
            base_run.join("app/com.discordapp.Discord"),
            base_run.join("app/dev.vencord.Vesktop"),
            base_run.join("snap.discord"),
            PathBuf::from("/tmp"),
            std::env::temp_dir(),
        ];

        for dir in &dirs {
            for i in 0..10 {
                let p = dir.join(format!("discord-ipc-{}", i));
                if p.exists() {
                    return Some(p);
                }
            }
        }
        None
    }

    fn connect_ipc(client_id: &str) -> Option<UnixStream> {
        let sock_path = Self::find_socket()?;
        let mut stream = UnixStream::connect(sock_path).ok()?;
        stream.set_write_timeout(Some(Duration::from_millis(500))).ok();
        stream.set_read_timeout(Some(Duration::from_millis(500))).ok();

        // Handshake: opcode 0
        let handshake = serde_json::json!({
            "v": 1,
            "client_id": client_id
        });

        if Self::send_ipc(&mut stream, 0, &handshake).is_ok() {
            // Read READY response from Discord so handshake completes cleanly
            let _ = Self::read_ipc(&mut stream);
            Some(stream)
        } else {
            None
        }
    }

    fn send_ipc(stream: &mut UnixStream, opcode: u32, payload: &serde_json::Value) -> Result<(), std::io::Error> {
        let json_bytes = serde_json::to_vec(payload)?;
        let length = json_bytes.len() as u32;

        let mut header = [0u8; 8];
        header[0..4].copy_from_slice(&opcode.to_le_bytes());
        header[4..8].copy_from_slice(&length.to_le_bytes());

        stream.write_all(&header)?;
        stream.write_all(&json_bytes)?;
        stream.flush()?;
        Ok(())
    }

    fn read_ipc(stream: &mut UnixStream) -> Result<(u32, Vec<u8>), std::io::Error> {
        let mut header = [0u8; 8];
        stream.read_exact(&mut header)?;
        let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if length > 65536 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "IPC frame too large"));
        }
        let mut buf = vec![0u8; length];
        stream.read_exact(&mut buf)?;
        Ok((opcode, buf))
    }

    pub fn update(
        &self,
        title: &str,
        artist: &str,
        artwork_url: Option<String>,
        current_pos: f64,
        total_dur: f64,
        is_paused: bool,
    ) {
        if let Some(ref tx) = self.tx {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let start = if !is_paused && current_pos >= 0.0 {
                Some(now.saturating_sub(current_pos as u64))
            } else {
                None
            };
            let end = if !is_paused && total_dur > 0.0 && current_pos < total_dur {
                let remaining = (total_dur - current_pos) as u64;
                Some(now + remaining)
            } else {
                None
            };

            let _ = tx.send(RpcCommand::Update {
                title: title.to_string(),
                artist: artist.to_string(),
                artwork_url,
                start_timestamp: start,
                end_timestamp: end,
                is_paused,
            });
        }
    }

    pub fn clear(&self) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(RpcCommand::Clear);
        }
    }
}
