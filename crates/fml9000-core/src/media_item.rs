use crate::models::{Track, YouTubeVideo};
use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::sync::Arc;

pub fn format_duration_secs(total_secs: i32) -> String {
  format!("{}:{:02}", total_secs / 60, total_secs % 60)
}

#[derive(Clone, Debug)]
pub enum MediaItem {
    Track(Arc<Track>),
    Video(Arc<YouTubeVideo>),
}

impl MediaItem {
    pub fn title(&self) -> &str {
        match self {
            MediaItem::Track(t) => t.title.as_deref().unwrap_or("Unknown"),
            MediaItem::Video(v) => &v.title,
        }
    }

    pub fn artist(&self) -> &str {
        match self {
            MediaItem::Track(t) => t.artist.as_deref().unwrap_or("Unknown"),
            MediaItem::Video(_) => "",
        }
    }

    pub fn artist_with_channel_names(&self, channel_names: &HashMap<i32, String>) -> String {
        match self {
            MediaItem::Track(t) => t.artist.clone().unwrap_or_else(|| "Unknown".to_string()),
            MediaItem::Video(v) => channel_names
                .get(&v.channel_id)
                .cloned()
                .unwrap_or_default(),
        }
    }

    pub fn album(&self) -> &str {
        match self {
            MediaItem::Track(t) => t.album.as_deref().unwrap_or("Unknown"),
            MediaItem::Video(_) => "",
        }
    }

    pub fn duration_seconds(&self) -> Option<i32> {
        match self {
            MediaItem::Track(t) => t.duration_seconds,
            MediaItem::Video(v) => v.duration_seconds,
        }
    }

    pub fn duration_str(&self) -> String {
        match self.duration_seconds() {
            Some(s) => format_duration_secs(s),
            None => "?:??".to_string(),
        }
    }

    pub fn last_played(&self) -> Option<NaiveDateTime> {
        match self {
            MediaItem::Track(t) => t.last_played,
            MediaItem::Video(v) => v.last_played,
        }
    }

    pub fn added(&self) -> Option<NaiveDateTime> {
        match self {
            MediaItem::Track(t) => t.added,
            MediaItem::Video(v) => v.added,
        }
    }

    pub fn play_count(&self) -> i32 {
        match self {
            MediaItem::Track(t) => t.play_count,
            MediaItem::Video(v) => v.play_count,
        }
    }

    pub fn last_played_str(&self) -> String {
        match self.last_played() {
            Some(d) => d.format("%Y-%m-%d").to_string(),
            None => "-".to_string(),
        }
    }

    pub fn added_str(&self) -> String {
        match self.added() {
            Some(d) => d.format("%Y-%m-%d").to_string(),
            None => "-".to_string(),
        }
    }

    pub fn as_track(&self) -> Option<&Arc<Track>> {
        match self {
            MediaItem::Track(t) => Some(t),
            MediaItem::Video(_) => None,
        }
    }

    pub fn as_video(&self) -> Option<&Arc<YouTubeVideo>> {
        match self {
            MediaItem::Track(_) => None,
            MediaItem::Video(v) => Some(v),
        }
    }

    pub fn thumbnail_url(&self) -> Option<String> {
        match self {
            MediaItem::Track(_) => None,
            MediaItem::Video(v) => {
              Some(v.thumbnail_url.clone().unwrap_or_else(|| {
                format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", v.video_id)
              }))
            }
        }
    }

    pub fn track_filename(&self) -> Option<&str> {
        match self {
            MediaItem::Track(t) => Some(&t.filename),
            MediaItem::Video(_) => None,
        }
    }

    pub fn video_db_id(&self) -> Option<i32> {
        match self {
            MediaItem::Track(_) => None,
            MediaItem::Video(v) => Some(v.id),
        }
    }

    pub fn youtube_video_id(&self) -> Option<&str> {
        match self {
            MediaItem::Track(_) => None,
            MediaItem::Video(v) => Some(&v.video_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn make_track(
        filename: &str,
        title: Option<&str>,
        artist: Option<&str>,
        album: Option<&str>,
        duration: Option<i32>,
    ) -> MediaItem {
        MediaItem::Track(Arc::new(Track {
            filename: filename.to_string(),
            title: title.map(str::to_string),
            artist: artist.map(str::to_string),
            album: album.map(str::to_string),
            album_artist: None,
            track: None,
            genre: None,
            added: None,
            duration_seconds: duration,
            play_count: 0,
            last_played: None,
        }))
    }

    fn make_video(video_id: &str, title: &str, duration: Option<i32>) -> MediaItem {
        MediaItem::Video(Arc::new(YouTubeVideo {
            id: 42,
            video_id: video_id.to_string(),
            channel_id: 1,
            title: title.to_string(),
            duration_seconds: duration,
            thumbnail_url: None,
            published_at: None,
            fetched_at: NaiveDate::from_ymd_opt(2024, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            play_count: 0,
            last_played: None,
            added: None,
        }))
    }

    #[test]
    fn track_title_with_value() {
        let item = make_track("/a.mp3", Some("My Song"), None, None, None);
        assert_eq!(item.title(), "My Song");
    }

    #[test]
    fn track_title_defaults_to_unknown() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.title(), "Unknown");
    }

    #[test]
    fn video_title() {
        let item = make_video("abc123", "Cool Video", None);
        assert_eq!(item.title(), "Cool Video");
    }

    #[test]
    fn track_artist_with_value() {
        let item = make_track("/a.mp3", None, Some("Band"), None, None);
        assert_eq!(item.artist(), "Band");
    }

    #[test]
    fn track_artist_defaults_to_unknown() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.artist(), "Unknown");
    }

