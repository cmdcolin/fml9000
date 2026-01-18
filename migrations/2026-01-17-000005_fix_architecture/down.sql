-- Recreate recently_played table
CREATE TABLE recently_played (
    filename TEXT PRIMARY KEY NOT NULL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Remove added column from youtube_videos
-- SQLite doesn't support DROP COLUMN directly in older versions
-- We'll recreate the table without it
CREATE TABLE youtube_videos_new (
    id INTEGER PRIMARY KEY NOT NULL,
    video_id TEXT NOT NULL UNIQUE,
    channel_id INTEGER NOT NULL REFERENCES youtube_channels(id),
    title TEXT NOT NULL,
    duration_seconds INTEGER,
    thumbnail_url TEXT,
    published_at TIMESTAMP,
    fetched_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    play_count INTEGER NOT NULL DEFAULT 0,
    last_played TIMESTAMP
);

INSERT INTO youtube_videos_new
SELECT id, video_id, channel_id, title, duration_seconds, thumbnail_url,
       published_at, fetched_at, play_count, last_played
FROM youtube_videos;

DROP TABLE youtube_videos;
ALTER TABLE youtube_videos_new RENAME TO youtube_videos;

-- Recreate playback_queue without constraints
DROP TABLE IF EXISTS playback_queue;

CREATE TABLE playback_queue (
    id INTEGER PRIMARY KEY NOT NULL,
    position INTEGER NOT NULL,
    track_filename TEXT,
    youtube_video_id INTEGER,
    added_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (track_filename) REFERENCES tracks(filename),
    FOREIGN KEY (youtube_video_id) REFERENCES youtube_videos(id)
);

CREATE INDEX idx_playback_queue_position ON playback_queue(position);
