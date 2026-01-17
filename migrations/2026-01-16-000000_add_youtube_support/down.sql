-- Undo YouTube support

DROP INDEX IF EXISTS idx_youtube_videos_channel;
DROP TABLE IF EXISTS youtube_videos;
DROP TABLE IF EXISTS youtube_channels;
