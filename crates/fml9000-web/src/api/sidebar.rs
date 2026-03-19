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
    db_id: Option<i32>,
    label: String,
    kind: String,
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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
            NavItem { id: "all-media".into(), db_id: None, label: "All Media".into(), kind: "auto".into() },
            NavItem { id: "all-tracks".into(), db_id: None, label: "All Tracks".into(), kind: "auto".into() },
            NavItem { id: "all-videos".into(), db_id: None, label: "All Videos".into(), kind: "auto".into() },
            NavItem { id: "recently-added".into(), db_id: None, label: "Recently Added".into(), kind: "auto".into() },
            NavItem { id: "recently-played".into(), db_id: None, label: "Recently Played".into(), kind: "auto".into() },
            NavItem { id: "queue".into(), db_id: None, label: "Playback Queue".into(), kind: "auto".into() },
        ],
        user_playlists: playlists
            .iter()
            .map(|p| NavItem {
                id: format!("playlist-{}", slugify(&p.name)),
                db_id: Some(p.id),
                label: p.name.clone(),
                kind: "playlist".into(),
            })
            .collect(),
        youtube_channels: channels
            .iter()
            .map(|c| NavItem {
                id: format!("channel-{}", slugify(&c.name)),
                db_id: Some(c.id),
                label: c.name.clone(),
                kind: "channel".into(),
            })
            .collect(),
    })
}
