use crate::schema::{playback_queue, playlist_tracks, playlists, tracks, youtube_channels, youtube_videos};
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::tracks)]
pub struct Track {
  pub filename: String,
  pub title: Option<String>,
  pub artist: Option<String>,
  pub track: Option<String>,
  pub album: Option<String>,
  pub genre: Option<String>,
  pub album_artist: Option<String>,
  pub added: Option<NaiveDateTime>,
  pub duration_seconds: Option<i32>,
  pub play_count: i32,
  pub last_played: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = tracks)]
pub struct NewTrack<'a> {
  pub filename: &'a str,
  pub artist: Option<&'a str>,
  pub title: Option<&'a str>,
  pub album: Option<&'a str>,
  pub genre: Option<&'a str>,
  pub track: Option<&'a str>,
  pub album_artist: Option<&'a str>,
  pub duration_seconds: Option<i32>,
}

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::youtube_channels)]
pub struct YouTubeChannel {
  pub id: i32,
  pub channel_id: String,
  pub name: String,
  pub handle: Option<String>,
  pub url: String,
  pub thumbnail_url: Option<String>,
  pub last_fetched: Option<NaiveDateTime>,
  pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = youtube_channels)]
pub struct NewYouTubeChannel<'a> {
  pub channel_id: &'a str,
  pub name: &'a str,
  pub handle: Option<&'a str>,
  pub url: &'a str,
  pub thumbnail_url: Option<&'a str>,
}

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::youtube_videos)]
pub struct YouTubeVideo {
  pub id: i32,
  pub video_id: String,
  pub channel_id: i32,
  pub title: String,
  pub duration_seconds: Option<i32>,
  pub thumbnail_url: Option<String>,
  pub published_at: Option<NaiveDateTime>,
  pub fetched_at: NaiveDateTime,
  pub play_count: i32,
  pub last_played: Option<NaiveDateTime>,
  pub added: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = youtube_videos)]
pub struct NewYouTubeVideo<'a> {
  pub video_id: &'a str,
  pub channel_id: i32,
  pub title: &'a str,
  pub duration_seconds: Option<i32>,
  pub thumbnail_url: Option<&'a str>,
  pub published_at: Option<NaiveDateTime>,
}

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::playlists)]
pub struct Playlist {
  pub id: i32,
  pub name: String,
  pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = playlists)]
pub struct NewPlaylist<'a> {
  pub name: &'a str,
}

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::playlist_tracks)]
pub struct PlaylistTrack {
  pub id: i32,
  pub playlist_id: i32,
  pub track_filename: Option<String>,
  pub youtube_video_id: Option<i32>,
  pub position: i32,
  pub added_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = playlist_tracks)]
pub struct NewPlaylistTrack<'a> {
  pub playlist_id: i32,
  pub track_filename: Option<&'a str>,
  pub youtube_video_id: Option<i32>,
  pub position: i32,
}

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::playback_queue)]
pub struct PlaybackQueueItem {
  pub id: i32,
  pub position: i32,
  pub track_filename: Option<String>,
  pub youtube_video_id: Option<i32>,
  pub added_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = playback_queue)]
pub struct NewPlaybackQueueItem<'a> {
  pub position: i32,
  pub track_filename: Option<&'a str>,
  pub youtube_video_id: Option<i32>,
}
