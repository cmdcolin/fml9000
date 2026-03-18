pub mod audio;
pub mod db;
pub mod media_item;
pub mod models;
pub mod playback;
pub mod schema;
pub mod settings;
pub mod thumbnail_cache;
pub mod youtube;
pub mod youtube_api;

// Re-exports for convenience
pub use audio::AudioPlayer;
pub use db::*;
pub use media_item::{format_duration_secs, MediaItem};
pub use models::*;
pub use playback::{compute_next_index, compute_prev_index, NextTrackResult};
pub use settings::{CoreSettings, RepeatMode};
pub use youtube_api::*;
