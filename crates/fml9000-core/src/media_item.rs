use crate::models::{Track, YouTubeVideo};
use chrono::NaiveDateTime;
use std::sync::Arc;

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
            MediaItem::Video(_) => "YouTube",
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
            Some(s) => format!("{}:{:02}", s / 60, s % 60),
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
