use crate::schema::tracks;
use diesel::prelude::*;

#[derive(Queryable)]
pub struct Track {
  pub filename: String,
  pub artist: String,
  pub title: String,
  pub album: String,
  pub genre: String,
  pub album_artist: String,
  pub added: Date,
}

#[derive(Queryable)]
pub struct RecentlyPlayed {
  pub filename: String,
  pub timestamp: Date,
}

#[derive(Insertable)]
#[diesel(table_name = tracks)]
pub struct NewTrack<'a> {
  pub filename: &'a str,
  pub artist: &'a str,
  pub title: &'a str,
  pub album: &'a str,
  pub genre: &'a str,
  pub album_artist: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = recently_played)]
pub struct NewRecentlyPlayed<'a> {
  pub filename: &'a str,
}