    #[test]
    fn video_artist_is_empty() {
        let item = make_video("abc", "Vid", None);
        assert_eq!(item.artist(), "");
    }

    #[test]
    fn track_album_with_value() {
        let item = make_track("/a.mp3", None, None, Some("Greatest Hits"), None);
        assert_eq!(item.album(), "Greatest Hits");
    }

    #[test]
    fn track_album_defaults_to_unknown() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.album(), "Unknown");
    }

    #[test]
    fn video_album_is_empty() {
        let item = make_video("abc", "Vid", None);
        assert_eq!(item.album(), "");
    }

    #[test]
    fn duration_str_formats_correctly() {
        let item = make_track("/a.mp3", None, None, None, Some(185));
        assert_eq!(item.duration_str(), "3:05");
    }

    #[test]
    fn duration_str_zero() {
        let item = make_track("/a.mp3", None, None, None, Some(0));
        assert_eq!(item.duration_str(), "0:00");
    }

    #[test]
    fn duration_str_exact_minute() {
        let item = make_track("/a.mp3", None, None, None, Some(120));
        assert_eq!(item.duration_str(), "2:00");
    }

    #[test]
    fn duration_str_none() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.duration_str(), "?:??");
    }

    #[test]
    fn duration_str_long() {
        let item = make_track("/a.mp3", None, None, None, Some(3661));
        assert_eq!(item.duration_str(), "61:01");
    }

    #[test]
    fn duration_str_video() {
        let item = make_video("abc", "Vid", Some(90));
        assert_eq!(item.duration_str(), "1:30");
    }

    #[test]
    fn track_filename_for_track() {
        let item = make_track("/music/song.mp3", None, None, None, None);
        assert_eq!(item.track_filename(), Some("/music/song.mp3"));
    }

    #[test]
    fn track_filename_for_video() {
        let item = make_video("abc", "Vid", None);
        assert_eq!(item.track_filename(), None);
    }

    #[test]
    fn video_db_id_for_video() {
        let item = make_video("abc", "Vid", None);
        assert_eq!(item.video_db_id(), Some(42));
    }

    #[test]
    fn video_db_id_for_track() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.video_db_id(), None);
    }

    #[test]
    fn youtube_video_id_for_video() {
        let item = make_video("dQw4w9WgXcQ", "Never Gonna", None);
        assert_eq!(item.youtube_video_id(), Some("dQw4w9WgXcQ"));
    }

    #[test]
    fn youtube_video_id_for_track() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.youtube_video_id(), None);
    }

    #[test]
    fn as_track_returns_some_for_track() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert!(item.as_track().is_some());
        assert!(item.as_video().is_none());
    }

    #[test]
    fn as_video_returns_some_for_video() {
        let item = make_video("abc", "Vid", None);
        assert!(item.as_video().is_some());
        assert!(item.as_track().is_none());
    }

    #[test]
    fn play_count_default() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.play_count(), 0);
    }

    #[test]
    fn last_played_str_none() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.last_played_str(), "-");
    }

    #[test]
    fn last_played_str_with_date() {
        let dt = NaiveDate::from_ymd_opt(2024, 6, 15)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let item = MediaItem::Track(Arc::new(Track {
            filename: "/a.mp3".to_string(),
            title: None,
            artist: None,
            album: None,
            album_artist: None,
            track: None,
            genre: None,
            added: None,
            duration_seconds: None,
            play_count: 3,
            last_played: Some(dt),
        }));
        assert_eq!(item.last_played_str(), "2024-06-15");
    }

    #[test]
    fn added_str_none() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.added_str(), "-");
    }

    #[test]
    fn added_str_with_date() {
        let dt = NaiveDate::from_ymd_opt(2023, 12, 25)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let item = MediaItem::Track(Arc::new(Track {
            filename: "/a.mp3".to_string(),
            title: None,
            artist: None,
            album: None,
            album_artist: None,
            track: None,
            genre: None,
            added: Some(dt),
            duration_seconds: None,
            play_count: 0,
            last_played: None,
        }));
        assert_eq!(item.added_str(), "2023-12-25");
    }

    #[test]
    fn duration_seconds_track() {
        let item = make_track("/a.mp3", None, None, None, Some(300));
        assert_eq!(item.duration_seconds(), Some(300));
    }

    #[test]
    fn duration_seconds_video() {
        let item = make_video("abc", "Vid", Some(600));
        assert_eq!(item.duration_seconds(), Some(600));
    }

    #[test]
    fn duration_seconds_none() {
        let item = make_track("/a.mp3", None, None, None, None);
        assert_eq!(item.duration_seconds(), None);
    }
}
