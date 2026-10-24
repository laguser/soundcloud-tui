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
    FileLoaded,
    Buffering(bool),
}

pub struct AudioPlayer {
    _process: Option<Child>,
    ipc_path: String,
    event_rx: Receiver<PlayerEvent>,
    cmd_tx: Sender<String>,
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
        let (cmd_tx, cmd_rx) = channel::<String>();
        let is_running = Arc::new(AtomicBool::new(false));

        // Start mpv in idle background mode with direct low-latency streaming & IPC
        let process = Command::new("mpv")
            .arg("--idle=yes")
            .arg("--no-config")
            .arg("--no-video")
            .arg("--force-window=no")
            .arg("--vo=null")
            .arg("--msg-level=all=no")
            .arg("--no-ytdl")
            .arg("--user-agent=Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
            .arg("--hls-bitrate=max")
            .arg("--audio-pitch-correction=no")
            .arg("--demuxer-lavf-o=allowed_extensions=ALL")
            .arg("--stream-lavf-o=reconnect=1,reconnect_streamed=1,reconnect_delay_max=4")
            .arg("--network-timeout=10")
            .arg("--demuxer-max-bytes=150M")
            .arg("--demuxer-max-back-bytes=100M")
            .arg("--demuxer-readahead-secs=300")
            .arg("--cache=yes")
            .arg("--cache-secs=300")
            .arg("--cache-pause=no")
            .arg("--stream-buffer-size=512k")
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
            Self::ipc_listener_loop(ipc_clone, tx, cmd_rx, running_clone);
        });

        Self {
            _process: process,
            ipc_path,
            event_rx: rx,
            cmd_tx,
            is_running,
        }
    }

    fn ipc_listener_loop(ipc_path: String, tx: Sender<PlayerEvent>, cmd_rx: Receiver<String>, is_running: Arc<AtomicBool>) {
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
            r#"{"command": ["observe_property", 6, "paused-for-cache"]}"#,
        ];

        for obs in observes {
            let _ = writeln!(stream_writer, "{}", obs);
        }
        let _ = stream_writer.flush();

        // Spawn dedicated command writer on the persistent stream
        let running_writer = Arc::clone(&is_running);
        thread::spawn(move || {
            while let Ok(cmd) = cmd_rx.recv() {
                if writeln!(stream_writer, "{}", cmd).is_err() || stream_writer.flush().is_err() {
                    running_writer.store(false, Ordering::SeqCst);
                    break;
                }
            }
        });

        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(event) = v.get("event").and_then(|e| e.as_str()) {
                    if event == "file-loaded" {
                        let _ = tx.send(PlayerEvent::FileLoaded);
                    } else if event == "end-file" {
                        let reason = v.get("reason").and_then(|r| r.as_str()).unwrap_or("");
                        if reason == "eof" {
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
                        "paused-for-cache" => {
                            if let Some(buffering) = v.get("data").and_then(|d| d.as_bool()) {
                                let _ = tx.send(PlayerEvent::Buffering(buffering));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub fn ensure_running(&mut self) {
        if !self.is_running.load(Ordering::SeqCst) {
            let old_ipc = self.ipc_path.clone();
            *self = Self::new();
            let _ = std::fs::remove_file(old_ipc);
        }
    }

    pub fn send_command(&self, cmd: &str) {
        let _ = self.cmd_tx.send(cmd.to_string());
    }

    pub fn play(&self, url: &str) {
        let cmd = serde_json::json!({
            "command": ["loadfile", url, "replace"]
        });
        if let Ok(msg) = serde_json::to_string(&cmd) {
            self.send_command(&msg);
        }
        self.send_command(r#"{"command": ["set_property", "pause", false]}"#);
    }

    pub fn set_pause(&self, paused: bool) {
        let cmd = serde_json::json!({
            "command": ["set_property", "pause", paused]
        });
        if let Ok(msg) = serde_json::to_string(&cmd) {
            self.send_command(&msg);
        }
    }

    #[allow(dead_code)]
    pub fn toggle_pause(&self) {
        let msg = r#"{"command": ["cycle", "pause"]}"#;
        self.send_command(msg);
    }

    pub fn seek(&self, seconds: f64) {
        let cmd = serde_json::json!({
            "command": ["seek", seconds, "relative"]
        });
        if let Ok(msg) = serde_json::to_string(&cmd) {
            self.send_command(&msg);
        }
    }

    pub fn seek_to(&self, seconds: f64) {
        let cmd = serde_json::json!({
            "command": ["seek", seconds, "absolute"]
        });
        if let Ok(msg) = serde_json::to_string(&cmd) {
            self.send_command(&msg);
        }
    }

    pub fn set_speed(&self, speed: f64) {
        let cmd = serde_json::json!({
            "command": ["set_property", "speed", speed]
        });
        if let Ok(msg) = serde_json::to_string(&cmd) {
            self.send_command(&msg);
        }
    }

    pub fn set_pitch_correction(&self, keep_pitch: bool) {
        let cmd = serde_json::json!({
            "command": ["set_property", "audio-pitch-correction", keep_pitch]
        });
        if let Ok(msg) = serde_json::to_string(&cmd) {
            self.send_command(&msg);
        }
    }

    pub fn set_audio_filter(&self, filter_str: &str) {
        let cmd = serde_json::json!({
            "command": ["set_property", "af", filter_str]
        });
        if let Ok(msg) = serde_json::to_string(&cmd) {
            self.send_command(&msg);
        }
    }

    pub fn set_volume(&self, vol: f64) {
        let cmd = serde_json::json!({
            "command": ["set_property", "volume", vol]
        });
        if let Ok(msg) = serde_json::to_string(&cmd) {
            self.send_command(&msg);
        }
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
