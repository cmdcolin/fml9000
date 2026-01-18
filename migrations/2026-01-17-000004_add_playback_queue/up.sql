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
