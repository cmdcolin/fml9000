use axum::extract::Query;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;

#[derive(serde_derive::Deserialize)]
pub struct ThumbnailQuery {
    key: String,
}

pub async fn get_thumbnail(
    Query(query): Query<ThumbnailQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let key = query.key;

    let cached_path = tokio::task::spawn_blocking(move || {
        fml9000_core::thumbnail_cache::get_cached_path(&key)
            .or_else(|| fml9000_core::thumbnail_cache::extract_and_cache_album_art(&key))
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let data = tokio::fs::read(&cached_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let content_type = if cached_path.extension().is_some_and(|e| e == "png") {
        "image/png"
    } else {
        "image/jpeg"
    };

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        data,
    ))
}
