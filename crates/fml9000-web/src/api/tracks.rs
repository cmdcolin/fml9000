use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use fml9000_core::{load_tracks, MediaItem};
use std::sync::Arc;

use crate::state::AppState;

#[derive(serde_derive::Serialize)]
pub struct TrackJson {
    pub f: String,
    pub t: Option<String>,
    pub ar: Option<String>,
    pub al: Option<String>,
    pub aa: Option<String>,
    pub tr: Option<String>,
    pub g: Option<String>,
    pub d: Option<i32>,
    pub pc: i32,
    pub lp: Option<String>,
    pub ad: Option<String>,
}

#[derive(serde_derive::Serialize)]
pub struct MediaItemJson {
    pub kind: String,
    pub f: Option<String>,
    pub video_id: Option<String>,
    pub video_db_id: Option<i32>,
    pub t: Option<String>,
    pub ar: Option<String>,
    pub al: Option<String>,
    pub d: Option<i32>,
    pub pc: i32,
    pub lp: Option<String>,
    pub ad: Option<String>,
}

pub async fn get_tracks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TrackJson>>, (StatusCode, String)> {
    let tracks = tokio::task::spawn_blocking(|| load_tracks())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let items: Vec<MediaItem> = tracks.iter().map(|t| MediaItem::Track(t.clone())).collect();
    *state.playlist_items.write().unwrap() = items;

    let json: Vec<TrackJson> = tracks
        .iter()
        .map(|t| TrackJson {
            f: t.filename.clone(),
            t: t.title.clone(),
            ar: t.artist.clone(),
            al: t.album.clone(),
            aa: t.album_artist.clone(),
            tr: t.track.clone(),
            g: t.genre.clone(),
            d: t.duration_seconds,
            pc: t.play_count,
            lp: t.last_played.map(|d| d.format("%Y-%m-%d").to_string()),
            ad: t.added.map(|d| d.format("%Y-%m-%d").to_string()),
        })
        .collect();

    Ok(Json(json))
}
