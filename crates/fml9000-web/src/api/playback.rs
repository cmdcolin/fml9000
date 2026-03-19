use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use fml9000_core::{
    compute_next_index, compute_prev_index, mark_as_played, MediaItem, NextTrackResult,
};
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
    play_index(&state, req.index)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

fn play_index(state: &AppState, index: usize) -> Result<(), (StatusCode, String)> {
    let items = state.playlist_items.read().unwrap();
    let item = items
        .get(index)
        .ok_or((StatusCode::BAD_REQUEST, "Index out of range".to_string()))?
        .clone();
    drop(items);

    match &item {
        MediaItem::Track(track) => {
            state
                .play_file(&track.filename)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            *state.current_index.write().unwrap() = Some(index);

            let item_clone = item.clone();
            tokio::task::spawn_blocking(move || mark_as_played(&item_clone));

            state.broadcast_state();
            Ok(())
        }
        MediaItem::Video(_video) => Err((
            StatusCode::NOT_IMPLEMENTED,
            "YouTube playback not yet supported in web UI".to_string(),
        )),
    }
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
    state.send_audio(AudioCommand::Stop);
    *state.current_index.write().unwrap() = None;
    state.broadcast_state();
    Json(serde_json::json!({"ok": true}))
}

pub async fn next(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let current_index = *state.current_index.read().unwrap();
    let playlist_len = state.playlist_items.read().unwrap().len();
    let shuffle = state.shuffle_enabled.load(Ordering::Relaxed);
    let repeat = *state.repeat_mode.lock().unwrap();

    match compute_next_index(current_index, playlist_len, shuffle, repeat) {
        NextTrackResult::PlayIndex(idx) => {
            play_index(&state, idx)?;
            Ok(Json(serde_json::json!({"ok": true, "index": idx})))
        }
        NextTrackResult::Stop => {
            state.send_audio(AudioCommand::Stop);
            *state.current_index.write().unwrap() = None;
            state.broadcast_state();
            Ok(Json(serde_json::json!({"ok": true, "stopped": true})))
        }
    }
}

pub async fn prev(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let current_index = *state.current_index.read().unwrap();
    let playlist_len = state.playlist_items.read().unwrap().len();

    match compute_prev_index(current_index, playlist_len) {
        NextTrackResult::PlayIndex(idx) => {
            play_index(&state, idx)?;
            Ok(Json(serde_json::json!({"ok": true, "index": idx})))
        }
        NextTrackResult::Stop => {
            state.send_audio(AudioCommand::Stop);
            state.broadcast_state();
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
    state.broadcast_state();
    Json(serde_json::json!({"ok": true}))
}
