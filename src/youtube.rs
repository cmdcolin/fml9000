use chrono::NaiveDateTime;
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct ChannelInfo {
  pub channel_id: String,
  pub name: String,
  pub handle: Option<String>,
  pub url: String,
  pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VideoInfo {
  pub video_id: String,
  pub title: String,
  pub duration_seconds: Option<i32>,
  pub thumbnail_url: Option<String>,
  pub published_at: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
struct YtDlpPlaylistEntry {
  id: Option<String>,
  title: Option<String>,
  duration: Option<f64>,
  thumbnail: Option<String>,
  upload_date: Option<String>,
  channel: Option<String>,
  channel_id: Option<String>,
  channel_url: Option<String>,
  uploader: Option<String>,
  uploader_id: Option<String>,
  uploader_url: Option<String>,
  playlist_channel: Option<String>,
  playlist_channel_id: Option<String>,
  playlist_uploader: Option<String>,
  playlist_uploader_id: Option<String>,
  playlist_webpage_url: Option<String>,
  #[serde(rename = "_type")]
  entry_type: Option<String>,
}

pub fn parse_youtube_url(url: &str) -> Option<String> {
  if url.contains("youtube.com") || url.contains("youtu.be") {
    Some(url.to_string())
  } else if url.starts_with('@') {
    Some(format!("https://www.youtube.com/{}/videos", url))
  } else if url.starts_with("UC") && url.len() > 20 {
    Some(format!("https://www.youtube.com/channel/{}/videos", url))
  } else {
    None
  }
}

pub fn fetch_channel_info(
  url: &str,
  on_progress: impl Fn(&str),
) -> Result<(ChannelInfo, Vec<VideoInfo>), String> {
  let parsed_url = parse_youtube_url(url).ok_or("Invalid YouTube URL")?;
  on_progress("Starting yt-dlp...");

  let mut child = Command::new("yt-dlp")
    .args([
      "--dump-json",
      "--flat-playlist",
      "--playlist-end",
      "100",
      "--no-warnings",
      &parsed_url,
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| format!("Failed to run yt-dlp: {e}"))?;

  let stdout = child.stdout.take().ok_or("Failed to get stdout")?;
  let reader = BufReader::new(stdout);

  let mut videos = Vec::new();
  let mut channel_info: Option<ChannelInfo> = None;

  for (idx, line) in reader.lines().enumerate() {
    let line = line.map_err(|e| format!("Failed to read line: {e}"))?;
    if line.trim().is_empty() {
      continue;
    }

    let entry: YtDlpPlaylistEntry = serde_json::from_str(&line)
      .map_err(|e| format!("Failed to parse JSON: {e}"))?;

    if channel_info.is_none() {
      let ch_id = entry
        .channel_id
        .as_ref()
        .or(entry.uploader_id.as_ref())
        .or(entry.playlist_channel_id.as_ref());

      if let Some(ch_id) = ch_id {
        let name = entry
          .channel
          .clone()
          .or(entry.uploader.clone())
          .or(entry.playlist_channel.clone())
          .or(entry.playlist_uploader.clone())
          .unwrap_or_else(|| "Unknown Channel".to_string());
        let ch_url = entry
          .channel_url
          .clone()
          .or(entry.uploader_url.clone())
          .or(entry.playlist_webpage_url.clone())
          .unwrap_or_else(|| parsed_url.clone());
        let handle = entry
          .playlist_uploader_id
          .clone()
          .filter(|h| h.starts_with('@'))
          .or_else(|| extract_handle(&ch_url));

        channel_info = Some(ChannelInfo {
          channel_id: ch_id.clone(),
          name,
          handle,
          url: ch_url,
          thumbnail_url: None,
        });
      }
    }

    if let Some(video_id) = entry.id {
      if entry.entry_type.as_deref() != Some("playlist") {
        let published_at = entry.upload_date.as_ref().and_then(|d| {
          NaiveDateTime::parse_from_str(&format!("{d} 00:00:00"), "%Y%m%d %H:%M:%S").ok()
        });

        videos.push(VideoInfo {
          video_id,
          title: entry.title.unwrap_or_else(|| "Unknown".to_string()),
          duration_seconds: entry.duration.map(|d| d as i32),
          thumbnail_url: entry.thumbnail,
          published_at,
        });

        on_progress(&format!("Fetched {} videos...", idx + 1));
      }
    }
  }

  let status = child.wait().map_err(|e| format!("Failed to wait for yt-dlp: {e}"))?;
  if !status.success() {
    return Err("yt-dlp failed".to_string());
  }

  let channel = channel_info.ok_or("Could not determine channel info")?;
  Ok((channel, videos))
}

fn extract_handle(url: &str) -> Option<String> {
  if url.contains("/@") {
    let start = url.find("/@")?;
    let rest = &url[start + 1..];
    let end = rest.find('/').unwrap_or(rest.len());
    Some(rest[..end].to_string())
  } else {
    None
  }
}

pub fn get_video_url(video_id: &str) -> String {
  format!("https://www.youtube.com/watch?v={video_id}")
}
