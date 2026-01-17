CREATE TABLE playlists (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  name VARCHAR NOT NULL,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE TABLE playlist_tracks (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
  track_filename VARCHAR REFERENCES tracks(filename) ON DELETE CASCADE,
  youtube_video_id INTEGER REFERENCES youtube_videos(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  added_at DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL,
  CHECK (
    (track_filename IS NOT NULL AND youtube_video_id IS NULL) OR
    (track_filename IS NULL AND youtube_video_id IS NOT NULL)
  )
);

CREATE INDEX idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);
