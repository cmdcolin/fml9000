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
use std::sync::{Arc, OnceLock};
use walkdir::WalkDir;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations");

#[derive(Hash, Eq, Ord, PartialEq, PartialOrd, Debug, Clone)]
pub struct Facet {
  pub album_artist_or_artist: Option<String>,
  pub album_artist: Option<String>,
  pub album: Option<String>,
  pub all: bool,
}

static DATABASE_URL: OnceLock<String> = OnceLock::new();

fn get_database_url() -> Result<&'static str, String> {
  if let Some(url) = DATABASE_URL.get() {
    return Ok(url.as_str());
  }
  let proj_dirs =
    get_project_dirs().ok_or_else(|| "Could not determine config directory".to_string())?;
  let config_dir = proj_dirs.config_dir();
  std::fs::create_dir_all(config_dir)
    .map_err(|e| format!("Failed to create config directory: {e}"))?;
  let path = config_dir.join("library.db");
  let path_str = path
    .to_str()
    .ok_or_else(|| "Database path contains invalid UTF-8".to_string())?;
  let url = format!("sqlite://{}", path_str);
  let _ = DATABASE_URL.set(url);
  Ok(DATABASE_URL.get().unwrap().as_str())
}

fn configure_connection(conn: &mut SqliteConnection) {
  let _ = diesel::sql_query("PRAGMA journal_mode=WAL;").execute(conn);
  let _ = diesel::sql_query("PRAGMA busy_timeout=5000;").execute(conn);
}

pub fn init_db() -> Result<(), String> {
  let database_url = get_database_url()?;
  let mut conn = SqliteConnection::establish(database_url)
    .map_err(|e| format!("Error connecting to database: {e}"))?;
  configure_connection(&mut conn);
  conn
    .run_pending_migrations(MIGRATIONS)
    .map_err(|e| format!("Failed to run migrations: {e}"))?;
  Ok(())
}

thread_local! {
  static DB_CONNECTION: RefCell<Option<SqliteConnection>> = const { RefCell::new(None) };
}

pub fn connect_db() -> Result<SqliteConnection, String> {
  let database_url = get_database_url()?;
  let mut conn = SqliteConnection::establish(database_url)
    .map_err(|e| format!("Error connecting to database: {e}"))?;
  configure_connection(&mut conn);
  Ok(conn)
}

pub fn with_db<T, F>(f: F) -> Result<T, String>
where
  F: FnOnce(&mut SqliteConnection) -> Result<T, String>,
{
  DB_CONNECTION.with(|cell| {
    let mut conn_opt = cell.borrow_mut();
    if conn_opt.is_none() {
      let database_url = get_database_url()?;
      let mut conn = SqliteConnection::establish(database_url)
        .map_err(|e| format!("Error connecting to database: {e}"))?;
      configure_connection(&mut conn);
      *conn_opt = Some(conn);
    }
    f(conn_opt.as_mut().unwrap())
  })
}


#[derive(Clone)]
pub enum ScanProgress {
  StartingFolder { folder: String },
  FoundFile { total_found: usize, skipped: usize, current_file: String },
  ScannedFile { total_found: usize, skipped: usize, added: usize, updated: usize, current_file: String },
  Complete { total_found: usize, skipped: usize, added: usize, updated: usize, stale_files: Vec<String> },
}

const AUDIO_EXTENSIONS: &[&str] = &[
  "mp3", "flac", "ogg", "opus", "wav", "aac", "m4a", "wma", "aiff", "aif", "ape", "wv", "mpc",
  "mp4", "webm",
];

pub fn is_audio_file(path: &std::path::Path) -> bool {
  path
    .extension()
    .and_then(|ext| ext.to_str())
    .is_some_and(|ext| AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

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
      let _ = progress_sender.send(ScanProgress::Complete { total_found: 0, skipped: 0, added: 0, updated: 0, stale_files: Vec::new() });
      return;
    }
  };

  let mut total_found = 0;
  let mut total_skipped = 0;
  let mut total_added = 0;
  let mut total_updated = 0;

  for folder in &folders {
    let _ = progress_sender.send(ScanProgress::StartingFolder { folder: folder.clone() });

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

      let _ = progress_sender.send(ScanProgress::FoundFile { total_found, skipped: total_skipped, current_file: path_str.clone() });

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

      let _ = progress_sender.send(ScanProgress::ScannedFile { total_found, skipped: total_skipped, added: total_added, updated: total_updated, current_file: path_str });
    }
  }

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

  let _ = progress_sender.send(ScanProgress::Complete { total_found, skipped: total_skipped, added: total_added, updated: total_updated, stale_files });
}

