use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DISCORD_CLIENT_ID: &str = "1328005886616469605"; // soundcloud-tui / music rich presence client id

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

pub struct DiscordRpc {
    tx: Option<Sender<RpcCommand>>,
}

impl DiscordRpc {
    pub fn new() -> Self {
        let (tx, rx) = channel::<RpcCommand>();

        thread::spawn(move || {
            let mut stream: Option<UnixStream> = None;

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
                            stream = Self::connect_ipc();
                        }

                        if let Some(ref mut s) = stream {
                            let state_str = if is_paused {
                                format!("{} (paused)", artist)
                            } else {
                                artist
                            };

                            let large_img = artwork_url
                                .unwrap_or_else(|| "https://a-v2.sndcdn.com/assets/images/brand/icon/favicon-8c5e634e.ico".to_string());

                            let mut activity = serde_json::json!({
                                "details": title,
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
                            }
                        }
                    }
                    RpcCommand::Clear => {
                        if let Some(ref mut s) = stream {
                            let payload = serde_json::json!({
                                "cmd": "SET_ACTIVITY",
                                "args": {
                                    "pid": std::process::id(),
                                    "activity": null
                                },
                                "nonce": "sctui-clear"
                            });
                            let _ = Self::send_ipc(s, 1, &payload);
                        }
                    }
                }
            }
        });

        Self { tx: Some(tx) }
    }

    fn find_socket() -> Option<PathBuf> {
        let run_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
            let uid = std::process::id(); // fallback
            format!("/run/user/{}", uid)
        });

        let dirs = [
            PathBuf::from(run_dir),
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

    fn connect_ipc() -> Option<UnixStream> {
        let sock_path = Self::find_socket()?;
        let mut stream = UnixStream::connect(sock_path).ok()?;
        stream.set_write_timeout(Some(Duration::from_millis(200))).ok();
        stream.set_read_timeout(Some(Duration::from_millis(200))).ok();

        // Handshake: opcode 0
        let handshake = serde_json::json!({
            "v": 1,
            "client_id": DISCORD_CLIENT_ID
        });

        if Self::send_ipc(&mut stream, 0, &handshake).is_ok() {
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
