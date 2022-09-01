// @generated automatically by Diesel CLI.

diesel::table! {
    tracks (id) {
        id -> Integer,
        filename -> Text,
        published -> Bool,
    }
}
