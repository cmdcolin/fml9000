use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use fml9000_core::{
    add_to_queue, clear_queue, get_queue_items, load_track_by_filename, load_video_by_id,
    remove_from_queue, MediaItem,
};
use std::sync::Arc;

use crate::state::AppState;

pub async fn get(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::api::tracks::MediaItemJson>>, (StatusCode, String)> {
    let items = tokio::task::spawn_blocking(get_queue_items)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    *state.playlist_items.write().unwrap() = items.clone();

    Ok(Json(items.iter().map(media_item_to_json).collect()))
}

#[derive(serde_derive::Deserialize)]
pub struct AddToQueueRequest {
    pub track_filename: Option<String>,
    pub youtube_video_id: Option<i32>,
}

pub async fn add(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<AddToQueueRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || {
        if let Some(filename) = req.track_filename {
            if let Some(track) = load_track_by_filename(&filename) {
                add_to_queue(&MediaItem::Track(track));
            }
        } else if let Some(vid_id) = req.youtube_video_id {
            if let Some(video) = load_video_by_id(vid_id) {
                add_to_queue(&MediaItem::Video(video));
            }
        }
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(serde_derive::Deserialize)]
pub struct RemoveFromQueueRequest {
    pub track_filename: Option<String>,
    pub youtube_video_id: Option<i32>,
}

pub async fn remove(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<RemoveFromQueueRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || {
        if let Some(filename) = req.track_filename {
            if let Some(track) = load_track_by_filename(&filename) {
                remove_from_queue(&MediaItem::Track(track));
            }
        } else if let Some(vid_id) = req.youtube_video_id {
            if let Some(video) = load_video_by_id(vid_id) {
                remove_from_queue(&MediaItem::Video(video));
            }
        }
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn clear(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let _ = tokio::task::spawn_blocking(clear_queue).await;
    Json(serde_json::json!({"ok": true}))
}

pub fn media_item_to_json(item: &MediaItem) -> crate::api::tracks::MediaItemJson {
    crate::api::tracks::MediaItemJson {
        kind: match item {
            MediaItem::Track(_) => "track".into(),
            MediaItem::Video(_) => "video".into(),
        },
        f: item.track_filename().map(|s| s.to_string()),
        video_id: item.youtube_video_id().map(|s| s.to_string()),
        video_db_id: item.video_db_id(),
        t: Some(item.title().to_string()),
        ar: Some(item.artist().to_string()),
        al: Some(item.album().to_string()),
        d: item.duration_seconds(),
        pc: item.play_count(),
        lp: item.last_played().map(|d| d.format("%Y-%m-%d").to_string()),
        ad: item.added().map(|d| d.format("%Y-%m-%d").to_string()),
    }
}
