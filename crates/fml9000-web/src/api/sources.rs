use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use fml9000_core::{
    get_all_media, get_all_videos, get_queue_items, load_recently_added_items,
    load_recently_played_items, load_tracks, MediaItem,
};
use std::sync::Arc;

use crate::api::queue::media_item_to_json;
use crate::state::AppState;

pub async fn get_source_items(
    State(state): State<Arc<AppState>>,
    Path(source_id): Path<String>,
) -> Result<Json<Vec<crate::api::tracks::MediaItemJson>>, (StatusCode, String)> {
    let items = tokio::task::spawn_blocking(move || -> Vec<MediaItem> {
        match source_id.as_str() {
            "all-media" | "all_media" => get_all_media(),
            "all-tracks" | "all_tracks" => load_tracks()
                .unwrap_or_default()
                .into_iter()
                .map(MediaItem::Track)
                .collect(),
            "all-videos" | "all_videos" => get_all_videos()
                .unwrap_or_default()
                .into_iter()
                .map(MediaItem::Video)
                .collect(),
            "recently-added" | "recently_added" => load_recently_added_items(0),
            "recently-played" | "recently_played" => load_recently_played_items(100),
            "queue" => get_queue_items(),
            _ => Vec::new(),
        }
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    *state.playlist_items.write().unwrap() = items.clone();

    Ok(Json(items.iter().map(media_item_to_json).collect()))
}
