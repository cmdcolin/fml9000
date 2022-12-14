use crate::schema::{recently_played, tracks};
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable)]
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
