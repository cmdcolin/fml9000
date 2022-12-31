use diesel::prelude::*;

#[derive(Queryable)]
pub struct Track {
  pub filename: String,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub album_artist: String,
  pub genre: String,
  pub added: Date,
}

#[derive(Queryable)]
pub struct RecentlyPlayed {
  pub filename: String,
  pub timestamp: Date,
}
