mod chunked_iterator;
pub mod models;
pub mod schema;

use self::models::*;
use self::schema::{playlist_tracks, playlists, tracks, youtube_channels, youtube_videos};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use directories::ProjectDirs;
use gtk::gio;
use gtk::glib::BoxedAnyObject;
use lofty::file::{AudioFile, TaggedFileExt};
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
  let config_dir = proj_dirs.config_dir();
  std::fs::create_dir_all(config_dir)
    .map_err(|e| format!("Failed to create config directory: {e}"))?;
  let path = config_dir.join("library.db");
  let path_str = path
    .to_str()
    .ok_or_else(|| "Database path contains invalid UTF-8".to_string())?;
  let database_url = format!("sqlite://{}", path_str);
  let mut conn = SqliteConnection::establish(&database_url)
    .map_err(|e| format!("Error connecting to database: {e}"))?;
  conn
    .run_pending_migrations(MIGRATIONS)
    .map_err(|e| format!("Failed to run migrations: {e}"))?;
  Ok(conn)
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
        let duration_seconds = tagged_file
          .properties()
          .duration()
          .as_secs()
          .try_into()
          .ok();

        let _ = diesel::insert_into(tracks::table)
          .values(NewTrack {
            filename: &path_str,
            artist: t.artist().as_deref(),
            album: t.album().as_deref(),
            album_artist: t.get_string(&ItemKey::AlbumArtist),
            title: t.title().as_deref(),
            track: t.get_string(&ItemKey::TrackNumber),
            genre: t.genre().as_deref(),
            duration_seconds,
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

/// Progress update sent during scanning
#[derive(Clone)]
pub enum ScanProgress {
  /// Starting to scan a folder
  StartingFolder(String),
  /// Found a file (total_found, skipped, current_file)
  FoundFile(usize, usize, String),
  /// Scanned a file (total_found, skipped, added, updated, current_file)
  ScannedFile(usize, usize, usize, usize, String),
  /// Scan complete (total_found, skipped, added, updated)
  Complete(usize, usize, usize, usize),
}

/// Run scan with progress reporting via a channel
/// - existing_complete: files that exist and have all metadata (will be skipped)
/// - existing_incomplete: files that exist but need metadata update (e.g., missing duration)
pub fn run_scan_with_progress(
  folders: Vec<String>,
  existing_complete: std::collections::HashSet<String>,
  existing_incomplete: std::collections::HashSet<String>,
  progress_sender: std::sync::mpsc::Sender<ScanProgress>,
) {
  use self::schema::tracks::dsl;

  let mut conn = match connect_db() {
    Ok(c) => c,
    Err(e) => {
      eprintln!("Warning: Could not connect to database for scanning: {e}");
      let _ = progress_sender.send(ScanProgress::Complete(0, 0, 0, 0));
      return;
    }
  };

  let mut total_found = 0;
  let mut total_skipped = 0;
  let mut total_added = 0;
  let mut total_updated = 0;

  for folder in &folders {
    let _ = progress_sender.send(ScanProgress::StartingFolder(folder.clone()));

    let walker = WalkDir::new(folder)
      .into_iter()
      .filter_map(|e| e.ok());

    for entry in walker {
      if !entry.file_type().is_file() {
        continue;
      }

      let path_str = entry.path().display().to_string();
      total_found += 1;

      let _ = progress_sender.send(ScanProgress::FoundFile(total_found, total_skipped, path_str.clone()));

      // Skip files that are complete
      if existing_complete.contains(&path_str) {
        total_skipped += 1;
        continue;
      }

      let needs_update = existing_incomplete.contains(&path_str);

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
        let duration_seconds: Option<i32> = tagged_file
          .properties()
          .duration()
          .as_secs()
          .try_into()
          .ok();

        if needs_update {
          // Update existing record with duration
          let result = diesel::update(dsl::tracks.filter(dsl::filename.eq(&path_str)))
            .set(dsl::duration_seconds.eq(duration_seconds))
            .execute(&mut conn);

          if result.is_ok() {
            total_updated += 1;
          }
        } else {
          // Insert new record
          let result = diesel::insert_into(tracks::table)
            .values(NewTrack {
              filename: &path_str,
              artist: t.artist().as_deref(),
              album: t.album().as_deref(),
              album_artist: t.get_string(&ItemKey::AlbumArtist),
              title: t.title().as_deref(),
              track: t.get_string(&ItemKey::TrackNumber),
              genre: t.genre().as_deref(),
              duration_seconds,
            })
            .execute(&mut conn);

          if result.is_ok() {
            total_added += 1;
          }
        }
      }

      let _ = progress_sender.send(ScanProgress::ScannedFile(total_found, total_skipped, total_added, total_updated, path_str));
    }
  }

  let _ = progress_sender.send(ScanProgress::Complete(total_found, total_skipped, total_added, total_updated));
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

pub fn update_track_play_stats(path: &str) {
  use self::schema::tracks::dsl;

  let Ok(mut conn) = connect_db() else {
    return;
  };
  let _ = diesel::update(dsl::tracks.filter(dsl::filename.eq(path)))
    .set((
      dsl::play_count.eq(dsl::play_count + 1),
      dsl::last_played.eq(diesel::dsl::now),
    ))
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

pub fn create_playlist(name: &str) -> Result<i32, String> {
  use self::models::NewPlaylist;

  let mut conn = connect_db()?;

  diesel::insert_into(playlists::table)
    .values(NewPlaylist { name })
    .execute(&mut conn)
    .map_err(|e| format!("Failed to create playlist: {e}"))?;

  playlists::table
    .order(playlists::id.desc())
    .select(playlists::id)
    .first::<i32>(&mut conn)
    .map_err(|e| format!("Failed to get playlist id: {e}"))
}

pub fn get_user_playlists() -> Result<Vec<Rc<models::Playlist>>, String> {
  use self::models::Playlist;

  let mut conn = connect_db()?;

  playlists::table
    .order(playlists::name.asc())
    .load::<Playlist>(&mut conn)
    .map(|v| v.into_iter().map(Rc::new).collect())
    .map_err(|e| format!("Failed to load playlists: {e}"))
}

pub fn add_track_to_playlist(playlist_id: i32, track_filename: &str) -> Result<(), String> {
  use self::models::NewPlaylistTrack;

  let mut conn = connect_db()?;

  let max_position: Option<i32> = playlist_tracks::table
    .filter(playlist_tracks::playlist_id.eq(playlist_id))
    .select(diesel::dsl::max(playlist_tracks::position))
    .first(&mut conn)
    .map_err(|e| format!("Failed to get max position: {e}"))?;

  let next_position = max_position.unwrap_or(0) + 1;

  diesel::insert_into(playlist_tracks::table)
    .values(NewPlaylistTrack {
      playlist_id,
      track_filename: Some(track_filename),
      youtube_video_id: None,
      position: next_position,
    })
    .execute(&mut conn)
    .map_err(|e| format!("Failed to add track to playlist: {e}"))?;

  Ok(())
}

pub fn add_video_to_playlist(playlist_id: i32, video_id: i32) -> Result<(), String> {
  use self::models::NewPlaylistTrack;

  let mut conn = connect_db()?;

  let max_position: Option<i32> = playlist_tracks::table
    .filter(playlist_tracks::playlist_id.eq(playlist_id))
    .select(diesel::dsl::max(playlist_tracks::position))
    .first(&mut conn)
    .map_err(|e| format!("Failed to get max position: {e}"))?;

  let next_position = max_position.unwrap_or(0) + 1;

  diesel::insert_into(playlist_tracks::table)
    .values(NewPlaylistTrack {
      playlist_id,
      track_filename: None,
      youtube_video_id: Some(video_id),
      position: next_position,
    })
    .execute(&mut conn)
    .map_err(|e| format!("Failed to add video to playlist: {e}"))?;

  Ok(())
}

pub enum UserPlaylistItem {
  Track(Rc<Track>),
  Video(Rc<models::YouTubeVideo>),
}

pub fn get_playlist_items(playlist_id: i32) -> Result<Vec<UserPlaylistItem>, String> {
  let mut conn = connect_db()?;

  let items: Vec<models::PlaylistTrack> = playlist_tracks::table
    .filter(playlist_tracks::playlist_id.eq(playlist_id))
    .order(playlist_tracks::position.asc())
    .load(&mut conn)
    .map_err(|e| format!("Failed to load playlist items: {e}"))?;

  let mut result = Vec::new();
  for item in items {
    if let Some(filename) = &item.track_filename {
      let track: Option<Track> = tracks::table
        .filter(tracks::filename.eq(filename))
        .first(&mut conn)
        .ok();
      if let Some(t) = track {
        result.push(UserPlaylistItem::Track(Rc::new(t)));
      }
    }
    if let Some(vid_id) = item.youtube_video_id {
      let video: Option<models::YouTubeVideo> = youtube_videos::table
        .filter(youtube_videos::id.eq(vid_id))
        .first(&mut conn)
        .ok();
      if let Some(v) = video {
        result.push(UserPlaylistItem::Video(Rc::new(v)));
      }
    }
  }

  Ok(result)
}

pub fn delete_playlist(id: i32) -> Result<(), String> {
  let mut conn = connect_db()?;

  diesel::delete(playlists::table.filter(playlists::id.eq(id)))
    .execute(&mut conn)
    .map_err(|e| format!("Failed to delete playlist: {e}"))?;

  Ok(())
}

pub fn rename_playlist(id: i32, new_name: &str) -> Result<(), String> {
  let mut conn = connect_db()?;

  diesel::update(playlists::table.filter(playlists::id.eq(id)))
    .set(playlists::name.eq(new_name))
    .execute(&mut conn)
    .map_err(|e| format!("Failed to rename playlist: {e}"))?;

  Ok(())
}

pub fn remove_track_from_playlist(playlist_id: i32, track_filename: &str) -> Result<(), String> {
  let mut conn = connect_db()?;

  diesel::delete(
    playlist_tracks::table
      .filter(playlist_tracks::playlist_id.eq(playlist_id))
      .filter(playlist_tracks::track_filename.eq(track_filename)),
  )
  .execute(&mut conn)
  .map_err(|e| format!("Failed to remove track: {e}"))?;

  Ok(())
}

pub fn remove_video_from_playlist(playlist_id: i32, video_id: i32) -> Result<(), String> {
  let mut conn = connect_db()?;

  diesel::delete(
    playlist_tracks::table
      .filter(playlist_tracks::playlist_id.eq(playlist_id))
      .filter(playlist_tracks::youtube_video_id.eq(video_id)),
  )
  .execute(&mut conn)
  .map_err(|e| format!("Failed to remove video: {e}"))?;

  Ok(())
}

pub fn reorder_playlist_items(playlist_id: i32, items: &[PlaylistItemIdentifier]) -> Result<(), String> {
  let mut conn = connect_db()?;

  for (position, item) in items.iter().enumerate() {
    match item {
      PlaylistItemIdentifier::Track(filename) => {
        diesel::update(
          playlist_tracks::table
            .filter(playlist_tracks::playlist_id.eq(playlist_id))
            .filter(playlist_tracks::track_filename.eq(filename)),
        )
        .set(playlist_tracks::position.eq(position as i32))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to update track position: {e}"))?;
      }
      PlaylistItemIdentifier::Video(vid_id) => {
        diesel::update(
          playlist_tracks::table
            .filter(playlist_tracks::playlist_id.eq(playlist_id))
            .filter(playlist_tracks::youtube_video_id.eq(vid_id)),
        )
        .set(playlist_tracks::position.eq(position as i32))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to update video position: {e}"))?;
      }
    }
  }

  Ok(())
}

pub enum PlaylistItemIdentifier {
  Track(String),
  Video(i32),
}
