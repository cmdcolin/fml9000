use fml9000_core::{
    compute_next_index, load_track_by_filename, load_video_by_id, mark_as_played,
    pop_queue_front, queue_len, update_play_stats, AudioPlayer, MediaItem, NextTrackResult,
};
use rodio::Decoder;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::process::{Command, Stdio};
use std::collections::HashMap;
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
        respond: mpsc::Sender<Result<f64, String>>,
    },
    PlayYouTube {
        video_id: String,
        duration_hint: Option<f64>,
        respond: mpsc::Sender<Result<f64, String>>,
    },
    Pause,
    Resume,
    Stop,
    Seek(f64),
    SetVolume(f32),
    GetState(mpsc::Sender<AudioThreadState>),
}

pub struct AudioThreadState {
    pub playing: bool,
    pub paused: bool,
    pub empty: bool,
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
    pub volume: f32,
}

fn download_youtube_audio(video_id: &str) -> Result<Vec<u8>, String> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let child = Command::new("yt-dlp")
        .args([
            "-f", "bestaudio",
            "-o", "-",
            "--no-warnings",
            "--quiet",
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run yt-dlp: {e}"))?;

    let output = child
        .wait_with_output()
        .map_err(|e| format!("yt-dlp failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp error: {stderr}"));
    }

    if output.stdout.is_empty() {
        return Err("yt-dlp returned no audio data".into());
    }

    Ok(output.stdout)
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
                let _ = respond.send(Ok(duration.map(|d| d.as_secs_f64()).unwrap_or(0.0)));
            }
            AudioCommand::PlayYouTube { video_id, duration_hint, respond } => {
                eprintln!("Downloading YouTube audio for {video_id}...");
                let data = match download_youtube_audio(&video_id) {
                    Ok(d) => d,
                    Err(e) => {
                        let _ = respond.send(Err(e));
                        continue;
                    }
                };
                eprintln!("Downloaded {} bytes, decoding...", data.len());
                let cursor = Cursor::new(data);
                let source = match Decoder::new(cursor) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = respond.send(Err(format!("Cannot decode YouTube audio: {e}")));
                        continue;
                    }
                };
                use rodio::Source;
                let duration = source.total_duration()
                    .or_else(|| duration_hint.map(|s| Duration::from_secs_f64(s)));
                audio.play_source(source, duration);
                audio.set_volume(volume);
                let dur_secs = duration.map(|d| d.as_secs_f64()).unwrap_or(0.0);
                let _ = respond.send(Ok(dur_secs));
            }
            AudioCommand::Pause => audio.pause(),
            AudioCommand::Resume => audio.play(),
            AudioCommand::Stop => {
                audio.stop();
                audio.clear_duration();
            }
            AudioCommand::Seek(pos) => audio.try_seek(Duration::from_secs_f64(pos)),
            AudioCommand::SetVolume(v) => {
                volume = v;
                audio.set_volume(v);
            }
            AudioCommand::GetState(respond) => {
                let _ = respond.send(AudioThreadState {
                    playing: audio.is_playing(),
                    paused: audio.is_paused(),
                    empty: audio.is_empty(),
                    position_secs: audio.get_pos().as_secs_f64(),
                    duration_secs: audio.get_duration().map(|d| d.as_secs_f64()),
                    volume,
                });
            }
        }
    }
}

enum PlayStats {
    None,
    Track {
        filename: String,
        duration_secs: f64,
        counted: bool,
    },
    Video {
        id: i32,
        duration_secs: f64,
        counted: bool,
    },
}

