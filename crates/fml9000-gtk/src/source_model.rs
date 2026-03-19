use fml9000_core::{
  get_all_media, get_all_videos, get_distinct_albums, get_playlist_items, get_queue_items,
  get_user_playlists, get_videos_for_channel, get_youtube_channels, load_recently_added_items,
  load_recently_played_items, load_tracks, MediaItem,
};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use std::cell::Ref;

#[derive(Clone, PartialEq)]
pub enum SourceKind {
  AllMedia,
  AllTracks,
  AllVideos,
  RecentlyAdded,
  RecentlyPlayed,
  PlaybackQueue,
  UserPlaylist(i32, String),
  YouTubeChannel(i32, String),
}

impl SourceKind {
  pub fn label(&self) -> String {
    match self {
      SourceKind::AllMedia => "All Media".to_string(),
      SourceKind::AllTracks => "All Tracks".to_string(),
      SourceKind::AllVideos => "All Videos".to_string(),
      SourceKind::RecentlyAdded => "Recently Added".to_string(),
      SourceKind::RecentlyPlayed => "Recently Played".to_string(),
      SourceKind::PlaybackQueue => "Playback Queue".to_string(),
      SourceKind::UserPlaylist(_, name) => name.clone(),
      SourceKind::YouTubeChannel(_, name) => name.clone(),
    }
  }

  pub fn playlist_id(&self) -> Option<i32> {
    match self {
      SourceKind::UserPlaylist(id, _) => Some(*id),
      _ => None,
    }
  }

  pub fn load_items(&self) -> Vec<MediaItem> {
    match self {
      SourceKind::AllMedia => get_all_media(),
      SourceKind::AllTracks => load_tracks()
        .unwrap_or_default()
        .into_iter()
        .map(MediaItem::Track)
        .collect(),
      SourceKind::AllVideos => get_all_videos()
        .unwrap_or_default()
        .into_iter()
        .map(MediaItem::Video)
        .collect(),
      SourceKind::RecentlyAdded => load_recently_added_items(0),
      SourceKind::RecentlyPlayed => load_recently_played_items(100),
      SourceKind::PlaybackQueue => get_queue_items(),
      SourceKind::UserPlaylist(id, _) => get_playlist_items(*id).unwrap_or_default(),
      SourceKind::YouTubeChannel(id, _) => get_videos_for_channel(*id)
        .unwrap_or_default()
        .into_iter()
        .map(MediaItem::Video)
        .collect(),
    }
  }
}

#[derive(Clone)]
pub enum SectionId {
  AutoPlaylists,
  UserPlaylists,
  YouTubeChannels,
}

pub enum TreeEntry {
  SectionHeader(String, SectionId),
  Source(SourceKind),
}

pub fn build_section_children(section: &SectionId) -> ListStore {
  let store = ListStore::new::<BoxedAnyObject>();
  match section {
    SectionId::AutoPlaylists => {
      for kind in auto_playlist_sources() {
        store.append(&BoxedAnyObject::new(TreeEntry::Source(kind)));
      }
    }
    SectionId::UserPlaylists => {
      if let Ok(playlists) = get_user_playlists() {
        for pl in playlists {
          store.append(&BoxedAnyObject::new(TreeEntry::Source(
            SourceKind::UserPlaylist(pl.id, pl.name.clone()),
          )));
        }
      }
    }
    SectionId::YouTubeChannels => {
      if let Ok(channels) = get_youtube_channels() {
        for ch in channels {
          store.append(&BoxedAnyObject::new(TreeEntry::Source(
            SourceKind::YouTubeChannel(ch.id, ch.name.clone()),
          )));
        }
      }
    }
  }
  store
}

pub fn auto_playlist_sources() -> Vec<SourceKind> {
  vec![
    SourceKind::AllMedia,
    SourceKind::AllTracks,
    SourceKind::AllVideos,
    SourceKind::RecentlyAdded,
    SourceKind::RecentlyPlayed,
    SourceKind::PlaybackQueue,
  ]
}

pub fn get_distinct_album_items() -> Vec<(String, String, String)> {
  get_distinct_albums()
    .into_iter()
    .map(|track| {
      let artist = track
        .album_artist
        .clone()
        .unwrap_or_else(|| {
          track.artist.clone().unwrap_or_else(|| "Unknown".to_string())
        });
      let album = track
        .album
        .clone()
        .unwrap_or_else(|| "Unknown".to_string());
      (artist, album, track.filename.clone())
    })
    .collect()
}

pub fn populate_section_headers(store: &ListStore) {
  store.append(&BoxedAnyObject::new(TreeEntry::SectionHeader(
    "Auto Playlists".to_string(),
    SectionId::AutoPlaylists,
  )));
  store.append(&BoxedAnyObject::new(TreeEntry::SectionHeader(
    "Playlists".to_string(),
    SectionId::UserPlaylists,
  )));
  store.append(&BoxedAnyObject::new(TreeEntry::SectionHeader(
    "YouTube".to_string(),
    SectionId::YouTubeChannels,
  )));
}

pub fn try_get_source_from_row(row: &gtk::TreeListRow) -> Option<SourceKind> {
  let obj = row.item()?.downcast::<BoxedAnyObject>().ok()?;
  let entry: Ref<TreeEntry> = obj.borrow();
  match &*entry {
    TreeEntry::Source(kind) => Some(kind.clone()),
    TreeEntry::SectionHeader(_, _) => None,
  }
}

pub fn load_media_items_to_store(items: &[MediaItem], store: &ListStore) {
  for item in items {
    store.append(&BoxedAnyObject::new(item.clone()));
  }
}
