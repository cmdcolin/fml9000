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
use lofty::file::TaggedFileExt;
use lofty::prelude::Accessor;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use std::collections::HashSet;
use std::rc::Rc;
use walkdir::WalkDir;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn run_migration(conn: &mut SqliteConnection) {
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

fn hashset(data: &[Rc<Track>]) -> HashSet<&String> {
  data.iter().map(|elt| &elt.filename).collect()
}

pub fn run_scan(folder: &str, rows: &[Rc<Track>]) {
  let existing_files = hashset(rows);
  let mut conn = connect_db();
  let chunk_size = 20;

  let walker = WalkDir::new(folder)
    .into_iter()
    .filter_map(|e| e.ok());

  for chunk in chunked_iterator::ChunkedIterator::new(walker, chunk_size) {
    for entry in chunk {
      if !entry.file_type().is_file() {
        continue;
      }

      let path_str = entry.path().display().to_string();
      if existing_files.contains(&path_str) {
        continue;
      }

      let Ok(tagged_file) = Probe::open(&path_str)
        .expect("ERROR: Bad path provided!")
        .read()
      else {
        continue;
      };

      let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

      if let Some(t) = tag {
        let _ = diesel::insert_into(tracks::table)
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
    }
  }
}

pub fn add_track_to_recently_played(path: &str) {
  use self::schema::recently_played;

  let mut conn = connect_db();
  let _ = diesel::replace_into(recently_played::table)
    .values(NewRecentlyPlayed { filename: path })
    .execute(&mut conn);
}

pub fn load_recently_played(limit: i64) -> Vec<Rc<Track>> {
  use self::schema::recently_played::dsl as rp;
  use self::schema::tracks::dsl as t;

  let conn = &mut connect_db();

  t::tracks
    .inner_join(rp::recently_played.on(t::filename.eq(rp::filename)))
    .order(rp::timestamp.desc())
    .limit(limit)
    .select(Track::as_select())
    .load::<Track>(conn)
    .unwrap_or_default()
    .into_iter()
    .map(Rc::new)
    .collect()
}

pub fn load_tracks() -> Vec<Rc<Track>> {
  use self::schema::tracks::dsl::*;

  let conn = &mut connect_db();
  tracks
    .load::<Track>(conn)
    .expect("Error loading tracks")
    .into_iter()
    .map(Rc::new)
    .collect()
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
