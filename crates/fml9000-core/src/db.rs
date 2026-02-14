use crate::media_item::MediaItem;
use crate::models::*;
use crate::schema::{playlist_tracks, playlists, tracks, youtube_channels, youtube_videos};
use crate::settings::get_project_dirs;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::Accessor;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;
use walkdir::WalkDir;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations");

#[derive(Hash, Eq, Ord, PartialEq, PartialOrd, Debug, Clone)]
pub struct Facet {
  pub album_artist_or_artist: Option<String>,
  pub album_artist: Option<String>,
  pub album: Option<String>,
  pub all: bool,
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
  /// Scan complete (total_found, skipped, added, updated, stale_files)
  Complete(usize, usize, usize, usize, Vec<String>),
}

const AUDIO_EXTENSIONS: &[&str] = &[
  "mp3", "flac", "ogg", "opus", "wav", "aac", "m4a", "wma", "aiff", "aif", "ape", "wv", "mpc",
  "mp4", "webm",
];

fn is_audio_file(path: &std::path::Path) -> bool {
  path
    .extension()
    .and_then(|ext| ext.to_str())
    .is_some_and(|ext| AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
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
  use crate::schema::tracks::dsl;

  let mut conn = match connect_db() {
    Ok(c) => c,
    Err(e) => {
      eprintln!("Warning: Could not connect to database for scanning: {e}");
      let _ = progress_sender.send(ScanProgress::Complete(0, 0, 0, 0, Vec::new()));
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
      .follow_links(false)
      .into_iter()
      .filter_map(|e| match e {
        Ok(entry) => Some(entry),
        Err(err) => {
          eprintln!("Warning: Failed to access path: {err}");
          None
        }
      });

    for entry in walker {
      if !entry.file_type().is_file() {
        continue;
      }

      if !is_audio_file(entry.path()) {
        continue;
      }

      let Some(path_str) = entry.path().to_str().map(|s| s.to_string()) else {
        eprintln!("Warning: Skipping file with non-UTF-8 path: {}", entry.path().display());
        continue;
      };
      total_found += 1;

      let _ = progress_sender.send(ScanProgress::FoundFile(total_found, total_skipped, path_str.clone()));

      // Skip files that are complete
      if existing_complete.contains(&path_str) {
        total_skipped += 1;
        continue;
      }

      let needs_update = existing_incomplete.contains(&path_str);

      let Ok(probe) = Probe::open(&path_str) else {
        eprintln!("Warning: Could not open file for probing: {path_str}");
        continue;
      };

      let Ok(tagged_file) = probe.read() else {
        eprintln!("Warning: Could not read tags from file: {path_str}");
        continue;
      };

      let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

      let duration_seconds: Option<i32> = tagged_file
        .properties()
        .duration()
        .as_secs()
        .try_into()
        .ok();

      if needs_update {
        // Update existing record with duration
        if let Err(e) = diesel::update(dsl::tracks.filter(dsl::filename.eq(&path_str)))
          .set(dsl::duration_seconds.eq(duration_seconds))
          .execute(&mut conn)
        {
          eprintln!("Warning: Failed to update track {path_str}: {e}");
        } else {
          total_updated += 1;
        }
      } else {
        let (artist, album, album_artist, title, track, genre) = if let Some(t) = tag {
          (
            t.artist().as_deref().map(str::to_string),
            t.album().as_deref().map(str::to_string),
            t.get_string(ItemKey::AlbumArtist).map(str::to_string),
            t.title().as_deref().map(str::to_string),
            t.get_string(ItemKey::TrackNumber).map(str::to_string),
            t.genre().as_deref().map(str::to_string),
          )
        } else {
          (None, None, None, None, None, None)
        };

        if let Err(e) = diesel::insert_into(tracks::table)
          .values(NewTrack {
            filename: &path_str,
            artist: artist.as_deref(),
            album: album.as_deref(),
            album_artist: album_artist.as_deref(),
            title: title.as_deref(),
            track: track.as_deref(),
            genre: genre.as_deref(),
            duration_seconds,
          })
          .execute(&mut conn)
        {
          eprintln!("Warning: Failed to insert track {path_str}: {e}");
        } else {
          total_added += 1;
        }
      }

      let _ = progress_sender.send(ScanProgress::ScannedFile(total_found, total_skipped, total_added, total_updated, path_str));
    }
  }

  // Detect stale tracks: files in DB that no longer exist on disk
  let all_db_files: Vec<String> = dsl::tracks
    .select(dsl::filename)
    .load::<String>(&mut conn)
    .unwrap_or_default();

  let scanned_folders: Vec<&str> = folders.iter().map(|s| s.as_str()).collect();
  let stale_files: Vec<String> = all_db_files
    .into_iter()
    .filter(|f| {
      scanned_folders.iter().any(|folder| f.starts_with(folder))
        && !std::path::Path::new(f).exists()
    })
    .collect();

  let _ = progress_sender.send(ScanProgress::Complete(total_found, total_skipped, total_added, total_updated, stale_files));
}

pub fn delete_tracks_by_filename(filenames: &[String]) -> Result<usize, String> {
  use crate::schema::tracks::dsl;

  with_db(|conn| {
    let mut deleted = 0;
    for filename in filenames {
      let count = diesel::delete(dsl::tracks.filter(dsl::filename.eq(filename)))
        .execute(conn)
        .map_err(|e| format!("Failed to delete track {filename}: {e}"))?;
      deleted += count;
    }
    Ok(deleted)
  })
}

pub fn mark_as_played(item: &MediaItem) {
  match item {
    MediaItem::Track(t) => {
      use crate::schema::tracks::dsl;
      if let Err(e) = with_db(|conn| {
        diesel::update(dsl::tracks.filter(dsl::filename.eq(&t.filename)))
          .set(dsl::last_played.eq(diesel::dsl::now))
          .execute(conn)
          .map_err(|e| e.to_string())?;
        Ok(())
      }) {
        eprintln!("Warning: Failed to mark track as played: {e}");
      }
    }
    MediaItem::Video(_) => {}
  }
}

pub fn update_play_stats(item: &MediaItem) {
  match item {
    MediaItem::Track(t) => {
      use crate::schema::tracks::dsl;
      if let Err(e) = with_db(|conn| {
        diesel::update(dsl::tracks.filter(dsl::filename.eq(&t.filename)))
          .set((
            dsl::play_count.eq(dsl::play_count + 1),
            dsl::last_played.eq(diesel::dsl::now),
          ))
          .execute(conn)
          .map_err(|e| e.to_string())?;
        Ok(())
      }) {
        eprintln!("Warning: Failed to update play stats for track: {e}");
      }
    }
    MediaItem::Video(v) => {
      use crate::schema::youtube_videos::dsl;
      if let Err(e) = with_db(|conn| {
        diesel::update(dsl::youtube_videos.filter(dsl::id.eq(v.id)))
          .set((
            dsl::play_count.eq(dsl::play_count + 1),
            dsl::last_played.eq(diesel::dsl::now),
          ))
          .execute(conn)
          .map_err(|e| e.to_string())?;
        Ok(())
      }) {
        eprintln!("Warning: Failed to update play stats for video: {e}");
      }
    }
  }
}

pub fn load_recently_played_items(limit: i64) -> Vec<MediaItem> {
  use crate::schema::tracks::dsl as t;
  use crate::schema::youtube_videos;

  with_db(|conn| {
    let tracks_result: Vec<Track> = t::tracks
      .filter(t::last_played.is_not_null())
      .select(Track::as_select())
      .load(conn)
      .unwrap_or_default();

    let videos: Vec<YouTubeVideo> = youtube_videos::table
      .filter(youtube_videos::last_played.is_not_null())
      .select(YouTubeVideo::as_select())
      .load(conn)
      .unwrap_or_default();

    let mut items: Vec<(MediaItem, chrono::NaiveDateTime)> = Vec::new();

    for track in tracks_result {
      if let Some(last_played) = track.last_played {
        items.push((MediaItem::Track(Arc::new(track)), last_played));
      }
    }

    for video in videos {
      if let Some(last_played) = video.last_played {
        items.push((MediaItem::Video(Arc::new(video)), last_played));
      }
    }

    items.sort_by(|a, b| b.1.cmp(&a.1));
    if limit > 0 {
      items.truncate(limit as usize);
    }
    Ok(items.into_iter().map(|(item, _)| item).collect())
  })
  .unwrap_or_default()
}

pub fn load_recently_added_items(limit: i64) -> Vec<MediaItem> {
  use crate::schema::tracks::dsl as t;
  use crate::schema::youtube_videos;

  with_db(|conn| {
    let tracks_result: Vec<Track> = t::tracks
      .select(Track::as_select())
      .order(t::added.desc())
      .load(conn)
      .unwrap_or_default();

    let videos: Vec<YouTubeVideo> = youtube_videos::table
      .select(YouTubeVideo::as_select())
      .order(youtube_videos::added.desc())
      .load(conn)
      .unwrap_or_default();

    let mut items: Vec<(MediaItem, chrono::NaiveDateTime)> = Vec::new();

    for track in tracks_result {
      let added = track.added.unwrap_or_default();
      items.push((MediaItem::Track(Arc::new(track)), added));
    }

    for video in videos {
      let added = video.added.unwrap_or(video.fetched_at);
      items.push((MediaItem::Video(Arc::new(video)), added));
    }

    items.sort_by(|a, b| b.1.cmp(&a.1));
    if limit > 0 {
      items.truncate(limit as usize);
    }
    Ok(items.into_iter().map(|(item, _)| item).collect())
  })
  .unwrap_or_default()
}

pub fn add_to_queue(item: &MediaItem) {
  use crate::schema::playback_queue;

  if let Err(e) = with_db(|conn| {
    let max_position: Option<i32> = playback_queue::table
      .select(diesel::dsl::max(playback_queue::position))
      .first(conn)
      .unwrap_or(None);

    let new_position = max_position.unwrap_or(-1) + 1;

    diesel::insert_into(playback_queue::table)
      .values(NewPlaybackQueueItem {
        position: new_position,
        track_filename: item.track_filename(),
        youtube_video_id: item.video_db_id(),
      })
      .execute(conn)
      .map_err(|e| e.to_string())?;
    Ok(())
  }) {
    eprintln!("Warning: Failed to add item to queue: {e}");
  }
}

pub fn remove_from_queue(item: &MediaItem) {
  use crate::schema::playback_queue;

  if let Err(e) = with_db(|conn| {
    match item {
      MediaItem::Track(t) => {
        diesel::delete(
          playback_queue::table.filter(playback_queue::track_filename.eq(&t.filename)),
        )
        .execute(conn)
        .map_err(|e| e.to_string())?;
      }
      MediaItem::Video(v) => {
        diesel::delete(
          playback_queue::table.filter(playback_queue::youtube_video_id.eq(v.id)),
        )
        .execute(conn)
        .map_err(|e| e.to_string())?;
      }
    }
    Ok(())
  }) {
    eprintln!("Warning: Failed to remove item from queue: {e}");
  }
}

pub fn pop_queue_front() -> Option<MediaItem> {
  use crate::schema::playback_queue;
  use crate::schema::tracks::dsl as t;
  use crate::schema::youtube_videos;

  with_db(|conn| {
    let item: Option<PlaybackQueueItem> = playback_queue::table
      .order(playback_queue::position.asc())
      .first(conn)
      .ok();

    if let Some(queue_item) = item {
      if let Err(e) = diesel::delete(playback_queue::table.filter(playback_queue::id.eq(queue_item.id)))
        .execute(conn)
      {
        eprintln!("Warning: Failed to remove queue item: {e}");
      }

      if let Some(filename) = queue_item.track_filename {
        return Ok(t::tracks
          .filter(t::filename.eq(&filename))
          .first::<Track>(conn)
          .ok()
          .map(|track| MediaItem::Track(Arc::new(track))));
      }
      if let Some(video_id) = queue_item.youtube_video_id {
        return Ok(youtube_videos::table
          .filter(youtube_videos::id.eq(video_id))
          .first::<YouTubeVideo>(conn)
          .ok()
          .map(|video| MediaItem::Video(Arc::new(video))));
      }
    }

    Ok(None)
  })
  .unwrap_or(None)
}

pub fn get_queue_items() -> Vec<MediaItem> {
  use crate::schema::{playback_queue, tracks, youtube_videos};

  with_db(|conn| {
    let queue_with_tracks: Vec<(PlaybackQueueItem, Option<Track>)> = playback_queue::table
      .left_join(tracks::table.on(playback_queue::track_filename.eq(tracks::filename.nullable())))
      .select((PlaybackQueueItem::as_select(), Option::<Track>::as_select()))
      .order(playback_queue::position.asc())
      .load(conn)
      .unwrap_or_default();

    let queue_with_videos: Vec<(PlaybackQueueItem, Option<YouTubeVideo>)> = playback_queue::table
      .left_join(youtube_videos::table.on(playback_queue::youtube_video_id.eq(youtube_videos::id.nullable())))
      .select((PlaybackQueueItem::as_select(), Option::<YouTubeVideo>::as_select()))
      .order(playback_queue::position.asc())
      .load(conn)
      .unwrap_or_default();

    let mut result = Vec::new();

    for (queue_item, track_opt) in queue_with_tracks {
      if let Some(track) = track_opt {
        result.push((queue_item.position, MediaItem::Track(Arc::new(track))));
      }
    }

    for (queue_item, video_opt) in queue_with_videos {
      if let Some(video) = video_opt {
        result.push((queue_item.position, MediaItem::Video(Arc::new(video))));
      }
    }

    result.sort_by_key(|(pos, _)| *pos);
    Ok(result.into_iter().map(|(_, item)| item).collect())
  })
  .unwrap_or_default()
}

pub fn clear_queue() {
  use crate::schema::playback_queue;

  if let Err(e) = with_db(|conn| {
    diesel::delete(playback_queue::table)
      .execute(conn)
      .map_err(|e| e.to_string())?;
    Ok(())
  }) {
    eprintln!("Warning: Failed to clear queue: {e}");
  }
}

pub fn queue_len() -> usize {
  use crate::schema::playback_queue;

  with_db(|conn| {
    playback_queue::table
      .count()
      .get_result::<i64>(conn)
      .map(|c| c as usize)
      .map_err(|e| e.to_string())
  })
  .unwrap_or(0)
}

pub fn load_tracks() -> Result<Vec<Arc<Track>>, String> {
  use crate::schema::tracks::dsl::*;

  with_db(|conn| {
    tracks
      .load::<Track>(conn)
      .map(|v| v.into_iter().map(Arc::new).collect())
      .map_err(|e| format!("Error loading tracks: {e}"))
  })
}

pub fn load_track_by_filename(path: &str) -> Option<Arc<Track>> {
  use crate::schema::tracks::dsl::*;

  with_db(|conn| {
    Ok(tracks
      .filter(filename.eq(path))
      .first::<Track>(conn)
      .ok()
      .map(Arc::new))
  })
  .unwrap_or(None)
}

pub fn load_video_by_id(vid_id: i32) -> Option<Arc<YouTubeVideo>> {
  use crate::schema::youtube_videos;

  with_db(|conn| {
    Ok(youtube_videos::table
      .filter(youtube_videos::id.eq(vid_id))
      .first::<YouTubeVideo>(conn)
      .ok()
      .map(Arc::new))
  })
  .unwrap_or(None)
}

pub fn load_video_by_video_id(vid_id: &str) -> Option<Arc<YouTubeVideo>> {
  use crate::schema::youtube_videos;

  with_db(|conn| {
    Ok(youtube_videos::table
      .filter(youtube_videos::video_id.eq(vid_id))
      .first::<YouTubeVideo>(conn)
      .ok()
      .map(Arc::new))
  })
  .unwrap_or(None)
}

pub fn build_facets(tracks_list: &[Arc<Track>]) -> Vec<Facet> {
  let mut facets = HashSet::new();
  for row in tracks_list {
    facets.insert(Facet {
      album: row.album.clone(),
      album_artist: row.album_artist.clone(),
      album_artist_or_artist: row.album_artist.clone().or(row.artist.clone()),
      all: false,
    });
  }
  let mut v: Vec<Facet> = vec![Facet {
    album: None,
    album_artist: None,
    album_artist_or_artist: None,
    all: true,
  }];
  let mut sorted: Vec<Facet> = facets.into_iter().collect();
  sorted.sort();
  v.extend(sorted);
  v
}

pub fn add_youtube_channel(
  channel_id: &str,
  name: &str,
  handle: Option<&str>,
  url: &str,
  thumbnail_url: Option<&str>,
) -> Result<i32, String> {
  use crate::models::NewYouTubeChannel;

  with_db(|conn| {
    diesel::insert_into(youtube_channels::table)
      .values(NewYouTubeChannel {
        channel_id,
        name,
        handle,
        url,
        thumbnail_url,
      })
      .execute(conn)
      .map_err(|e| format!("Failed to insert channel: {e}"))?;

    youtube_channels::table
      .filter(youtube_channels::channel_id.eq(channel_id))
      .select(youtube_channels::id)
      .first::<i32>(conn)
      .map_err(|e| format!("Failed to get channel id: {e}"))
  })
}

pub fn get_youtube_channels() -> Result<Vec<Arc<YouTubeChannel>>, String> {
  use crate::models::YouTubeChannel;

  with_db(|conn| {
    youtube_channels::table
      .load::<YouTubeChannel>(conn)
      .map(|v| v.into_iter().map(Arc::new).collect())
      .map_err(|e| format!("Failed to load channels: {e}"))
  })
}

pub fn delete_youtube_channel(id: i32) -> Result<(), String> {
  with_db(|conn| {
    diesel::delete(youtube_channels::table.filter(youtube_channels::id.eq(id)))
      .execute(conn)
      .map_err(|e| format!("Failed to delete channel: {e}"))?;
    Ok(())
  })
}

pub fn add_youtube_videos(
  db_channel_id: i32,
  videos: &[(String, String, Option<i32>, Option<String>, Option<chrono::NaiveDateTime>)],
) -> Result<(), String> {
  use crate::models::NewYouTubeVideo;

  with_db(|conn| {
    for (video_id, title, duration, thumbnail, published_at) in videos {
      if let Err(e) = diesel::insert_or_ignore_into(youtube_videos::table)
        .values(NewYouTubeVideo {
          video_id,
          channel_id: db_channel_id,
          title,
          duration_seconds: *duration,
          thumbnail_url: thumbnail.as_deref(),
          published_at: *published_at,
        })
        .execute(conn)
      {
        eprintln!("Warning: Failed to insert video {video_id}: {e}");
      }
    }
    Ok(())
  })
}

pub fn get_all_media() -> Vec<MediaItem> {
  let mut items: Vec<MediaItem> = load_tracks()
    .unwrap_or_default()
    .into_iter()
    .map(MediaItem::Track)
    .collect();
  for v in get_all_videos().unwrap_or_default() {
    items.push(MediaItem::Video(v));
  }
  items
}

pub fn get_all_videos() -> Result<Vec<Arc<YouTubeVideo>>, String> {
  use crate::models::YouTubeVideo;

  with_db(|conn| {
    youtube_videos::table
      .order(youtube_videos::published_at.desc())
      .load::<YouTubeVideo>(conn)
      .map(|v| v.into_iter().map(Arc::new).collect())
      .map_err(|e| format!("Failed to load videos: {e}"))
  })
}

pub fn get_videos_for_channel(db_channel_id: i32) -> Result<Vec<Arc<YouTubeVideo>>, String> {
  use crate::models::YouTubeVideo;

  with_db(|conn| {
    youtube_videos::table
      .filter(youtube_videos::channel_id.eq(db_channel_id))
      .order(youtube_videos::published_at.desc())
      .load::<YouTubeVideo>(conn)
      .map(|v| v.into_iter().map(Arc::new).collect())
      .map_err(|e| format!("Failed to load videos: {e}"))
  })
}

pub fn update_channel_last_fetched(id: i32) -> Result<(), String> {
  with_db(|conn| {
    diesel::update(youtube_channels::table.filter(youtube_channels::id.eq(id)))
      .set(youtube_channels::last_fetched.eq(diesel::dsl::now))
      .execute(conn)
      .map_err(|e| format!("Failed to update last_fetched: {e}"))?;
    Ok(())
  })
}

pub fn get_video_count_for_channel(db_channel_id: i32) -> Result<i64, String> {
  with_db(|conn| {
    youtube_videos::table
      .filter(youtube_videos::channel_id.eq(db_channel_id))
      .count()
      .get_result::<i64>(conn)
      .map_err(|e| format!("Failed to count videos: {e}"))
  })
}

pub fn get_video_ids_for_channel(db_channel_id: i32) -> Result<std::collections::HashSet<String>, String> {
  with_db(|conn| {
    youtube_videos::table
      .filter(youtube_videos::channel_id.eq(db_channel_id))
      .select(youtube_videos::video_id)
      .load::<String>(conn)
      .map(|v| v.into_iter().collect())
      .map_err(|e| format!("Failed to load video IDs: {e}"))
  })
}

pub fn create_playlist(name: &str) -> Result<i32, String> {
  use crate::models::NewPlaylist;

  with_db(|conn| {
    diesel::insert_into(playlists::table)
      .values(NewPlaylist { name })
      .execute(conn)
      .map_err(|e| format!("Failed to create playlist: {e}"))?;

    playlists::table
      .order(playlists::id.desc())
      .select(playlists::id)
      .first::<i32>(conn)
      .map_err(|e| format!("Failed to get playlist id: {e}"))
  })
}

pub fn get_user_playlists() -> Result<Vec<Arc<Playlist>>, String> {
  use crate::models::Playlist;

  with_db(|conn| {
    playlists::table
      .order(playlists::name.asc())
      .load::<Playlist>(conn)
      .map(|v| v.into_iter().map(Arc::new).collect())
      .map_err(|e| format!("Failed to load playlists: {e}"))
  })
}

pub fn add_to_playlist(playlist_id: i32, item: &MediaItem) -> Result<(), String> {
  use crate::models::NewPlaylistTrack;

  with_db(|conn| {
    let max_position: Option<i32> = playlist_tracks::table
      .filter(playlist_tracks::playlist_id.eq(playlist_id))
      .select(diesel::dsl::max(playlist_tracks::position))
      .first(conn)
      .map_err(|e| format!("Failed to get max position: {e}"))?;

    let next_position = max_position.unwrap_or(0) + 1;

    diesel::insert_into(playlist_tracks::table)
      .values(NewPlaylistTrack {
        playlist_id,
        track_filename: item.track_filename(),
        youtube_video_id: item.video_db_id(),
        position: next_position,
      })
      .execute(conn)
      .map_err(|e| format!("Failed to add item to playlist: {e}"))?;
    Ok(())
  })
}

pub fn get_playlist_items(playlist_id: i32) -> Result<Vec<MediaItem>, String> {
  with_db(|conn| {
    let items_with_tracks: Vec<(PlaylistTrack, Option<Track>)> = playlist_tracks::table
      .filter(playlist_tracks::playlist_id.eq(playlist_id))
      .left_join(tracks::table.on(playlist_tracks::track_filename.eq(tracks::filename.nullable())))
      .select((PlaylistTrack::as_select(), Option::<Track>::as_select()))
      .order(playlist_tracks::position.asc())
      .load(conn)
      .map_err(|e| format!("Failed to load playlist items: {e}"))?;

    let items_with_videos: Vec<(PlaylistTrack, Option<YouTubeVideo>)> = playlist_tracks::table
      .filter(playlist_tracks::playlist_id.eq(playlist_id))
      .left_join(youtube_videos::table.on(playlist_tracks::youtube_video_id.eq(youtube_videos::id.nullable())))
      .select((PlaylistTrack::as_select(), Option::<YouTubeVideo>::as_select()))
      .order(playlist_tracks::position.asc())
      .load(conn)
      .map_err(|e| format!("Failed to load playlist videos: {e}"))?;

    let mut result: Vec<(i32, MediaItem)> = Vec::new();

    for (playlist_item, track_opt) in items_with_tracks {
      if let Some(track) = track_opt {
        result.push((playlist_item.position, MediaItem::Track(Arc::new(track))));
      }
    }

    for (playlist_item, video_opt) in items_with_videos {
      if let Some(video) = video_opt {
        result.push((playlist_item.position, MediaItem::Video(Arc::new(video))));
      }
    }

    result.sort_by_key(|(pos, _)| *pos);
    Ok(result.into_iter().map(|(_, item)| item).collect())
  })
}

pub fn delete_playlist(id: i32) -> Result<(), String> {
  with_db(|conn| {
    diesel::delete(playlists::table.filter(playlists::id.eq(id)))
      .execute(conn)
      .map_err(|e| format!("Failed to delete playlist: {e}"))?;
    Ok(())
  })
}

pub fn rename_playlist(id: i32, new_name: &str) -> Result<(), String> {
  with_db(|conn| {
    diesel::update(playlists::table.filter(playlists::id.eq(id)))
      .set(playlists::name.eq(new_name))
      .execute(conn)
      .map_err(|e| format!("Failed to rename playlist: {e}"))?;
    Ok(())
  })
}

pub fn remove_from_playlist(playlist_id: i32, item: &MediaItem) -> Result<(), String> {
  with_db(|conn| {
    match item {
      MediaItem::Track(t) => {
        diesel::delete(
          playlist_tracks::table
            .filter(playlist_tracks::playlist_id.eq(playlist_id))
            .filter(playlist_tracks::track_filename.eq(&t.filename)),
        )
        .execute(conn)
        .map_err(|e| format!("Failed to remove track: {e}"))?;
      }
      MediaItem::Video(v) => {
        diesel::delete(
          playlist_tracks::table
            .filter(playlist_tracks::playlist_id.eq(playlist_id))
            .filter(playlist_tracks::youtube_video_id.eq(v.id)),
        )
        .execute(conn)
        .map_err(|e| format!("Failed to remove video: {e}"))?;
      }
    }
    Ok(())
  })
}

pub enum PlaylistItemIdentifier {
  Track(String),
  Video(i32),
}

pub fn reorder_playlist_items(playlist_id: i32, items: &[PlaylistItemIdentifier]) -> Result<(), String> {
  with_db(|conn| {
    for (position, item) in items.iter().enumerate() {
      match item {
        PlaylistItemIdentifier::Track(filename) => {
          diesel::update(
            playlist_tracks::table
              .filter(playlist_tracks::playlist_id.eq(playlist_id))
              .filter(playlist_tracks::track_filename.eq(filename)),
          )
          .set(playlist_tracks::position.eq(position as i32))
          .execute(conn)
          .map_err(|e| format!("Failed to update track position: {e}"))?;
        }
        PlaylistItemIdentifier::Video(vid_id) => {
          diesel::update(
            playlist_tracks::table
              .filter(playlist_tracks::playlist_id.eq(playlist_id))
              .filter(playlist_tracks::youtube_video_id.eq(vid_id)),
          )
          .set(playlist_tracks::position.eq(position as i32))
          .execute(conn)
          .map_err(|e| format!("Failed to update video position: {e}"))?;
        }
      }
    }
    Ok(())
  })
}
