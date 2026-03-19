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
    #[serde(skip_serializing_if = "Option::is_none")]
    new_count: Option<usize>,
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
    State(state): State<Arc<AppState>>,
) -> Json<SidebarData> {
    let new_counts = state.new_video_counts.lock().unwrap().clone();

    let (playlists, channels) = tokio::task::spawn_blocking(|| {
        let playlists = get_user_playlists().unwrap_or_default();
        let channels = get_youtube_channels().unwrap_or_default();
        (playlists, channels)
    })
    .await
    .unwrap_or_default();

    let auto = |id: &str, label: &str| NavItem {
        id: id.into(), db_id: None, label: label.into(), kind: "auto".into(), new_count: None,
    };

    Json(SidebarData {
        auto_playlists: vec![
            auto("all-media", "All Media"),
            auto("all-tracks", "All Tracks"),
            auto("all-videos", "All Videos"),
            auto("recently-added", "Recently Added"),
            auto("recently-played", "Recently Played"),
            auto("queue", "Playback Queue"),
        ],
        user_playlists: playlists
            .iter()
            .map(|p| NavItem {
                id: format!("playlist-{}", slugify(&p.name)),
                db_id: Some(p.id),
                label: p.name.clone(),
                kind: "playlist".into(),
                new_count: None,
            })
            .collect(),
        youtube_channels: channels
            .iter()
            .map(|c| {
                let count = new_counts.get(&c.id).copied().filter(|n| *n > 0);
                NavItem {
                    id: format!("channel-{}", slugify(&c.name)),
                    db_id: Some(c.id),
                    label: c.name.clone(),
                    kind: "channel".into(),
                    new_count: count,
                }
            })
            .collect(),
    })
}
