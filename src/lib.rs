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
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use walkdir::WalkDir;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

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

fn get_database_url() -> Result<String, String> {
  let proj_dirs =
    get_project_dirs().ok_or_else(|| "Could not determine config directory".to_string())?;
  let config_dir = proj_dirs.config_dir();
  std::fs::create_dir_all(config_dir)
    .map_err(|e| format!("Failed to create config directory: {e}"))?;
  let path = config_dir.join("library.db");
  let path_str = path
    .to_str()
    .ok_or_else(|| "Database path contains invalid UTF-8".to_string())?;
  Ok(format!("sqlite://{}", path_str))
}

/// Initialize the database and run migrations. Call once at startup.
pub fn init_db() -> Result<(), String> {
  let database_url = get_database_url()?;
  let mut conn = SqliteConnection::establish(&database_url)
    .map_err(|e| format!("Error connecting to database: {e}"))?;
  conn
    .run_pending_migrations(MIGRATIONS)
    .map_err(|e| format!("Failed to run migrations: {e}"))?;
  Ok(())
}

thread_local! {
  static DB_CONNECTION: RefCell<Option<SqliteConnection>> = const { RefCell::new(None) };
}

/// Get a database connection. Uses a cached thread-local connection.
pub fn connect_db() -> Result<SqliteConnection, String> {
  let database_url = get_database_url()?;
  SqliteConnection::establish(&database_url)
    .map_err(|e| format!("Error connecting to database: {e}"))
}

/// Execute a database operation using a cached thread-local connection.
/// This avoids opening a new connection for each operation.
pub fn with_db<T, F>(f: F) -> Result<T, String>
where
  F: FnOnce(&mut SqliteConnection) -> Result<T, String>,
{
  DB_CONNECTION.with(|cell| {
    let mut conn_opt = cell.borrow_mut();
    if conn_opt.is_none() {
      let database_url = get_database_url()?;
      let conn = SqliteConnection::establish(&database_url)
        .map_err(|e| format!("Error connecting to database: {e}"))?;
      *conn_opt = Some(conn);
    }
    f(conn_opt.as_mut().unwrap())
  })
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
  use self::schema::tracks::dsl;

  let _ = with_db(|conn| {
    diesel::update(dsl::tracks.filter(dsl::filename.eq(path)))
      .set(dsl::last_played.eq(diesel::dsl::now))
      .execute(conn)
      .map_err(|e| e.to_string())?;
    Ok(())
  });
}

pub fn update_track_play_stats(path: &str) {
  use self::schema::tracks::dsl;

  let _ = with_db(|conn| {
    diesel::update(dsl::tracks.filter(dsl::filename.eq(path)))
      .set((
        dsl::play_count.eq(dsl::play_count + 1),
        dsl::last_played.eq(diesel::dsl::now),
      ))
      .execute(conn)
      .map_err(|e| e.to_string())?;
    Ok(())
  });
}

pub fn update_video_play_stats(video_id: i32) {
  use self::schema::youtube_videos::dsl;

  let _ = with_db(|conn| {
    diesel::update(dsl::youtube_videos.filter(dsl::id.eq(video_id)))
      .set((
        dsl::play_count.eq(dsl::play_count + 1),
        dsl::last_played.eq(diesel::dsl::now),
      ))
      .execute(conn)
      .map_err(|e| e.to_string())?;
    Ok(())
  });
}

pub fn load_recently_played_items(limit: i64) -> Vec<QueueItem> {
  use self::schema::tracks::dsl as t;
  use self::schema::youtube_videos;

  let Ok(mut conn) = connect_db() else {
    return Vec::new();
  };

  let tracks: Vec<Track> = t::tracks
    .filter(t::last_played.is_not_null())
    .select(Track::as_select())
    .load(&mut conn)
    .unwrap_or_default();

  let videos: Vec<models::YouTubeVideo> = youtube_videos::table
    .filter(youtube_videos::last_played.is_not_null())
    .select(models::YouTubeVideo::as_select())
    .load(&mut conn)
    .unwrap_or_default();

  let mut items: Vec<(QueueItem, chrono::NaiveDateTime)> = Vec::new();

  for track in tracks {
    if let Some(last_played) = track.last_played {
      items.push((QueueItem::Track(Rc::new(track)), last_played));
    }
  }

  for video in videos {
    if let Some(last_played) = video.last_played {
      items.push((QueueItem::Video(Rc::new(video)), last_played));
    }
  }

  items.sort_by(|a, b| b.1.cmp(&a.1));
  if limit > 0 {
    items.truncate(limit as usize);
  }
  items.into_iter().map(|(item, _)| item).collect()
}