pub fn scan_single_file(path: &std::path::Path) -> Result<Option<Arc<Track>>, String> {
  use crate::schema::tracks::dsl;

  if !is_audio_file(path) {
    return Ok(None);
  }

  let path_str = path.to_str().ok_or_else(|| "Non-UTF-8 path".to_string())?;

  with_db(|conn| {
    let existing: Option<Track> = dsl::tracks
      .filter(dsl::filename.eq(path_str))
      .first(conn)
      .optional()
      .map_err(|e| format!("DB query error: {e}"))?;

    if existing.is_some() {
      return Ok(None);
    }

    let probe = Probe::open(path_str).map_err(|e| format!("Could not open file: {e}"))?;
    let tagged_file = probe.read().map_err(|e| format!("Could not read tags: {e}"))?;

    let tag = tagged_file
      .primary_tag()
      .or_else(|| tagged_file.first_tag());

    let duration_seconds: Option<i32> = tagged_file
      .properties()
      .duration()
      .as_secs()
      .try_into()
      .ok();

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

    diesel::insert_into(tracks::table)
      .values(NewTrack {
        filename: path_str,
        artist: artist.as_deref(),
        album: album.as_deref(),
        album_artist: album_artist.as_deref(),
        title: title.as_deref(),
        track: track.as_deref(),
        genre: genre.as_deref(),
        duration_seconds,
      })
      .execute(conn)
      .map_err(|e| format!("Failed to insert track: {e}"))?;

    let inserted: Track = dsl::tracks
      .filter(dsl::filename.eq(path_str))
      .first(conn)
      .map_err(|e| format!("Failed to load inserted track: {e}"))?;

    Ok(Some(Arc::new(inserted)))
  })
}

