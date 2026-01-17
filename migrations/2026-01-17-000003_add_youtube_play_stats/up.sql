ALTER TABLE youtube_videos ADD COLUMN play_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE youtube_videos ADD COLUMN last_played TIMESTAMP;
