use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use fml9000_core::{
    add_to_playlist, create_playlist, delete_playlist, get_playlist_items, get_user_playlists,
    load_track_by_filename, load_video_by_id, remove_from_playlist, rename_playlist, MediaItem,
};
use std::sync::Arc;

use crate::api::queue::media_item_to_json;
use crate::state::AppState;

#[derive(serde_derive::Serialize)]
pub struct PlaylistJson {
    id: i32,
    name: String,
    created_at: String,
}

pub async fn list(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<PlaylistJson>>, (StatusCode, String)> {
    let playlists = tokio::task::spawn_blocking(|| get_user_playlists())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(
        playlists
            .iter()
            .map(|p| PlaylistJson {
                id: p.id,
                name: p.name.clone(),
                created_at: p.created_at.format("%Y-%m-%d").to_string(),
            })
            .collect(),
    ))
}

#[derive(serde_derive::Deserialize)]
pub struct CreateRequest {
    pub name: String,
}

pub async fn create(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<CreateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let name = req.name;
    let id = tokio::task::spawn_blocking(move || create_playlist(&name))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({"ok": true, "id": id})))
}

pub async fn delete(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || delete_playlist(id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(serde_derive::Deserialize)]
pub struct RenameRequest {
    pub name: String,
}

pub async fn rename(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(req): Json<RenameRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let name = req.name;
    tokio::task::spawn_blocking(move || rename_playlist(id, &name))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn get_items(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<Vec<crate::api::tracks::MediaItemJson>>, (StatusCode, String)> {
    let items = tokio::task::spawn_blocking(move || get_playlist_items(id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    *state.playlist_items.write().unwrap() = items.clone();

    Ok(Json(items.iter().map(media_item_to_json).collect()))
}

#[derive(serde_derive::Deserialize)]
pub struct AddItemRequest {
    pub track_filename: Option<String>,
    pub youtube_video_id: Option<i32>,
}

pub async fn add_item(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(req): Json<AddItemRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || {
        if let Some(filename) = req.track_filename {
            if let Some(track) = load_track_by_filename(&filename) {
                add_to_playlist(id, &MediaItem::Track(track))
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            }
        } else if let Some(vid_id) = req.youtube_video_id {
            if let Some(video) = load_video_by_id(vid_id) {
                add_to_playlist(id, &MediaItem::Video(video))
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e: (StatusCode, String)| e)?;

    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn remove_item(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(req): Json<AddItemRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || {
        if let Some(filename) = req.track_filename {
            if let Some(track) = load_track_by_filename(&filename) {
                remove_from_playlist(id, &MediaItem::Track(track))
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            }
        } else if let Some(vid_id) = req.youtube_video_id {
            if let Some(video) = load_video_by_id(vid_id) {
                remove_from_playlist(id, &MediaItem::Video(video))
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e: (StatusCode, String)| e)?;

    Ok(Json(serde_json::json!({"ok": true})))
}