pub fn delete_tracks_by_filename(filenames: &[String]) -> Result<usize, String> {
  use crate::schema::tracks::dsl;

  with_db(|conn| {
    diesel::delete(dsl::tracks.filter(dsl::filename.eq_any(filenames)))
      .execute(conn)
      .map_err(|e| format!("Failed to delete tracks: {e}"))
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
      .order(t::last_played.desc())
      .limit(limit)
      .load(conn)
      .unwrap_or_default();

    let videos: Vec<YouTubeVideo> = youtube_videos::table
      .filter(youtube_videos::last_played.is_not_null())
      .select(YouTubeVideo::as_select())
      .order(youtube_videos::last_played.desc())
      .limit(limit)
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
      .limit(limit)
      .load(conn)
      .unwrap_or_default();

    let videos: Vec<YouTubeVideo> = youtube_videos::table
      .select(YouTubeVideo::as_select())
      .order(youtube_videos::added.desc())
      .limit(limit)
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
    let rows: Vec<(PlaybackQueueItem, Option<Track>, Option<YouTubeVideo>)> = playback_queue::table
      .left_join(tracks::table)
      .left_join(youtube_videos::table)
      .select((PlaybackQueueItem::as_select(), Option::<Track>::as_select(), Option::<YouTubeVideo>::as_select()))
      .order(playback_queue::position.asc())
      .load(conn)
      .unwrap_or_default();

    let mut result = Vec::new();
    for (_queue_item, track_opt, video_opt) in rows {
      if let Some(track) = track_opt {
        result.push(MediaItem::Track(Arc::new(track)));
      } else if let Some(video) = video_opt {
        result.push(MediaItem::Video(Arc::new(video)));
      }
    }
    Ok(result)
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

pub fn get_distinct_albums() -> Vec<Arc<Track>> {
  use diesel::sql_query;
  use diesel::sql_types::*;

  #[derive(QueryableByName)]
  struct AlbumRow {
    #[diesel(sql_type = Text)]
    filename: String,
    #[diesel(sql_type = Nullable<Text>)]
    artist: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    album: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    album_artist: Option<String>,
  }

  with_db(|conn| {
    let rows: Vec<AlbumRow> = sql_query(
      "SELECT MIN(filename) as filename, MIN(artist) as artist, album, album_artist \
       FROM tracks GROUP BY album_artist, album ORDER BY album_artist, album"
    )
    .load(conn)
    .unwrap_or_default();

    Ok(rows.into_iter().map(|r| Arc::new(Track {
      filename: r.filename,
      title: None,
      artist: r.artist,
      album: r.album,
      album_artist: r.album_artist,
      track: None,
      genre: None,
      added: None,
      duration_seconds: None,
      play_count: 0,
      last_played: None,
    })).collect())
  })
  .unwrap_or_default()
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

pub fn load_tracks_by_album(artist_query: &str, album_query: &str) -> Vec<Arc<Track>> {
  use crate::schema::tracks::dsl;

  with_db(|conn| {
    let all: Vec<Track> = dsl::tracks
      .filter(dsl::album.eq(album_query))
      .order(dsl::track.asc())
      .load::<Track>(conn)
      .unwrap_or_default();

    let result: Vec<Arc<Track>> = all
      .into_iter()
      .filter(|t| {
        let t_artist = t.album_artist.as_deref().or(t.artist.as_deref()).unwrap_or("Unknown");
        t_artist == artist_query
      })
      .map(Arc::new)
      .collect();

    Ok(result)
  })
  .unwrap_or_default()
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

pub fn get_channel_name_map() -> std::collections::HashMap<i32, String> {
  with_db(|conn| {
    let channels: Vec<(i32, String)> = youtube_channels::table
      .select((youtube_channels::id, youtube_channels::name))
      .load(conn)
      .unwrap_or_default();
    let mut map = std::collections::HashMap::new();
    for (id, name) in channels {
      map.insert(id, name);
    }
    Ok(map)
  })
  .unwrap_or_default()
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
    let new_videos: Vec<NewYouTubeVideo> = videos
      .iter()
      .map(|(video_id, title, duration, thumbnail, published_at)| NewYouTubeVideo {
        video_id,
        channel_id: db_channel_id,
        title,
        duration_seconds: *duration,
        thumbnail_url: thumbnail.as_deref(),
        published_at: *published_at,
      })
      .collect();

    diesel::insert_or_ignore_into(youtube_videos::table)
      .values(&new_videos)
      .execute(conn)
      .map_err(|e| format!("Failed to insert videos: {e}"))?;

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
    let rows: Vec<(PlaylistTrack, Option<Track>, Option<YouTubeVideo>)> = playlist_tracks::table
      .filter(playlist_tracks::playlist_id.eq(playlist_id))
      .left_join(tracks::table)
      .left_join(youtube_videos::table)
      .select((PlaylistTrack::as_select(), Option::<Track>::as_select(), Option::<YouTubeVideo>::as_select()))
      .order(playlist_tracks::position.asc())
      .load(conn)
      .map_err(|e| format!("Failed to load playlist items: {e}"))?;

    let mut result = Vec::new();
    for (_playlist_item, track_opt, video_opt) in rows {
      if let Some(track) = track_opt {
        result.push(MediaItem::Track(Arc::new(track)));
      } else if let Some(video) = video_opt {
        result.push(MediaItem::Video(Arc::new(video)));
      }
    }
    Ok(result)
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

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;
  use std::sync::Arc;

  fn make_track(
    filename: &str,
    title: Option<&str>,
    artist: Option<&str>,
    album: Option<&str>,
    album_artist: Option<&str>,
  ) -> Arc<Track> {
    Arc::new(Track {
      filename: filename.to_string(),
      title: title.map(str::to_string),
      artist: artist.map(str::to_string),
      album: album.map(str::to_string),
      album_artist: album_artist.map(str::to_string),
      track: None,
      genre: None,
      added: None,
      duration_seconds: None,
      play_count: 0,
      last_played: None,
    })
  }

  #[test]
  fn is_audio_file_recognizes_common_formats() {
    let extensions = [
      "mp3", "flac", "ogg", "opus", "wav", "aac", "m4a", "wma", "aiff", "aif", "ape", "wv",
      "mpc", "mp4", "webm",
    ];
    for ext in extensions {
      assert!(
        is_audio_file(Path::new(&format!("song.{ext}"))),
        "{ext} should be recognized as audio"
      );
    }
  }

  #[test]
  fn is_audio_file_case_insensitive() {
    assert!(is_audio_file(Path::new("song.MP3")));
    assert!(is_audio_file(Path::new("song.Flac")));
    assert!(is_audio_file(Path::new("song.OGG")));
  }

  #[test]
  fn is_audio_file_rejects_non_audio() {
    assert!(!is_audio_file(Path::new("readme.txt")));
    assert!(!is_audio_file(Path::new("image.png")));
    assert!(!is_audio_file(Path::new("document.pdf")));
    assert!(!is_audio_file(Path::new("code.rs")));
    assert!(!is_audio_file(Path::new("data.json")));
  }

  #[test]
  fn is_audio_file_no_extension() {
    assert!(!is_audio_file(Path::new("Makefile")));
    assert!(!is_audio_file(Path::new("/some/path/noext")));
  }

  #[test]
  fn build_facets_empty_input() {
    let facets = build_facets(&[]);
    assert_eq!(facets.len(), 1);
    assert!(facets[0].all);
  }

  #[test]
  fn build_facets_first_entry_is_all() {
    let tracks = vec![make_track("/a.mp3", Some("Song"), Some("Artist"), Some("Album"), None)];
    let facets = build_facets(&tracks);
    assert!(facets[0].all);
    assert!(facets[0].album.is_none());
    assert!(facets[0].album_artist.is_none());
  }

  #[test]
  fn build_facets_deduplicates() {
    let tracks = vec![
      make_track("/a.mp3", Some("Song1"), Some("Artist"), Some("Album"), None),
      make_track("/b.mp3", Some("Song2"), Some("Artist"), Some("Album"), None),
    ];
    let facets = build_facets(&tracks);
    assert_eq!(facets.len(), 2);
  }

  #[test]
  fn build_facets_multiple_albums() {
    let tracks = vec![
      make_track("/a.mp3", None, Some("Artist"), Some("Album1"), None),
      make_track("/b.mp3", None, Some("Artist"), Some("Album2"), None),
    ];
    let facets = build_facets(&tracks);
    assert_eq!(facets.len(), 3);
  }

  #[test]
  fn build_facets_uses_album_artist_over_artist() {
    let tracks = vec![make_track(
      "/a.mp3",
      None,
      Some("Track Artist"),
      Some("Album"),
      Some("Album Artist"),
    )];
    let facets = build_facets(&tracks);
    let non_all = &facets[1];
    assert_eq!(
      non_all.album_artist_or_artist.as_deref(),
      Some("Album Artist")
    );
    assert_eq!(non_all.album_artist.as_deref(), Some("Album Artist"));
  }

  #[test]
  fn build_facets_falls_back_to_artist_when_no_album_artist() {
    let tracks = vec![make_track("/a.mp3", None, Some("Artist"), Some("Album"), None)];
    let facets = build_facets(&tracks);
    let non_all = &facets[1];
    assert_eq!(non_all.album_artist_or_artist.as_deref(), Some("Artist"));
    assert!(non_all.album_artist.is_none());
  }

  #[test]
  fn build_facets_sorted() {
    let tracks = vec![
      make_track("/a.mp3", None, Some("Zebra"), Some("Z Album"), None),
      make_track("/b.mp3", None, Some("Alpha"), Some("A Album"), None),
    ];
    let facets = build_facets(&tracks);
    assert!(facets[0].all);
    let albums: Vec<Option<&str>> = facets[1..].iter().map(|f| f.album.as_deref()).collect();
    assert_eq!(albums, vec![Some("A Album"), Some("Z Album")]);
  }

  #[test]
  fn build_facets_handles_none_fields() {
    let tracks = vec![make_track("/a.mp3", None, None, None, None)];
    let facets = build_facets(&tracks);
    assert_eq!(facets.len(), 2);
    let non_all = &facets[1];
    assert!(non_all.album.is_none());
    assert!(non_all.album_artist.is_none());
    assert!(non_all.album_artist_or_artist.is_none());
  }

  #[test]
  fn scan_progress_starting_folder() {
    let p = ScanProgress::StartingFolder {
      folder: "/music".to_string(),
    };
    if let ScanProgress::StartingFolder { folder } = p {
      assert_eq!(folder, "/music");
    } else {
      panic!("expected StartingFolder variant");
    }
  }

  #[test]
  fn scan_progress_found_file() {
    let p = ScanProgress::FoundFile {
      total_found: 5,
      skipped: 2,
      current_file: "song.mp3".to_string(),
    };
    if let ScanProgress::FoundFile {
      total_found,
      skipped,
      current_file,
    } = p
    {
      assert_eq!(total_found, 5);
      assert_eq!(skipped, 2);
      assert_eq!(current_file, "song.mp3");
    } else {
      panic!("expected FoundFile variant");
    }
  }

  #[test]
  fn scan_progress_complete() {
    let stale = vec!["/old/file.mp3".to_string()];
    let p = ScanProgress::Complete {
      total_found: 100,
      skipped: 10,
      added: 80,
      updated: 10,
      stale_files: stale.clone(),
    };
    if let ScanProgress::Complete {
      total_found,
      skipped,
      added,
      updated,
      stale_files,
    } = p
    {
      assert_eq!(total_found, 100);
      assert_eq!(skipped, 10);
      assert_eq!(added, 80);
      assert_eq!(updated, 10);
      assert_eq!(stale_files, stale);
    } else {
      panic!("expected Complete variant");
    }
  }

  #[test]
  fn scan_progress_clone() {
    let p = ScanProgress::ScannedFile {
      total_found: 1,
      skipped: 0,
      added: 1,
      updated: 0,
      current_file: "test.flac".to_string(),
    };
    let p2 = p.clone();
    if let ScanProgress::ScannedFile { current_file, .. } = p2 {
      assert_eq!(current_file, "test.flac");
    }
  }

  #[test]
  fn facet_derives() {
    let f = Facet {
      album: Some("A".to_string()),
      album_artist: None,
      album_artist_or_artist: Some("B".to_string()),
      all: false,
    };
    let f2 = f.clone();
    assert_eq!(f, f2);
    assert_eq!(format!("{:?}", f), format!("{:?}", f2));
  }
}
