// @generated automatically by Diesel CLI.

diesel::table! {
    recently_played (filename) {
        filename -> Text,
        timestamp -> Nullable<Timestamp>,
    }
}

diesel::table! {
    tracks (filename) {
        filename -> Text,
        artist -> Nullable<Text>,
        title -> Nullable<Text>,
        album -> Nullable<Text>,
        genre -> Nullable<Text>,
        album_artist -> Nullable<Text>,
        track -> Nullable<Text>,
        added -> Nullable<Timestamp>,
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
    }
}

diesel::joinable!(youtube_videos -> youtube_channels (channel_id));

diesel::allow_tables_to_appear_in_same_query!(
    recently_played,
    tracks,
    youtube_channels,
    youtube_videos,
);