pub struct AppState {
    audio_tx: mpsc::Sender<AudioCommand>,
    pub playlist_items: RwLock<Vec<MediaItem>>,
    pub current_index: RwLock<Option<usize>>,
    pub shuffle_enabled: AtomicBool,
    pub repeat_mode: Mutex<fml9000_core::RepeatMode>,
    pub ws_broadcast: broadcast::Sender<String>,
    was_playing: AtomicBool,
    play_stats: Mutex<PlayStats>,
    pub volume: Mutex<f32>,
    pub new_video_counts: Mutex<HashMap<i32, usize>>,
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
            was_playing: AtomicBool::new(false),
            play_stats: Mutex::new(PlayStats::None),
            volume: Mutex::new(1.0),
            new_video_counts: Mutex::new(HashMap::new()),
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
            empty: true,
            position_secs: 0.0,
            duration_secs: None,
            volume: 1.0,
        })
    }

    pub fn play_file(&self, path: &str) -> Result<f64, String> {
        let (tx, rx) = mpsc::channel();
        self.send_audio(AudioCommand::PlayFile {
            path: path.to_string(),
            respond: tx,
        });
        rx.recv().map_err(|e| format!("Audio thread error: {e}"))?
    }

    pub fn play_youtube(&self, video_id: &str, duration_hint: Option<f64>) -> Result<f64, String> {
        let (tx, rx) = mpsc::channel();
        self.send_audio(AudioCommand::PlayYouTube {
            video_id: video_id.to_string(),
            duration_hint,
            respond: tx,
        });
        // YouTube download can take a while — use a generous timeout
        rx.recv_timeout(Duration::from_secs(120))
            .map_err(|e| format!("YouTube playback timeout: {e}"))?
    }

    pub fn play_index(&self, index: usize) -> Result<(), String> {
        let items = self.playlist_items.read().unwrap();
        let item = items.get(index).ok_or("Index out of range")?.clone();
        drop(items);

        match &item {
            MediaItem::Track(track) => {
                let duration_secs = self.play_file(&track.filename)?;
                *self.current_index.write().unwrap() = Some(index);
                self.was_playing.store(true, Ordering::Relaxed);

                *self.play_stats.lock().unwrap() = PlayStats::Track {
                    filename: track.filename.clone(),
                    duration_secs,
                    counted: false,
                };

                let item_clone = item.clone();
                std::thread::spawn(move || mark_as_played(&item_clone));

                self.broadcast_state();
                Ok(())
            }
            MediaItem::Video(video) => {
                let duration_hint = video.duration_seconds.map(|s| s as f64);
                let duration_secs = self.play_youtube(&video.video_id, duration_hint)?;
                *self.current_index.write().unwrap() = Some(index);
                self.was_playing.store(true, Ordering::Relaxed);

                let effective_duration = if duration_secs > 0.0 {
                    duration_secs
                } else {
                    duration_hint.unwrap_or(0.0)
                };

                *self.play_stats.lock().unwrap() = PlayStats::Video {
                    id: video.id,
                    duration_secs: effective_duration,
                    counted: false,
                };

                let item_clone = item.clone();
                std::thread::spawn(move || mark_as_played(&item_clone));

                self.broadcast_state();
                Ok(())
            }
        }
    }

    pub fn play_next(&self) {
        if queue_len() > 0 {
            if let Some(queue_item) = pop_queue_front() {
                let items = self.playlist_items.read().unwrap();
                for (i, playlist_item) in items.iter().enumerate() {
                    let matches = match (&queue_item, playlist_item) {
                        (MediaItem::Track(a), MediaItem::Track(b)) => a.filename == b.filename,
                        (MediaItem::Video(a), MediaItem::Video(b)) => a.id == b.id,
                        _ => false,
                    };
                    if matches {
                        drop(items);
                        let _ = self.play_index(i);
                        return;
                    }
                }
            }
        }

        let current_index = *self.current_index.read().unwrap();
        let playlist_len = self.playlist_items.read().unwrap().len();
        let shuffle = self.shuffle_enabled.load(Ordering::Relaxed);
        let repeat = *self.repeat_mode.lock().unwrap();

        match compute_next_index(current_index, playlist_len, shuffle, repeat) {
            NextTrackResult::PlayIndex(idx) => {
                let _ = self.play_index(idx);
            }
            NextTrackResult::Stop => {
                self.send_audio(AudioCommand::Stop);
                *self.current_index.write().unwrap() = None;
                self.was_playing.store(false, Ordering::Relaxed);
                *self.play_stats.lock().unwrap() = PlayStats::None;
                self.broadcast_state();
            }
        }
    }

    pub fn tick(&self) {
        let audio = self.get_audio_state();

        // Check play stats (50% threshold)
        if let Some(duration_secs) = audio.duration_secs {
            if duration_secs > 0.0 {
                let mut stats = self.play_stats.lock().unwrap();
                match &mut *stats {
                    PlayStats::Track {
                        filename,
                        duration_secs: dur,
                        counted,
                    } => {
                        if !*counted && *dur > 0.0 && audio.position_secs >= *dur * 0.5 {
                            *counted = true;
                            let fname = filename.clone();
                            std::thread::spawn(move || {
                                if let Some(track) = load_track_by_filename(&fname) {
                                    update_play_stats(&MediaItem::Track(track));
                                }
                            });
                        }
                    }
                    PlayStats::Video {
                        id,
                        duration_secs: dur,
                        counted,
                    } => {
                        if !*counted && *dur > 0.0 && audio.position_secs >= *dur * 0.5 {
                            *counted = true;
                            let vid_id = *id;
                            std::thread::spawn(move || {
                                if let Some(video) = load_video_by_id(vid_id) {
                                    update_play_stats(&MediaItem::Video(video));
                                }
                            });
                        }
                    }
                    PlayStats::None => {}
                }
            }
        }

        // Auto-advance: detect track finished
        if self.was_playing.load(Ordering::Relaxed) && audio.empty && !audio.paused {
            self.was_playing.store(false, Ordering::Relaxed);
            self.play_next();
        } else if audio.playing {
            self.was_playing.store(true, Ordering::Relaxed);
        }
    }

    pub fn stop(&self) {
        self.send_audio(AudioCommand::Stop);
        *self.current_index.write().unwrap() = None;
        self.was_playing.store(false, Ordering::Relaxed);
        *self.play_stats.lock().unwrap() = PlayStats::None;
        self.broadcast_state();
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

    pub fn save_settings(&self) {
        let shuffle = self.shuffle_enabled.load(Ordering::Relaxed);
        let repeat = *self.repeat_mode.lock().unwrap();
        let volume = *self.volume.lock().unwrap();

        let mut settings: fml9000_core::CoreSettings = fml9000_core::settings::read_settings();
        settings.shuffle_enabled = shuffle;
        settings.repeat_mode = repeat;
        settings.volume = volume as f64;

        if let Err(e) = fml9000_core::settings::write_settings(&settings) {
            eprintln!("Failed to save settings: {e}");
        }
    }
}
