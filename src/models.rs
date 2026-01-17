use crate::schema::{recently_played, tracks, youtube_channels, youtube_videos};
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::tracks)]
pub struct Track {
  pub filename: String,
  pub artist: Option<String>,
  pub title: Option<String>,
  pub album: Option<String>,
  pub genre: Option<String>,
  pub album_artist: Option<String>,
  pub track: Option<String>,
  pub added: Option<NaiveDateTime>,
}

#[derive(Queryable)]
pub struct RecentlyPlayed {
  pub filename: String,
  pub timestamp: NaiveDateTime,
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
}

#[derive(Insertable)]
#[diesel(table_name = recently_played)]
pub struct NewRecentlyPlayed<'a> {
  pub filename: &'a str,
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
