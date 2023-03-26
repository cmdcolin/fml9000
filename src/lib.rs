mod chunked_iterator;
pub mod models;
pub mod schema;

use self::models::*;
use self::schema::tracks;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use directories::ProjectDirs;
use gtk::gio;
use gtk::glib::BoxedAnyObject;
use lofty::{Accessor, ItemKey, Probe, TaggedFileExt};
use std::collections::HashSet;
use std::rc::Rc;
use walkdir::WalkDir;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

fn run_migration(conn: &mut SqliteConnection) {
  conn.run_pending_migrations(MIGRATIONS).unwrap();
}

#[derive(Hash, Eq, Ord, PartialEq, PartialOrd, Debug)]
pub struct Facet {
  pub album_artist_or_artist: Option<String>,
  pub album_artist: Option<String>,
  pub album: Option<String>,
  pub all: bool,
}

pub fn connect_db() -> SqliteConnection {
  let proj_dirs = ProjectDirs::from("com", "github", "fml9000").unwrap();
  let path = proj_dirs.config_dir().join("library.db");
  let database_url = format!("sqlite://{}", path.to_str().unwrap());
  SqliteConnection::establish(&database_url)
    .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

fn hashset(data: &Vec<Rc<Track>>) -> HashSet<&std::string::String> {
  HashSet::from_iter(data.iter().map(|elt| &elt.filename))
}

pub fn run_scan(folder: &str, rows: &Vec<Rc<Track>>) {
  let hash = hashset(rows);
  let mut conn = connect_db();
  let transaction_size = 20;

  for chunk in chunked_iterator::ChunkedIterator::new(
    WalkDir::new(folder).into_iter().filter_map(|e| e.ok()),
    transaction_size,
  ) {
    for file in chunk {
      if file.file_type().is_file() {
        let path = file.path();
        let path_str = path.display().to_string();
        if !hash.contains(&path_str) {
          let tagged_file = Probe::open(&path_str)
            .expect("ERROR: Bad path provided!")
            .read();
          match tagged_file {
            Ok(tagged_file) => {
              let tag = match tagged_file.primary_tag() {
                Some(primary_tag) => Some(primary_tag),
                None => tagged_file.first_tag(),
              };
              match tag {
                Some(t) => {
                  diesel::insert_into(tracks::table)
                    .values(NewTrack {
                      filename: &path_str,
                      artist: t.artist().as_deref(),
                      album: t.album().as_deref(),
                      album_artist: t.get_string(&ItemKey::AlbumArtist),
                      title: t.title().as_deref(),
                      track: t.get_string(&ItemKey::TrackNumber),
                      genre: t.genre().as_deref(),
                    })
                    .execute(&mut conn);
                }
                None => (),
              }
            }
            Err(_) => (),
          };
        }
      }
    }
  }
}

pub fn add_track_to_recently_played(_path: &str) -> () {
  // let conn = connect_db();
  // conn.execute("INSERT INTO recently_played (filename) VALUES (?)", (path,))?;

  // Ok(())
}

pub fn load_tracks() -> Vec<Rc<Track>> {
  use self::schema::tracks::dsl::*;

  let conn = &mut connect_db();
  let results = tracks.load::<Track>(conn).expect("Error loading tracks");

  results.into_iter().map(|r| Rc::new(r)).collect()
}

pub fn load_playlist_store<'a, I>(vals: I, store: &gio::ListStore)
where
  I: Iterator<Item = &'a Rc<Track>>,
{
  for row in vals {
    store.append(&BoxedAnyObject::new(row.clone()));
  }
}

pub fn load_facet_store(rows: &[Rc<Track>], facet_store: &gio::ListStore) {
  let mut facets = HashSet::new();
  for row in rows {
    facets.insert(Facet {
      album: row.album.clone(),
      album_artist: row.album_artist.clone(),
      album_artist_or_artist: row.album_artist.clone().or(row.artist.clone()),
      all: false,
    });
  }
  facet_store.append(&BoxedAnyObject::new(Facet {
    album: None,
    album_artist: None,
    album_artist_or_artist: None,
    all: true,
  }));
  let mut v = Vec::from_iter(facets);
  v.sort();
  for uniq in v {
    facet_store.append(&BoxedAnyObject::new(uniq))
  }
}
