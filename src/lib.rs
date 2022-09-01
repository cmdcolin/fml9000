pub mod models;
pub mod schema;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenvy::dotenv;
use models::*;
use std::env;

pub fn establish_connection() -> SqliteConnection {
  dotenv().ok();

  let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
  SqliteConnection::establish(&database_url)
    .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub fn query_db() {
  use self::schema::tracks::dsl::*;

  let connection = &mut establish_connection();
  let results = tracks
    .filter(published.eq(true))
    .limit(5)
    .load::<Track>(connection)
    .expect("Error loading tracks");

  println!("Displaying {} tracks", results.len());
  for track in results {
    println!("{}", track.id);
    println!("{}", track.filename);
    println!("-----------\n");
  }
}
