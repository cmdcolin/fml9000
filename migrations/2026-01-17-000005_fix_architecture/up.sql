-- Drop the redundant recently_played table
-- We'll use last_played field on tracks/videos tables directly
DROP TABLE IF EXISTS recently_played;

-- Add 'added' column to youtube_videos for consistency with tracks
-- Default to fetched_at for existing rows
ALTER TABLE youtube_videos ADD COLUMN added TIMESTAMP;
UPDATE youtube_videos SET added = fetched_at WHERE added IS NULL;

-- Recreate playback_queue with proper constraints
-- SQLite doesn't support ALTER TABLE for constraints, so we recreate
DROP TABLE IF EXISTS playback_queue;

CREATE TABLE playback_queue (
    id INTEGER PRIMARY KEY NOT NULL,
    position INTEGER NOT NULL,
    track_filename TEXT REFERENCES tracks(filename) ON DELETE CASCADE,
    youtube_video_id INTEGER REFERENCES youtube_videos(id) ON DELETE CASCADE,
    added_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- Ensure exactly one of track_filename or youtube_video_id is set
    CHECK (
        (track_filename IS NOT NULL AND youtube_video_id IS NULL) OR
        (track_filename IS NULL AND youtube_video_id IS NOT NULL)
    )
);

CREATE INDEX idx_playback_queue_position ON playback_queue(position);
