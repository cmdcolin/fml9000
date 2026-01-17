use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, setup_col};
use crate::new_playlist_dialog;
use crate::playback_controller::PlaybackController;
use crate::settings::FmlSettings;
use crate::youtube_add_dialog;
use adw::prelude::*;
use fml9000::models::{Track, YouTubeVideo};
use fml9000::{
  add_track_to_playlist, add_video_to_playlist, delete_playlist, get_playlist_items,
  get_user_playlists, get_videos_for_channel, get_youtube_channels, load_playlist_store,
  load_recently_played, rename_playlist, UserPlaylistItem,
};
use gtk::gdk;
use gtk::gio::ListStore;
use gtk::glib;
use gtk::glib::BoxedAnyObject;
use gtk::{ColumnView, ColumnViewColumn, DropTarget, GestureClick, PopoverMenu, ScrolledWindow, SignalListItemFactory, SingleSelection};
use std::cell::{Ref, RefCell};
use std::rc::Rc;

#[derive(Clone, PartialEq)]
enum PlaylistType {
  RecentlyAdded,
  RecentlyPlayed,
  YouTubeChannel(i32, String),
  UserPlaylist(i32, String),
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
  current_playlist_id: Rc<RefCell<Option<i32>>>,
) -> gtk::Box {
  let selection = SingleSelection::builder().model(playlist_mgr_store).build();
  let columnview = ColumnView::builder().model(&selection).build();
  let factory = SignalListItemFactory::new();

  let playback_controller_for_setup = playback_controller.clone();
  let playlist_mgr_store_for_setup = playlist_mgr_store.clone();
  factory.connect_setup(move |_factory, item| {
    setup_col(item);

    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    if let Some(child) = list_item.child() {
      let drop_target = DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY);

      let current_playlist_id: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));

      let pc = playback_controller_for_setup.clone();
      let store = playlist_mgr_store_for_setup.clone();
      let pid = current_playlist_id.clone();
      let pid_for_enter = current_playlist_id.clone();
      let child_for_enter = child.clone();
      drop_target.connect_enter(move |_target, _x, _y| {
        if pid_for_enter.borrow().is_some() {
          child_for_enter.add_css_class("drop-target-hover");
        }
        gdk::DragAction::COPY
      });

      let child_for_leave = child.clone();
      drop_target.connect_leave(move |_target| {
        child_for_leave.remove_css_class("drop-target-hover");
      });

      let child_for_drop = child.clone();
      drop_target.connect_drop(move |_target, value, _x, _y| {
        child_for_drop.remove_css_class("drop-target-hover");

        let Ok(data) = value.get::<String>() else {
          return false;
        };

        if let Some(playlist_id) = *pid.borrow() {
          return handle_drop_on_playlist(playlist_id, &data);
        }

        let store_clone = store.clone();
        let data_clone = data.clone();
        new_playlist_dialog::show_dialog(
          pc.clone(),
          data.clone(),
          move |playlist_id| {
            let _ = handle_drop_on_playlist(playlist_id, &data_clone);
            store_clone.remove_all();
            populate_playlist_store(&store_clone);
          },
        );
        true
      });

      child.add_controller(drop_target);

      if let Some(cell) = child.downcast_ref::<crate::grid_cell::GridCell>() {
        cell.set_playlist_id(current_playlist_id);
      }
    }
  });

  let settings_for_bind = settings.clone();
  factory.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let row_height = settings_for_bind.borrow().row_height;
    cell.set_row_height(row_height.height_pixels(), row_height.is_compact());
    let playlist: Ref<Playlist> = obj.borrow();

    if let PlaylistType::UserPlaylist(id, _) = &playlist.playlist_type {
      cell.set_entry(&Entry {
        name: format!("♪ {}", playlist.name),
      });
      cell.add_css_class("user-playlist");
      cell.set_playlist_id_value(Some(*id));
    } else {
      cell.set_entry(&Entry {
        name: playlist.name.clone(),
      });
      cell.remove_css_class("user-playlist");
      cell.set_playlist_id_value(None);
    }
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
  let current_playlist_id_clone = current_playlist_id.clone();
  selection.connect_selection_changed(move |sel, _, _| {
    if let Some(item) = sel.selected_item() {
      let obj = item.downcast::<BoxedAnyObject>().unwrap();
      let playlist: Ref<Playlist> = obj.borrow();

      main_playlist_store_clone.remove_all();

      match &playlist.playlist_type {
        PlaylistType::RecentlyAdded => {
          *current_playlist_id_clone.borrow_mut() = None;
          load_playlist_store(all_tracks_clone.iter(), &main_playlist_store_clone);
        }
        PlaylistType::RecentlyPlayed => {
          *current_playlist_id_clone.borrow_mut() = None;
          let recent = load_recently_played(100);
          load_playlist_store(recent.iter(), &main_playlist_store_clone);
        }
        PlaylistType::YouTubeChannel(id, _) => {
          *current_playlist_id_clone.borrow_mut() = None;
          if let Ok(videos) = get_videos_for_channel(*id) {
            load_youtube_videos(&videos, &main_playlist_store_clone);
          }
        }
        PlaylistType::UserPlaylist(id, _) => {
          *current_playlist_id_clone.borrow_mut() = Some(*id);
          if let Ok(items) = get_playlist_items(*id) {
            load_user_playlist_items(&items, &main_playlist_store_clone);
          }
        }
      }
    }
  });

  let playlist_menu = gtk::gio::Menu::new();
  playlist_menu.append(Some("Rename"), Some("playlist-mgr.rename"));
  playlist_menu.append(Some("Delete"), Some("playlist-mgr.delete"));

  let playlist_popover = PopoverMenu::from_model(Some(&playlist_menu));
  playlist_popover.set_parent(&columnview);
  playlist_popover.set_has_arrow(false);

  let current_playlist: Rc<RefCell<Option<(i32, String)>>> = Rc::new(RefCell::new(None));

  let action_group = gtk::gio::SimpleActionGroup::new();

  let cp = current_playlist.clone();
  let store_for_rename = playlist_mgr_store.clone();
  let pc_for_rename = playback_controller.clone();
  let rename_action = gtk::gio::SimpleAction::new("rename", None);
  rename_action.connect_activate(move |_, _| {
    if let Some((id, name)) = cp.borrow().clone() {
      show_rename_dialog(pc_for_rename.clone(), id, &name, {
        let store = store_for_rename.clone();
        move || {
          store.remove_all();
          populate_playlist_store(&store);
        }
      });
    }
  });
  action_group.add_action(&rename_action);

  let cp = current_playlist.clone();
  let store_for_delete = playlist_mgr_store.clone();
  let delete_action = gtk::gio::SimpleAction::new("delete", None);
  delete_action.connect_activate(move |_, _| {
    if let Some((id, _)) = cp.borrow().clone() {
      if delete_playlist(id).is_ok() {
        store_for_delete.remove_all();
        populate_playlist_store(&store_for_delete);
      }
    }
  });
  action_group.add_action(&delete_action);

  columnview.insert_action_group("playlist-mgr", Some(&action_group));

  let gesture = GestureClick::builder().button(3).build();
  let cp = current_playlist.clone();
  let popover = playlist_popover.clone();
  let sel_for_gesture = selection.clone();
  let store_for_gesture = playlist_mgr_store.clone();
  gesture.connect_released(move |gesture, _n_press, x, y| {
    let mut found_playlist: Option<(i32, String)> = None;

    if let Some(selected_item) = sel_for_gesture.selected_item() {
      let obj = selected_item.downcast::<BoxedAnyObject>().unwrap();
      let playlist: Ref<Playlist> = obj.borrow();
      if let PlaylistType::UserPlaylist(id, name) = &playlist.playlist_type {
        found_playlist = Some((*id, name.clone()));
      }
    }

    if found_playlist.is_none() {
      for i in 0..store_for_gesture.n_items() {
        if let Some(item) = store_for_gesture.item(i) {
          let obj = item.downcast::<BoxedAnyObject>().unwrap();
          let playlist: Ref<Playlist> = obj.borrow();
          if let PlaylistType::UserPlaylist(id, name) = &playlist.playlist_type {
            found_playlist = Some((*id, name.clone()));
            sel_for_gesture.set_selected(i);
            break;
          }
        }
      }
    }

    if let Some(pl) = found_playlist {
      *cp.borrow_mut() = Some(pl);
      let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
      popover.set_pointing_to(Some(&rect));
      popover.popup();
      gesture.set_state(gtk::EventSequenceState::Claimed);
    }
  });

  columnview.add_controller(gesture);

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

  if let Ok(user_playlists) = get_user_playlists() {
    for playlist in user_playlists {
      store.append(&BoxedAnyObject::new(Playlist {
        name: playlist.name.clone(),
        playlist_type: PlaylistType::UserPlaylist(playlist.id, playlist.name.clone()),
      }));
    }
  }

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

