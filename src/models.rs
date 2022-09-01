use diesel::prelude::*;

#[derive(Queryable)]
pub struct Track {
  pub filename: String,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub album_artist: String,
}
