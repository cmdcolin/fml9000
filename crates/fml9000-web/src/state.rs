use fml9000_core::{AudioPlayer, MediaItem};
use rodio::Decoder;
use std::fs::File;
use std::io::BufReader;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;

#[derive(Clone, serde_derive::Serialize)]
pub struct PlaybackState {
    pub playing: bool,
    pub paused: bool,
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
    pub current_index: Option<usize>,
    pub current_track: Option<TrackInfo>,
    pub shuffle_enabled: bool,
    pub repeat_mode: String,
    pub volume: f32,
}

#[derive(Clone, serde_derive::Serialize)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_str: String,
}

pub enum AudioCommand {
    PlayFile {
        path: String,
        respond: mpsc::Sender<Result<PlayFileResult, String>>,
    },
    Pause,
    Resume,
    Stop,
    Seek(f64),
    SetVolume(f32),
    GetState(mpsc::Sender<AudioThreadState>),
}

pub struct PlayFileResult {
    pub _duration_secs: Option<f64>,
}

pub struct AudioThreadState {
    pub playing: bool,
    pub paused: bool,
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
    pub volume: f32,
}

fn run_audio_thread(rx: mpsc::Receiver<AudioCommand>) {
    let (audio, err) = AudioPlayer::new();
    if let Some(e) = err {
        eprintln!("Audio init warning: {e}");
    }
    let mut volume = 1.0f32;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCommand::PlayFile { path, respond } => {
                let file = match File::open(&path) {
                    Ok(f) => BufReader::new(f),
                    Err(e) => {
                        let _ = respond.send(Err(format!("Cannot open file: {e}")));
                        continue;
                    }
                };
                let source = match Decoder::new(file) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = respond.send(Err(format!("Cannot decode file: {e}")));
                        continue;
                    }
                };
                use rodio::Source;
                let duration = source.total_duration();
                audio.play_source(source, duration);
                audio.set_volume(volume);
                let _ = respond.send(Ok(PlayFileResult {
                    _duration_secs: duration.map(|d| d.as_secs_f64()),
                }));
            }
            AudioCommand::Pause => audio.pause(),
            AudioCommand::Resume => audio.play(),
            AudioCommand::Stop => audio.stop(),
            AudioCommand::Seek(pos) => audio.try_seek(Duration::from_secs_f64(pos)),
            AudioCommand::SetVolume(v) => {
                volume = v;
                audio.set_volume(v);
            }
            AudioCommand::GetState(respond) => {
                let _ = respond.send(AudioThreadState {
                    playing: audio.is_playing(),
                    paused: audio.is_paused(),
                    position_secs: audio.get_pos().as_secs_f64(),
                    duration_secs: audio.get_duration().map(|d| d.as_secs_f64()),
                    volume,
                });
            }
        }
    }
}

pub struct AppState {
    audio_tx: mpsc::Sender<AudioCommand>,
    pub playlist_items: RwLock<Vec<MediaItem>>,
    pub current_index: RwLock<Option<usize>>,
    pub shuffle_enabled: AtomicBool,
    pub repeat_mode: Mutex<fml9000_core::RepeatMode>,
    pub ws_broadcast: broadcast::Sender<String>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let (audio_tx, audio_rx) = mpsc::channel();
        std::thread::spawn(move || run_audio_thread(audio_rx));

        let (ws_tx, _) = broadcast::channel(64);

        Arc::new(Self {
            audio_tx,
            playlist_items: RwLock::new(Vec::new()),
            current_index: RwLock::new(None),
            shuffle_enabled: AtomicBool::new(false),
            repeat_mode: Mutex::new(fml9000_core::RepeatMode::All),
            ws_broadcast: ws_tx,
        })
    }

    pub fn send_audio(&self, cmd: AudioCommand) {
        let _ = self.audio_tx.send(cmd);
    }

    pub fn get_audio_state(&self) -> AudioThreadState {
        let (tx, rx) = mpsc::channel();
        self.send_audio(AudioCommand::GetState(tx));
        rx.recv().unwrap_or(AudioThreadState {
            playing: false,
            paused: false,
            position_secs: 0.0,
            duration_secs: None,
            volume: 1.0,
        })
    }

    pub fn play_file(&self, path: &str) -> Result<PlayFileResult, String> {
        let (tx, rx) = mpsc::channel();
        self.send_audio(AudioCommand::PlayFile {
            path: path.to_string(),
            respond: tx,
        });
        rx.recv().map_err(|e| format!("Audio thread error: {e}"))?
    }

    pub fn get_playback_state(&self) -> PlaybackState {
        let audio = self.get_audio_state();
        let current_index = *self.current_index.read().unwrap();
        let items = self.playlist_items.read().unwrap();
        let current_track = current_index.and_then(|idx| {
            items.get(idx).map(|item| TrackInfo {
                title: item.title().to_string(),
                artist: item.artist().to_string(),
                album: item.album().to_string(),
                duration_str: item.duration_str(),
            })
        });
        let repeat_mode = *self.repeat_mode.lock().unwrap();

        PlaybackState {
            playing: audio.playing,
            paused: audio.paused,
            position_secs: audio.position_secs,
            duration_secs: audio.duration_secs,
            current_index,
            current_track,
            shuffle_enabled: self.shuffle_enabled.load(Ordering::Relaxed),
            repeat_mode: format!("{:?}", repeat_mode).to_lowercase(),
            volume: audio.volume,
        }
    }

    pub fn broadcast_state(&self) {
        let state = self.get_playback_state();
        if let Ok(json) = serde_json::to_string(&serde_json::json!({
            "type": "playback_state",
            "data": state,
        })) {
            let _ = self.ws_broadcast.send(json);
        }
    }
}
