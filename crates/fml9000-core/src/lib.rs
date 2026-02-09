pub mod audio;
pub mod db;
pub mod media_item;
pub mod models;
pub mod schema;
pub mod settings;
pub mod vaporwave;
pub mod youtube;
pub mod youtube_api;

// Re-exports for convenience
pub use audio::AudioPlayer;
pub use db::*;
pub use media_item::MediaItem;
pub use models::*;
pub use settings::{CoreSettings, RepeatMode};
pub use vaporwave::{
  check_ffmpeg_available, check_yt_dlp_available, calculate_vaporwave_duration, VaporwaveDecoder,
  YouTubeVaporwaveDecoder,
};
pub use youtube_api::*;