pub fn load_recently_added_items(limit: i64) -> Vec<QueueItem> {
  use self::schema::tracks::dsl as t;
  use self::schema::youtube_videos;

  let Ok(mut conn) = connect_db() else {
    return Vec::new();
  };

  let tracks: Vec<Track> = t::tracks
    .select(Track::as_select())
    .order(t::added.desc())
    .load(&mut conn)
    .unwrap_or_default();

  let videos: Vec<models::YouTubeVideo> = youtube_videos::table
    .select(models::YouTubeVideo::as_select())
    .order(youtube_videos::added.desc())
    .load(&mut conn)
    .unwrap_or_default();

  let mut items: Vec<(QueueItem, chrono::NaiveDateTime)> = Vec::new();

  for track in tracks {
    let added = track.added.unwrap_or_default();
    items.push((QueueItem::Track(Rc::new(track)), added));
  }

  for video in videos {
    let added = video.added.unwrap_or(video.fetched_at);
    items.push((QueueItem::Video(Rc::new(video)), added));
  }

  items.sort_by(|a, b| b.1.cmp(&a.1));
  if limit > 0 {
    items.truncate(limit as usize);
  }
  items.into_iter().map(|(item, _)| item).collect()
}

pub enum QueueItem {
  Track(Rc<Track>),
  Video(Rc<models::YouTubeVideo>),
}

pub fn add_track_to_queue(track_filename: &str) {
  use self::schema::playback_queue;

  let _ = with_db(|conn| {
    let max_position: Option<i32> = playback_queue::table
      .select(diesel::dsl::max(playback_queue::position))
      .first(conn)
      .unwrap_or(None);

    let new_position = max_position.unwrap_or(-1) + 1;

    diesel::insert_into(playback_queue::table)
      .values(models::NewPlaybackQueueItem {
        position: new_position,
        track_filename: Some(track_filename),
        youtube_video_id: None,
      })
      .execute(conn)
      .map_err(|e| e.to_string())?;
    Ok(())
  });
}

pub fn add_video_to_queue(video_id: i32) {
  use self::schema::playback_queue;

  let _ = with_db(|conn| {
    let max_position: Option<i32> = playback_queue::table
      .select(diesel::dsl::max(playback_queue::position))
      .first(conn)
      .unwrap_or(None);

    let new_position = max_position.unwrap_or(-1) + 1;

    diesel::insert_into(playback_queue::table)
      .values(models::NewPlaybackQueueItem {
        position: new_position,
        track_filename: None,
        youtube_video_id: Some(video_id),
      })
      .execute(conn)
      .map_err(|e| e.to_string())?;
    Ok(())
  });
}

pub fn remove_track_from_queue(track_filename: &str) {
  use self::schema::playback_queue;

  let _ = with_db(|conn| {
    diesel::delete(
      playback_queue::table.filter(playback_queue::track_filename.eq(track_filename)),
    )
    .execute(conn)
    .map_err(|e| e.to_string())?;
    Ok(())
  });
}

pub fn remove_video_from_queue(video_id: i32) {
  use self::schema::playback_queue;

  let _ = with_db(|conn| {
    diesel::delete(
      playback_queue::table.filter(playback_queue::youtube_video_id.eq(video_id)),
    )
    .execute(conn)
    .map_err(|e| e.to_string())?;
    Ok(())
  });
}

pub fn pop_queue_front() -> Option<QueueItem> {
  use self::schema::playback_queue;

  let Ok(mut conn) = connect_db() else {
    return None;
  };

  let item: Option<models::PlaybackQueueItem> = playback_queue::table
    .order(playback_queue::position.asc())
    .first(&mut conn)
    .ok();

  if let Some(queue_item) = item {
    let _ = diesel::delete(playback_queue::table.filter(playback_queue::id.eq(queue_item.id)))
      .execute(&mut conn);

    if let Some(filename) = queue_item.track_filename {
      return load_track_by_filename(&filename).map(QueueItem::Track);
    }
    if let Some(video_id) = queue_item.youtube_video_id {
      return load_video_by_id(video_id).map(QueueItem::Video);
    }
  }

  None
}

