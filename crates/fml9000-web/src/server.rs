use axum::routing::{delete, get, post, put};
use axum::Router;
use std::sync::Arc;
use tower_http::services::ServeDir;

use crate::api::{browse, playback, playlists, queue, sidebar, sources, thumbnails, tracks, youtube};
use crate::state::AppState;
use crate::ws;

pub fn create_router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        // Tracks
        .route("/tracks", get(tracks::get_tracks))
        // Sidebar navigation
        .route("/sidebar", get(sidebar::get_sidebar))
        // Source items (auto playlists)
        .route("/sources/{source_id}", get(sources::get_source_items))
        // Playback
        .route("/playback/state", get(playback::get_state))
        .route("/playback/play", post(playback::play))
        .route("/playback/pause", post(playback::pause))
        .route("/playback/resume", post(playback::resume))
        .route("/playback/stop", post(playback::stop))
        .route("/playback/next", post(playback::next))
        .route("/playback/prev", post(playback::prev))
        .route("/playback/seek", post(playback::seek))
        .route("/playback/volume", post(playback::volume))
        .route("/playback/shuffle", post(playback::set_shuffle))
        .route("/playback/repeat", post(playback::set_repeat))
        // Browse (albums)
        .route("/albums", get(browse::get_albums))
        .route("/albums/{artist}/{album}", get(browse::get_album_tracks))
        // Thumbnails
        .route("/thumbnails/{key}", get(thumbnails::get_thumbnail))
        // Queue
        .route("/queue", get(queue::get))
        .route("/queue", post(queue::add))
        .route("/queue/clear", post(queue::clear))
        .route("/queue/remove", post(queue::remove))
        // Playlists
        .route("/playlists", get(playlists::list))
        .route("/playlists", post(playlists::create))
        .route("/playlists/{id}", delete(playlists::delete))
        .route("/playlists/{id}", put(playlists::rename))
        .route("/playlists/{id}/items", get(playlists::get_items))
        .route("/playlists/{id}/items", post(playlists::add_item))
        .route("/playlists/{id}/items/remove", post(playlists::remove_item))
        // YouTube
        .route("/youtube/channels", get(youtube::list_channels))
        .route("/youtube/channels/{id}", delete(youtube::delete_channel))
        .route("/youtube/channels/{id}/videos", get(youtube::get_channel_videos));

    let static_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("static");

    Router::new()
        .nest("/api", api)
        .route("/ws", get(ws::ws_handler))
        .fallback_service(ServeDir::new(static_dir))
        .with_state(state)
}