fn load_user_playlist_items(items: &[UserPlaylistItem], store: &ListStore) {
  for item in items {
    match item {
      UserPlaylistItem::Track(track) => {
        store.append(&BoxedAnyObject::new(track.clone()));
      }
      UserPlaylistItem::Video(video) => {
        store.append(&BoxedAnyObject::new(video.clone()));
      }
    }
  }
}

fn handle_drop_on_playlist(playlist_id: i32, data: &str) -> bool {
  let mut success = false;
  for line in data.lines() {
    if let Some(filename) = line.strip_prefix("track:") {
      if add_track_to_playlist(playlist_id, filename).is_ok() {
        success = true;
      }
    }
    if let Some(video_id_str) = line.strip_prefix("video:") {
      if let Ok(video_id) = video_id_str.parse::<i32>() {
        if add_video_to_playlist(playlist_id, video_id).is_ok() {
          success = true;
        }
      }
    }
  }
  success
}

fn show_rename_dialog(
  playback_controller: Rc<PlaybackController>,
  playlist_id: i32,
  current_name: &str,
  on_renamed: impl Fn() + 'static,
) {
  let wnd = playback_controller.window();

  let dialog = gtk::Window::builder()
    .title("Rename Playlist")
    .default_width(350)
    .default_height(150)
    .modal(true)
    .transient_for(&**wnd)
    .build();

  let content = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .spacing(12)
    .margin_top(24)
    .margin_bottom(24)
    .margin_start(24)
    .margin_end(24)
    .build();

  let name_entry = gtk::Entry::builder()
    .text(current_name)
    .hexpand(true)
    .build();

  let button_box = gtk::Box::builder()
    .orientation(gtk::Orientation::Horizontal)
    .spacing(12)
    .halign(gtk::Align::End)
    .build();

  let cancel_btn = gtk::Button::builder().label("Cancel").build();
  let rename_btn = gtk::Button::builder()
    .label("Rename")
    .css_classes(["suggested-action"])
    .build();

  button_box.append(&cancel_btn);
  button_box.append(&rename_btn);

  content.append(&name_entry);
  content.append(&button_box);

  dialog.set_child(Some(&content));

  let dialog_weak = dialog.downgrade();
  cancel_btn.connect_clicked(move |_| {
    if let Some(d) = dialog_weak.upgrade() {
      d.close();
    }
  });

  let dialog_weak = dialog.downgrade();
  let name_entry_clone = name_entry.clone();
  rename_btn.connect_clicked(move |_| {
    let new_name = name_entry_clone.text().to_string();
    if !new_name.is_empty() {
      if rename_playlist(playlist_id, &new_name).is_ok() {
        on_renamed();
        if let Some(d) = dialog_weak.upgrade() {
          d.close();
        }
      }
    }
  });

  dialog.present();
}
