use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::state::{AppState, AudioCommand};

#[derive(serde_derive::Deserialize)]
pub struct PlayRequest {
    pub index: usize,
}

#[derive(serde_derive::Deserialize)]
pub struct SeekRequest {
    pub position_secs: f64,
}

#[derive(serde_derive::Deserialize)]
pub struct VolumeRequest {
    pub volume: f32,
}

pub async fn play(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PlayRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state
        .play_index(req.index)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn pause(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    state.send_audio(AudioCommand::Pause);
    state.broadcast_state();
    Json(serde_json::json!({"ok": true}))
}

pub async fn resume(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    state.send_audio(AudioCommand::Resume);
    state.broadcast_state();
    Json(serde_json::json!({"ok": true}))
}

pub async fn stop(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    state.stop();
    Json(serde_json::json!({"ok": true}))
}

pub async fn next(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    state.play_next();
    Json(serde_json::json!({"ok": true}))
}

pub async fn prev(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let current_index = *state.current_index.read().unwrap();
    let playlist_len = state.playlist_items.read().unwrap().len();

    match fml9000_core::compute_prev_index(current_index, playlist_len) {
        fml9000_core::NextTrackResult::PlayIndex(idx) => {
            state
                .play_index(idx)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(Json(serde_json::json!({"ok": true, "index": idx})))
        }
        fml9000_core::NextTrackResult::Stop => {
            state.stop();
            Ok(Json(serde_json::json!({"ok": true, "stopped": true})))
        }
    }
}

pub async fn seek(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SeekRequest>,
) -> Json<serde_json::Value> {
    state.send_audio(AudioCommand::Seek(req.position_secs));
    state.broadcast_state();
    Json(serde_json::json!({"ok": true}))
}

pub async fn volume(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VolumeRequest>,
) -> Json<serde_json::Value> {
    state.send_audio(AudioCommand::SetVolume(req.volume));
    *state.volume.lock().unwrap() = req.volume;
    state.save_settings();
    state.broadcast_state();
    Json(serde_json::json!({"ok": true}))
}

pub async fn get_state(State(state): State<Arc<AppState>>) -> Json<crate::state::PlaybackState> {
    Json(state.get_playback_state())
}

#[derive(serde_derive::Deserialize)]
pub struct ShuffleRequest {
    pub enabled: bool,
}

pub async fn set_shuffle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ShuffleRequest>,
) -> Json<serde_json::Value> {
    state.shuffle_enabled.store(req.enabled, Ordering::Relaxed);
    state.save_settings();
    state.broadcast_state();
    Json(serde_json::json!({"ok": true}))
}

#[derive(serde_derive::Deserialize)]
pub struct RepeatRequest {
    pub mode: String,
}

pub async fn set_repeat(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RepeatRequest>,
) -> Json<serde_json::Value> {
    let mode = match req.mode.as_str() {
        "off" => fml9000_core::RepeatMode::Off,
        "one" => fml9000_core::RepeatMode::One,
        _ => fml9000_core::RepeatMode::All,
    };
    *state.repeat_mode.lock().unwrap() = mode;
    state.save_settings();
    state.broadcast_state();
    Json(serde_json::json!({"ok": true}))
}