pub fn get_queue_items() -> Vec<QueueItem> {
  use self::schema::{playback_queue, tracks, youtube_videos};

  let Ok(mut conn) = connect_db() else {
    return Vec::new();
  };

  let queue_with_tracks: Vec<(models::PlaybackQueueItem, Option<Track>)> = playback_queue::table
    .left_join(tracks::table.on(playback_queue::track_filename.eq(tracks::filename.nullable())))
    .select((models::PlaybackQueueItem::as_select(), Option::<Track>::as_select()))
    .order(playback_queue::position.asc())
    .load(&mut conn)
    .unwrap_or_default();

  let queue_with_videos: Vec<(models::PlaybackQueueItem, Option<models::YouTubeVideo>)> = playback_queue::table
    .left_join(youtube_videos::table.on(playback_queue::youtube_video_id.eq(youtube_videos::id.nullable())))
    .select((models::PlaybackQueueItem::as_select(), Option::<models::YouTubeVideo>::as_select()))
    .order(playback_queue::position.asc())
    .load(&mut conn)
    .unwrap_or_default();

  let mut result = Vec::new();

  for (queue_item, track_opt) in queue_with_tracks {
    if let Some(track) = track_opt {
      result.push((queue_item.position, QueueItem::Track(Rc::new(track))));
    }
  }

  for (queue_item, video_opt) in queue_with_videos {
    if let Some(video) = video_opt {
      result.push((queue_item.position, QueueItem::Video(Rc::new(video))));
    }
  }

  result.sort_by_key(|(pos, _)| *pos);
  result.into_iter().map(|(_, item)| item).collect()
}

pub fn clear_queue() {
  use self::schema::playback_queue;

  let _ = with_db(|conn| {
    diesel::delete(playback_queue::table)
      .execute(conn)
      .map_err(|e| e.to_string())?;
    Ok(())
  });
}

pub fn queue_len() -> usize {
  use self::schema::playback_queue;

  with_db(|conn| {
    playback_queue::table
      .count()
      .get_result::<i64>(conn)
      .map(|c| c as usize)
      .map_err(|e| e.to_string())
  })
  .unwrap_or(0)
}

pub fn load_tracks() -> Result<Vec<Rc<Track>>, String> {
  use self::schema::tracks::dsl::*;

  let mut conn = connect_db()?;

  tracks
    .load::<Track>(&mut conn)
    .map(|v| v.into_iter().map(Rc::new).collect())
    .map_err(|e| format!("Error loading tracks: {e}"))
}

pub fn load_track_by_filename(path: &str) -> Option<Rc<Track>> {
  use self::schema::tracks::dsl::*;

  let mut conn = connect_db().ok()?;

  tracks
    .filter(filename.eq(path))
    .first::<Track>(&mut conn)
    .ok()
    .map(Rc::new)
}

pub fn load_video_by_id(vid_id: i32) -> Option<Rc<models::YouTubeVideo>> {
  use self::schema::youtube_videos;

  let mut conn = connect_db().ok()?;

  youtube_videos::table
    .filter(youtube_videos::id.eq(vid_id))
    .first::<models::YouTubeVideo>(&mut conn)
    .ok()
    .map(Rc::new)
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

pub fn get_video_ids_for_channel(db_channel_id: i32) -> Result<std::collections::HashSet<String>, String> {
  let mut conn = connect_db()?;

  youtube_videos::table
    .filter(youtube_videos::channel_id.eq(db_channel_id))
    .select(youtube_videos::video_id)
    .load::<String>(&mut conn)
    .map(|v| v.into_iter().collect())
    .map_err(|e| format!("Failed to load video IDs: {e}"))
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

pub fn get_playlist_items(playlist_id: i32) -> Result<Vec<QueueItem>, String> {
  let mut conn = connect_db()?;

  let items_with_tracks: Vec<(models::PlaylistTrack, Option<Track>)> = playlist_tracks::table
    .filter(playlist_tracks::playlist_id.eq(playlist_id))
    .left_join(tracks::table.on(playlist_tracks::track_filename.eq(tracks::filename.nullable())))
    .select((models::PlaylistTrack::as_select(), Option::<Track>::as_select()))
    .order(playlist_tracks::position.asc())
    .load(&mut conn)
    .map_err(|e| format!("Failed to load playlist items: {e}"))?;

  let items_with_videos: Vec<(models::PlaylistTrack, Option<models::YouTubeVideo>)> = playlist_tracks::table
    .filter(playlist_tracks::playlist_id.eq(playlist_id))
    .left_join(youtube_videos::table.on(playlist_tracks::youtube_video_id.eq(youtube_videos::id.nullable())))
    .select((models::PlaylistTrack::as_select(), Option::<models::YouTubeVideo>::as_select()))
    .order(playlist_tracks::position.asc())
    .load(&mut conn)
    .map_err(|e| format!("Failed to load playlist videos: {e}"))?;

  let mut result: Vec<(i32, QueueItem)> = Vec::new();

  for (playlist_item, track_opt) in items_with_tracks {
    if let Some(track) = track_opt {
      result.push((playlist_item.position, QueueItem::Track(Rc::new(track))));
    }
  }

  for (playlist_item, video_opt) in items_with_videos {
    if let Some(video) = video_opt {
      result.push((playlist_item.position, QueueItem::Video(Rc::new(video))));
    }
  }

  result.sort_by_key(|(pos, _)| *pos);
  Ok(result.into_iter().map(|(_, item)| item).collect())
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
