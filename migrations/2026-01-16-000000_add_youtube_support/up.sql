-- Add YouTube channel support

CREATE TABLE youtube_channels (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  channel_id VARCHAR NOT NULL UNIQUE,
  name VARCHAR NOT NULL,
  handle VARCHAR,
  url VARCHAR NOT NULL,
  thumbnail_url VARCHAR,
  last_fetched DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE TABLE youtube_videos (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  video_id VARCHAR NOT NULL UNIQUE,
  channel_id INTEGER NOT NULL REFERENCES youtube_channels(id) ON DELETE CASCADE,
  title VARCHAR NOT NULL,
  duration_seconds INTEGER,
  thumbnail_url VARCHAR,
  published_at DATETIME,
  fetched_at DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE INDEX idx_youtube_videos_channel ON youtube_videos(channel_id);
