use chrono::NaiveDateTime;
use serde::Deserialize;
use std::collections::HashSet;

const GET_HANDLE_ENDPOINT: &str =
  "https://gbt7w5u4c1.execute-api.us-east-1.amazonaws.com/default/youtubeGetPlaylistFromHandle";
const GET_CONTENTS_ENDPOINT: &str =
  "https://m0v7dr1zz2.execute-api.us-east-1.amazonaws.com/default/youtubeGetPlaylistContents";

#[derive(Debug, Deserialize)]
struct PlaylistIdResponse {
  #[serde(rename = "playlistId")]
  playlist_id: String,
}

#[derive(Debug, Deserialize)]
struct ResourceId {
  #[serde(rename = "videoId")]
  video_id: String,
}

#[derive(Debug, Deserialize)]
struct Thumbnail {
  url: String,
}

#[derive(Debug, Deserialize)]
struct Thumbnails {
  medium: Option<Thumbnail>,
}

#[derive(Debug, Deserialize)]
struct Snippet {
  #[serde(rename = "resourceId")]
  resource_id: ResourceId,
  title: String,
  #[serde(rename = "publishedAt")]
  published_at: Option<String>,
  thumbnails: Option<Thumbnails>,
}

#[derive(Debug, Deserialize)]
struct PlaylistItem {
  snippet: Snippet,
}

#[derive(Debug, Deserialize)]
struct PlaylistContentsResponse {
  items: Vec<PlaylistItem>,
  #[serde(rename = "nextPageToken")]
  next_page_token: Option<String>,
  #[serde(rename = "totalResults")]
  total_results: i32,
}

#[derive(Debug, Clone)]
pub struct ApiVideoInfo {
  pub video_id: String,
  pub title: String,
  pub published_at: Option<NaiveDateTime>,
  pub thumbnail_url: Option<String>,
}

pub fn get_playlist_id_for_handle(handle: &str) -> Result<String, String> {
  let clean_handle = handle.trim_start_matches('@');
  let url = format!("{}?handle={}", GET_HANDLE_ENDPOINT, clean_handle);

  let response = ureq::get(&url)
    .call()
    .map_err(|e| format!("Failed to fetch playlist ID: {e}"))?;

  let result: PlaylistIdResponse = response
    .into_body()
    .read_json()
    .map_err(|e| format!("Failed to parse playlist ID response: {e}"))?;

  Ok(result.playlist_id)
}

pub fn fetch_new_videos(
  playlist_id: &str,
  existing_video_ids: &HashSet<String>,
  on_progress: impl Fn(usize, i32),
) -> Result<Vec<ApiVideoInfo>, String> {
  let mut all_videos = Vec::new();
  let mut next_page_token: Option<String> = None;
  let mut found_existing = false;

  loop {
    let url = match &next_page_token {
      Some(token) => format!(
        "{}?playlistId={}&nextPageToken={}",
        GET_CONTENTS_ENDPOINT, playlist_id, token
      ),
      None => format!("{}?playlistId={}", GET_CONTENTS_ENDPOINT, playlist_id),
    };

    let response = ureq::get(&url)
      .call()
      .map_err(|e| format!("Failed to fetch playlist contents: {e}"))?;

    let result: PlaylistContentsResponse = response
      .into_body()
      .read_json()
      .map_err(|e| format!("Failed to parse playlist contents: {e}"))?;

    for item in result.items {
      let video_id = item.snippet.resource_id.video_id;

      if existing_video_ids.contains(&video_id) {
        found_existing = true;
        break;
      }

      let published_at = item.snippet.published_at.as_ref().and_then(|s| {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")
          .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ"))
          .ok()
      });

      let thumbnail_url = item
        .snippet
        .thumbnails
        .and_then(|t| t.medium)
        .map(|t| t.url);

      all_videos.push(ApiVideoInfo {
        video_id,
        title: item.snippet.title,
        published_at,
        thumbnail_url,
      });
    }

    on_progress(all_videos.len(), result.total_results);

    if found_existing {
      break;
    }

    match result.next_page_token {
      Some(token) if !token.is_empty() => {
        next_page_token = Some(token);
        std::thread::sleep(std::time::Duration::from_millis(500));
      }
      _ => break,
    }
  }

  Ok(all_videos)
}
