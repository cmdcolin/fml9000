use diesel::prelude::*;

#[derive(Queryable)]
pub struct Track {
  pub id: i64,
  pub filename: String,
  pub published: bool,
}
