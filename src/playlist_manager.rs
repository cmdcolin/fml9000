use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, setup_col};
use crate::playback_controller::PlaybackController;
use crate::settings::FmlSettings;
use crate::youtube_add_dialog;
use adw::prelude::*;
use fml9000::models::{Track, YouTubeVideo};
use fml9000::{get_videos_for_channel, get_youtube_channels, load_playlist_store, load_recently_played};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::{ColumnView, ColumnViewColumn, ScrolledWindow, SignalListItemFactory, SingleSelection};
use std::cell::{Ref, RefCell};
use std::rc::Rc;

#[derive(Clone, PartialEq)]
enum PlaylistType {
  RecentlyAdded,
  RecentlyPlayed,
  YouTubeChannel(i32, String),
}

struct Playlist {
  name: String,
  playlist_type: PlaylistType,
}

pub fn create_playlist_manager(
  playlist_mgr_store: &ListStore,
  main_playlist_store: ListStore,
  all_tracks: Rc<Vec<Rc<Track>>>,
  playback_controller: Rc<PlaybackController>,
  settings: Rc<RefCell<FmlSettings>>,
) -> gtk::Box {
  let selection = SingleSelection::builder().model(playlist_mgr_store).build();
  let columnview = ColumnView::builder().model(&selection).build();
  let factory = SignalListItemFactory::new();

  factory.connect_setup(move |_factory, item| setup_col(item));
  let settings_for_bind = settings.clone();
  factory.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let row_height = settings_for_bind.borrow().row_height;
    cell.set_row_height(row_height.height_pixels(), row_height.is_compact());
    let playlist: Ref<Playlist> = obj.borrow();
    cell.set_entry(&Entry {
      name: playlist.name.clone(),
    });
  });

  populate_playlist_store(playlist_mgr_store);

  let column = ColumnViewColumn::builder()
    .title("Playlists")
    .factory(&factory)
    .expand(true)
    .build();

  columnview.append_column(&column);

  let main_playlist_store_clone = main_playlist_store.clone();
  let all_tracks_clone = all_tracks.clone();
  selection.connect_selection_changed(move |sel, _, _| {
    if let Some(item) = sel.selected_item() {
      let obj = item.downcast::<BoxedAnyObject>().unwrap();
      let playlist: Ref<Playlist> = obj.borrow();

      main_playlist_store_clone.remove_all();

      match &playlist.playlist_type {
        PlaylistType::RecentlyAdded => {
          load_playlist_store(all_tracks_clone.iter(), &main_playlist_store_clone);
        }
        PlaylistType::RecentlyPlayed => {
          let recent = load_recently_played(100);
          load_playlist_store(recent.iter(), &main_playlist_store_clone);
        }
        PlaylistType::YouTubeChannel(id, _) => {
          if let Ok(videos) = get_videos_for_channel(*id) {
            load_youtube_videos(&videos, &main_playlist_store_clone);
          }
        }
      }
    }
  });

  let add_yt_btn = gtk::Button::builder()
    .icon_name("list-add-symbolic")
    .tooltip_text("Add YouTube Channel")
    .css_classes(["flat"])
    .build();

  let playlist_mgr_store_clone = playlist_mgr_store.clone();
  let playback_controller_clone = playback_controller.clone();
  add_yt_btn.connect_clicked(move |_| {
    let store = playlist_mgr_store_clone.clone();
    youtube_add_dialog::show_dialog(playback_controller_clone.clone(), move || {
      store.remove_all();
      populate_playlist_store(&store);
    });
  });

  let header_box = gtk::Box::builder()
    .orientation(gtk::Orientation::Horizontal)
    .build();
  header_box.append(&gtk::Label::builder().label("Playlists").hexpand(true).xalign(0.0).build());
  header_box.append(&add_yt_btn);

  let scrolled = ScrolledWindow::builder()
    .child(&columnview)
    .vexpand(true)
    .build();

  let container = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .spacing(4)
    .build();
  container.append(&header_box);
  container.append(&scrolled);

  container
}

fn populate_playlist_store(store: &ListStore) {
  store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently added".to_string(),
    playlist_type: PlaylistType::RecentlyAdded,
  }));
  store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently played".to_string(),
    playlist_type: PlaylistType::RecentlyPlayed,
  }));

  if let Ok(channels) = get_youtube_channels() {
    for channel in channels {
      store.append(&BoxedAnyObject::new(Playlist {
        name: format!("YT: {}", channel.name),
        playlist_type: PlaylistType::YouTubeChannel(channel.id, channel.name.clone()),
      }));
    }
  }
}

fn load_youtube_videos(videos: &[Rc<YouTubeVideo>], store: &ListStore) {
  for video in videos {
    store.append(&BoxedAnyObject::new(video.clone()));
  }
}
