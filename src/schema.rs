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
        added -> Nullable<Timestamp>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(recently_played, tracks,);
