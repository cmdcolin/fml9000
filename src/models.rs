use diesel::prelude::*;

#[derive(Queryable)]
pub struct Track {
  pub id: i64,
  pub filename: String,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub album_artist: String,
}
