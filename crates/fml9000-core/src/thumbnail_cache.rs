use crate::settings::get_project_dirs;
use lofty::file::TaggedFileExt;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

fn cache_dir() -> Option<PathBuf> {
  get_project_dirs().map(|dirs| dirs.config_dir().join("thumbnails"))
}

fn hash_key(key: &str) -> String {
  let mut hasher = DefaultHasher::new();
  key.hash(&mut hasher);
  format!("{:016x}", hasher.finish())
}

pub fn get_cached_path(key: &str) -> Option<PathBuf> {
  let dir = cache_dir()?;
  let path = dir.join(hash_key(key));
  if path.exists() {
    Some(path)
  } else {
    None
  }
}

pub fn save_to_cache(key: &str, data: &[u8]) -> Option<PathBuf> {
  let dir = cache_dir()?;
  std::fs::create_dir_all(&dir).ok()?;
  let path = dir.join(hash_key(key));
  std::fs::write(&path, data).ok()?;
  Some(path)
}

pub fn fetch_and_cache_url(url: &str) -> Option<PathBuf> {
  if let Some(cached) = get_cached_path(url) {
    return Some(cached);
  }
  let response = ureq::get(url).call().ok()?;
  let data = response.into_body().read_to_vec().ok()?;
  save_to_cache(url, &data)
}

pub fn download_all_video_thumbnails(
  on_progress: impl Fn(usize, usize),
) -> (usize, usize) {
  let videos = crate::db::get_all_videos().unwrap_or_default();
  let total = videos.len();
  let mut downloaded = 0;
  for (i, video) in videos.iter().enumerate() {
    let url = video.thumbnail_url.clone().unwrap_or_else(|| {
      format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", video.video_id)
    });
    if get_cached_path(&url).is_none() {
      if fetch_and_cache_url(&url).is_some() {
        downloaded += 1;
      }
    }
    on_progress(i + 1, total);
  }
  (downloaded, total)
}

pub fn download_all_album_art(
  on_progress: impl Fn(usize, usize),
) -> (usize, usize) {
  let albums = crate::db::get_distinct_albums();
  let total = albums.len();
  let mut extracted = 0;
  for (i, track) in albums.iter().enumerate() {
    if get_cached_path(&track.filename).is_none() {
      if extract_and_cache_album_art(&track.filename).is_some() {
        extracted += 1;
      }
    }
    on_progress(i + 1, total);
  }
  (extracted, total)
}

pub fn extract_and_cache_album_art(track_filename: &str) -> Option<PathBuf> {
  if let Some(cached) = get_cached_path(track_filename) {
    return Some(cached);
  }

  let probe = lofty::probe::Probe::open(track_filename).ok()?;
  let tagged_file = probe.read().ok()?;
  let tag = tagged_file
    .primary_tag()
    .or_else(|| tagged_file.first_tag());
  if let Some(t) = tag {
    let pictures = t.pictures();
    if let Some(picture) = pictures.first() {
      return save_to_cache(track_filename, picture.data());
    }
  }

  let mut dir = PathBuf::from(track_filename);
  dir.pop();
  for name in &[
    "cover.jpg",
    "cover.png",
    "folder.jpg",
    "folder.png",
    "album.jpg",
    "album.png",
    "front.jpg",
    "front.png",
  ] {
    let path = dir.join(name);
    if path.exists() {
      if let Ok(data) = std::fs::read(&path) {
        return save_to_cache(track_filename, &data);
      }
    }
  }

  None
}
