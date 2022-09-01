// @generated automatically by Diesel CLI.

diesel::table! {
    tracks (id) {
        id -> Nullable<Integer>,
        filename -> Nullable<Text>,
        title -> Nullable<Text>,
        artist -> Nullable<Text>,
        album -> Nullable<Text>,
        album_artist -> Nullable<Text>,
    }
}
