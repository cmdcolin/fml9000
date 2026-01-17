mod chunked_iterator;
pub mod models;
pub mod schema;

use self::models::*;
use self::schema::{tracks, youtube_channels, youtube_videos};
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

fn get_project_dirs() -> Option<ProjectDirs> {
  ProjectDirs::from("com", "github", "fml9000")
}

pub fn connect_db() -> Result<SqliteConnection, String> {
  let proj_dirs =
    get_project_dirs().ok_or_else(|| "Could not determine config directory".to_string())?;
  let path = proj_dirs.config_dir().join("library.db");
  let path_str = path
    .to_str()
    .ok_or_else(|| "Database path contains invalid UTF-8".to_string())?;
  let database_url = format!("sqlite://{}", path_str);
  SqliteConnection::establish(&database_url)
    .map_err(|e| format!("Error connecting to database: {e}"))
}

fn hashset(data: &[Rc<Track>]) -> HashSet<&String> {
  data.iter().map(|elt| &elt.filename).collect()
}

pub fn run_scan(folder: &str, rows: &[Rc<Track>]) {
  let existing_files = hashset(rows);
  let mut conn = match connect_db() {
    Ok(c) => c,
    Err(e) => {
      eprintln!("Warning: Could not connect to database for scanning: {e}");
      return;
    }
  };
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

      let Ok(probe) = Probe::open(&path_str) else {
        continue;
      };

      let Ok(tagged_file) = probe.read() else {
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

pub fn run_scan_folders(folders: &[String], rows: &[Rc<Track>]) {
  for folder in folders {
    run_scan(folder, rows);
  }
}

pub fn add_track_to_recently_played(path: &str) {
  use self::schema::recently_played;

  let Ok(mut conn) = connect_db() else {
    return;
  };
  let _ = diesel::replace_into(recently_played::table)
    .values(NewRecentlyPlayed { filename: path })
    .execute(&mut conn);
}

pub fn load_recently_played(limit: i64) -> Vec<Rc<Track>> {
  use self::schema::recently_played::dsl as rp;
  use self::schema::tracks::dsl as t;

  let Ok(mut conn) = connect_db() else {
    return Vec::new();
  };

  t::tracks
    .inner_join(rp::recently_played.on(t::filename.eq(rp::filename)))
    .order(rp::timestamp.desc())
    .limit(limit)
    .select(Track::as_select())
    .load::<Track>(&mut conn)
    .unwrap_or_default()
    .into_iter()
    .map(Rc::new)
    .collect()
}

pub fn load_tracks() -> Result<Vec<Rc<Track>>, String> {
  use self::schema::tracks::dsl::*;

  let mut conn = connect_db()?;

  tracks
    .load::<Track>(&mut conn)
    .map(|v| v.into_iter().map(Rc::new).collect())
    .map_err(|e| format!("Error loading tracks: {e}"))
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

pub fn add_youtube_channel(
  channel_id: &str,
  name: &str,
  handle: Option<&str>,
  url: &str,
  thumbnail_url: Option<&str>,
) -> Result<i32, String> {
  use self::models::NewYouTubeChannel;

  let mut conn = connect_db()?;

  diesel::insert_into(youtube_channels::table)
    .values(NewYouTubeChannel {
      channel_id,
      name,
      handle,
      url,
      thumbnail_url,
    })
    .execute(&mut conn)
    .map_err(|e| format!("Failed to insert channel: {e}"))?;

  youtube_channels::table
    .filter(youtube_channels::channel_id.eq(channel_id))
    .select(youtube_channels::id)
    .first::<i32>(&mut conn)
    .map_err(|e| format!("Failed to get channel id: {e}"))
}

pub fn get_youtube_channels() -> Result<Vec<Rc<models::YouTubeChannel>>, String> {
  use self::models::YouTubeChannel;

  let mut conn = connect_db()?;

  youtube_channels::table
    .load::<YouTubeChannel>(&mut conn)
    .map(|v| v.into_iter().map(Rc::new).collect())
    .map_err(|e| format!("Failed to load channels: {e}"))
}

pub fn delete_youtube_channel(id: i32) -> Result<(), String> {
  let mut conn = connect_db()?;

  diesel::delete(youtube_channels::table.filter(youtube_channels::id.eq(id)))
    .execute(&mut conn)
    .map_err(|e| format!("Failed to delete channel: {e}"))?;

  Ok(())
}

pub fn add_youtube_videos(
  db_channel_id: i32,
  videos: &[(String, String, Option<i32>, Option<String>, Option<chrono::NaiveDateTime>)],
) -> Result<(), String> {
  use self::models::NewYouTubeVideo;

  let mut conn = connect_db()?;

  for (video_id, title, duration, thumbnail, published_at) in videos {
    let _ = diesel::insert_or_ignore_into(youtube_videos::table)
      .values(NewYouTubeVideo {
        video_id,
        channel_id: db_channel_id,
        title,
        duration_seconds: *duration,
        thumbnail_url: thumbnail.as_deref(),
        published_at: *published_at,
      })
      .execute(&mut conn);
  }

  Ok(())
}

pub fn get_videos_for_channel(db_channel_id: i32) -> Result<Vec<Rc<models::YouTubeVideo>>, String> {
  use self::models::YouTubeVideo;

  let mut conn = connect_db()?;

  youtube_videos::table
    .filter(youtube_videos::channel_id.eq(db_channel_id))
    .order(youtube_videos::published_at.desc())
    .load::<YouTubeVideo>(&mut conn)
    .map(|v| v.into_iter().map(Rc::new).collect())
    .map_err(|e| format!("Failed to load videos: {e}"))
}

pub fn update_channel_last_fetched(id: i32) -> Result<(), String> {
  let mut conn = connect_db()?;

  diesel::update(youtube_channels::table.filter(youtube_channels::id.eq(id)))
    .set(youtube_channels::last_fetched.eq(diesel::dsl::now))
    .execute(&mut conn)
    .map_err(|e| format!("Failed to update last_fetched: {e}"))?;

  Ok(())
}
