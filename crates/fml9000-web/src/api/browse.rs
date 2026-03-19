use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use fml9000_core::{get_distinct_albums, load_tracks_by_album};
use std::sync::Arc;

use crate::state::AppState;

#[derive(serde_derive::Serialize)]
pub struct AlbumJson {
    artist: String,
    album: String,
    representative_filename: String,
}

pub async fn get_albums(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<AlbumJson>>, (StatusCode, String)> {
    let albums = tokio::task::spawn_blocking(|| get_distinct_albums())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let json: Vec<AlbumJson> = albums
        .iter()
        .map(|t| AlbumJson {
            artist: t
                .album_artist
                .clone()
                .unwrap_or_else(|| t.artist.clone().unwrap_or_else(|| "Unknown".into())),
            album: t.album.clone().unwrap_or_else(|| "Unknown".into()),
            representative_filename: t.filename.clone(),
        })
        .collect();

    Ok(Json(json))
}

pub async fn get_album_tracks(
    State(state): State<Arc<AppState>>,
    Path((artist, album)): Path<(String, String)>,
) -> Result<Json<Vec<crate::api::tracks::TrackJson>>, (StatusCode, String)> {
    let tracks = tokio::task::spawn_blocking(move || load_tracks_by_album(&artist, &album))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items: Vec<fml9000_core::MediaItem> = tracks
        .iter()
        .map(|t| fml9000_core::MediaItem::Track(t.clone()))
        .collect();
    *state.playlist_items.write().unwrap() = items;

    let json: Vec<crate::api::tracks::TrackJson> = tracks
        .iter()
        .map(|t| crate::api::tracks::TrackJson {
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
