use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use fml9000_core::{
    delete_youtube_channel, get_videos_for_channel, get_youtube_channels, MediaItem,
};
use std::sync::Arc;

use crate::api::queue::media_item_to_json;
use crate::state::AppState;

#[derive(serde_derive::Serialize)]
pub struct ChannelJson {
    id: i32,
    channel_id: String,
    name: String,
    handle: Option<String>,
    url: String,
    thumbnail_url: Option<String>,
}

pub async fn list_channels(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<ChannelJson>>, (StatusCode, String)> {
    let channels = tokio::task::spawn_blocking(|| get_youtube_channels())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(
        channels
            .iter()
            .map(|c| ChannelJson {
                id: c.id,
                channel_id: c.channel_id.clone(),
                name: c.name.clone(),
                handle: c.handle.clone(),
                url: c.url.clone(),
                thumbnail_url: c.thumbnail_url.clone(),
            })
            .collect(),
    ))
}

pub async fn get_channel_videos(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<Vec<crate::api::tracks::MediaItemJson>>, (StatusCode, String)> {
    let videos = tokio::task::spawn_blocking(move || get_videos_for_channel(id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let items: Vec<MediaItem> = videos.into_iter().map(MediaItem::Video).collect();
    *state.playlist_items.write().unwrap() = items.clone();

    Ok(Json(items.iter().map(media_item_to_json).collect()))
}

pub async fn delete_channel(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || delete_youtube_channel(id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({"ok": true})))
}
