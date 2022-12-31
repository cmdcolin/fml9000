// use crate::schema::{recently_played, tracks};
use diesel::prelude::*;
use diesel::sql_types::Timestamp;

#[derive(Queryable)]
pub struct Track {
  pub filename: String,
  pub artist: Option<String>,
  pub title: Option<String>,
  pub album: Option<String>,
  pub genre: Option<String>,
  pub album_artist: Option<String>,
  pub added: Timestamp,
}

#[derive(Queryable)]
pub struct RecentlyPlayed {
  pub filename: String,
  pub timestamp: Timestamp,
}

// #[derive(Insertable)]
// #[diesel(table_name = tracks)]
// pub struct NewTrack<'a> {
//   pub filename: &'a str,
//   pub artist: &'a str,
//   pub title: &'a str,
//   pub album: &'a str,
//   pub genre: &'a str,
//   pub album_artist: &'a str,
// }

// #[derive(Insertable)]
// #[diesel(table_name = recently_played)]
// pub struct NewRecentlyPlayed<'a> {
//   pub filename: &'a str,
// }
