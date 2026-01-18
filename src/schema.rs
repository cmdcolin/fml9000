// @generated automatically by Diesel CLI.

diesel::table! {
    playback_queue (id) {
        id -> Integer,
        position -> Integer,
        track_filename -> Nullable<Text>,
        youtube_video_id -> Nullable<Integer>,
        added_at -> Timestamp,
    }
}

diesel::table! {
    playlist_tracks (id) {
        id -> Integer,
        playlist_id -> Integer,
        track_filename -> Nullable<Text>,
        youtube_video_id -> Nullable<Integer>,
        position -> Integer,
        added_at -> Timestamp,
    }
}

diesel::table! {
    playlists (id) {
        id -> Integer,
        name -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    recently_played (filename) {
        filename -> Text,
        timestamp -> Nullable<Timestamp>,
    }
}

diesel::table! {
    tracks (filename) {
        filename -> Text,
        title -> Nullable<Text>,
        artist -> Nullable<Text>,
        track -> Nullable<Text>,
        album -> Nullable<Text>,
        genre -> Nullable<Text>,
        album_artist -> Nullable<Text>,
        added -> Nullable<Timestamp>,
        duration_seconds -> Nullable<Integer>,
        play_count -> Integer,
        last_played -> Nullable<Timestamp>,
    }
}

diesel::table! {
    youtube_channels (id) {
        id -> Integer,
        channel_id -> Text,
        name -> Text,
        handle -> Nullable<Text>,
        url -> Text,
        thumbnail_url -> Nullable<Text>,
        last_fetched -> Nullable<Timestamp>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    youtube_videos (id) {
        id -> Integer,
        video_id -> Text,
        channel_id -> Integer,
        title -> Text,
        duration_seconds -> Nullable<Integer>,
        thumbnail_url -> Nullable<Text>,
        published_at -> Nullable<Timestamp>,
        fetched_at -> Timestamp,
        play_count -> Integer,
        last_played -> Nullable<Timestamp>,
    }
}

diesel::joinable!(playback_queue -> tracks (track_filename));
diesel::joinable!(playback_queue -> youtube_videos (youtube_video_id));
diesel::joinable!(playlist_tracks -> playlists (playlist_id));
diesel::joinable!(playlist_tracks -> tracks (track_filename));
diesel::joinable!(playlist_tracks -> youtube_videos (youtube_video_id));
diesel::joinable!(youtube_videos -> youtube_channels (channel_id));

diesel::allow_tables_to_appear_in_same_query!(
  playback_queue,
  playlist_tracks,
  playlists,
  recently_played,
  tracks,
  youtube_channels,
  youtube_videos,
);
