use axum::extract::State;
use axum::Json;
use fml9000_core::{get_user_playlists, get_youtube_channels};
use std::sync::Arc;

use crate::state::AppState;

#[derive(serde_derive::Serialize)]
pub struct SidebarData {
    auto_playlists: Vec<NavItem>,
    user_playlists: Vec<NavItem>,
    youtube_channels: Vec<NavItem>,
}

#[derive(serde_derive::Serialize)]
pub struct NavItem {
    id: String,
    label: String,
    kind: String,
}

pub async fn get_sidebar(
    State(_state): State<Arc<AppState>>,
) -> Json<SidebarData> {
    let (playlists, channels) = tokio::task::spawn_blocking(|| {
        let playlists = get_user_playlists().unwrap_or_default();
        let channels = get_youtube_channels().unwrap_or_default();
        (playlists, channels)
    })
    .await
    .unwrap_or_default();

    Json(SidebarData {
        auto_playlists: vec![
            NavItem { id: "all_media".into(), label: "All Media".into(), kind: "auto".into() },
            NavItem { id: "all_tracks".into(), label: "All Tracks".into(), kind: "auto".into() },
            NavItem { id: "all_videos".into(), label: "All Videos".into(), kind: "auto".into() },
            NavItem { id: "recently_added".into(), label: "Recently Added".into(), kind: "auto".into() },
            NavItem { id: "recently_played".into(), label: "Recently Played".into(), kind: "auto".into() },
            NavItem { id: "queue".into(), label: "Playback Queue".into(), kind: "auto".into() },
        ],
        user_playlists: playlists
            .iter()
            .map(|p| NavItem {
                id: format!("playlist_{}", p.id),
                label: p.name.clone(),
                kind: "playlist".into(),
            })
            .collect(),
        youtube_channels: channels
            .iter()
            .map(|c| NavItem {
                id: format!("channel_{}", c.id),
                label: c.name.clone(),
                kind: "channel".into(),
            })
            .collect(),
    })
}
