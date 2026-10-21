use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PlayerEvent {
    TimePos(f64),
    Duration(f64),
    Paused(bool),
    Idle(bool),
    Volume(f64),
    PlaybackEnded,
    PlaybackError(String),
}

pub struct AudioPlayer {
    _process: Option<Child>,
    ipc_path: String,
    event_rx: Receiver<PlayerEvent>,
    #[allow(dead_code)]
    is_running: Arc<AtomicBool>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let ipc_path = format!("/tmp/sctui_mpv_{}_{}.sock", std::process::id(), timestamp);

        let (tx, rx) = channel();
        let is_running = Arc::new(AtomicBool::new(false));

        // Start mpv in idle background mode with lossless streaming & IPC (completely headless)
        let process = Command::new("mpv")
            .arg("--idle=yes")
            .arg("--no-video")
            .arg("--force-window=no")
            .arg("--vo=null")
            .arg("--msg-level=all=no")
            .arg("--ytdl-format=bestaudio/best")
            .arg("--user-agent=Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
            .arg("--hls-bitrate=max")
            .arg("--audio-pitch-correction=no")
            .arg("--demuxer-readahead-secs=15")
            .arg("--audio-buffer=0.5")
            .arg(format!("--input-ipc-server={}", ipc_path))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok();

        let running_clone = Arc::clone(&is_running);
        let ipc_clone = ipc_path.clone();

        // Spawn IPC event listener thread
        thread::spawn(move || {
            Self::ipc_listener_loop(ipc_clone, tx, running_clone);
        });

        Self {
            _process: process,
            ipc_path,
            event_rx: rx,
            is_running,
        }
    }

    fn ipc_listener_loop(ipc_path: String, tx: Sender<PlayerEvent>, is_running: Arc<AtomicBool>) {
        // Wait for socket to be ready
        for _ in 0..50 {
            if std::path::Path::new(&ipc_path).exists() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        let stream = match UnixStream::connect(&ipc_path) {
            Ok(s) => s,
            Err(_) => return,
        };

        is_running.store(true, Ordering::SeqCst);

        let mut stream_writer = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return,
        };

        // Subscribe to mpv properties
        let observes = [
            r#"{"command": ["observe_property", 1, "time-pos"]}"#,
            r#"{"command": ["observe_property", 2, "duration"]}"#,
            r#"{"command": ["observe_property", 3, "pause"]}"#,
            r#"{"command": ["observe_property", 4, "idle-active"]}"#,
            r#"{"command": ["observe_property", 5, "volume"]}"#,
        ];

        for obs in observes {
            let _ = writeln!(stream_writer, "{}", obs);
        }
        let _ = stream_writer.flush();

        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(event) = v.get("event").and_then(|e| e.as_str()) {
                    if event == "end-file" {
                        let reason = v.get("reason").and_then(|r| r.as_str()).unwrap_or("");
                        if reason == "eof" || reason == "stop" {
                            let _ = tx.send(PlayerEvent::PlaybackEnded);
                        } else if reason == "error" {
                            let err_msg = v.get("file_error").and_then(|e| e.as_str()).unwrap_or("Failed to load stream");
                            let _ = tx.send(PlayerEvent::PlaybackError(err_msg.to_string()));
                        }
                    }
                }

                if let Some(prop) = v.get("name").and_then(|p| p.as_str()) {
                    match prop {
                        "time-pos" => {
                            if let Some(pos) = v.get("data").and_then(|d| d.as_f64()) {
                                let _ = tx.send(PlayerEvent::TimePos(pos));
                            }
                        }
                        "duration" => {
                            if let Some(dur) = v.get("data").and_then(|d| d.as_f64()) {
                                let _ = tx.send(PlayerEvent::Duration(dur));
                            }
                        }
                        "pause" => {
                            if let Some(paused) = v.get("data").and_then(|d| d.as_bool()) {
                                let _ = tx.send(PlayerEvent::Paused(paused));
                            }
                        }
                        "idle-active" => {
                            if let Some(idle) = v.get("data").and_then(|d| d.as_bool()) {
                                let _ = tx.send(PlayerEvent::Idle(idle));
                            }
                        }
                        "volume" => {
                            if let Some(vol) = v.get("data").and_then(|d| d.as_f64()) {
                                let _ = tx.send(PlayerEvent::Volume(vol));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub fn send_command(&self, cmd: &str) {
        if let Ok(mut stream) = UnixStream::connect(&self.ipc_path) {
            let _ = writeln!(stream, "{}", cmd);
            let _ = stream.flush();
        }
    }

    pub fn play(&self, url: &str) {
        let msg = format!(r#"{{"command": ["loadfile", "{}", "replace"]}}"#, url);
        self.send_command(&msg);
        self.send_command(r#"{"command": ["set_property", "pause", false]}"#);
    }

    pub fn toggle_pause(&self) {
        let msg = r#"{"command": ["cycle", "pause"]}"#;
        self.send_command(msg);
    }

    pub fn seek(&self, seconds: f64) {
        let msg = format!(r#"{{"command": ["seek", {}, "relative"]}}"#, seconds);
        self.send_command(&msg);
    }

    pub fn set_volume(&self, vol: f64) {
        let msg = format!(r#"{{"command": ["set_property", "volume", {}]}}"#, vol);
        self.send_command(&msg);
    }

    pub fn poll_event(&self) -> Option<PlayerEvent> {
        self.event_rx.try_recv().ok()
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        let msg = r#"{"command": ["quit"]}"#;
        self.send_command(msg);
        let _ = std::fs::remove_file(&self.ipc_path);
    }
}
