use crate::grid_cell::Entry;
use crate::new_playlist_dialog;
use crate::playback_controller::PlaybackController;
use crate::settings::FmlSettings;
use crate::youtube_add_dialog;
use adw::prelude::*;
use fml9000_core::{
  add_to_playlist, delete_playlist, get_all_media, get_all_videos, get_playlist_items,
  get_queue_items, get_user_playlists, get_videos_for_channel, get_youtube_channels,
  load_recently_added_items, load_recently_played_items, load_tracks, rename_playlist, MediaItem,
};
use gtk::gdk;
use gtk::gio::ListStore;
use gtk::glib;
use gtk::glib::BoxedAnyObject;
use gtk::{ColumnView, ColumnViewColumn, DropTarget, GestureClick, PopoverMenu, ScrolledWindow, SignalListItemFactory, SingleSelection, TreeExpander, TreeListModel, TreeListRow};
use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;

#[derive(Clone, PartialEq)]
enum PlaylistType {
  AllMedia,
  AllTracks,
  AllVideos,
  RecentlyAdded,
  RecentlyPlayed,
  PlaybackQueue,
  YouTubeChannel(i32, String),
  UserPlaylist(i32, String),
}

struct Playlist {
  name: String,
  playlist_type: PlaylistType,
}

#[derive(Clone)]
enum SectionKind {
  AutoPlaylists,
  UserPlaylists,
  YouTubeChannels,
}

enum PlaylistEntry {
  SectionHeader(String, SectionKind),
  Playlist(Playlist),
}

fn build_children_store(kind: &SectionKind) -> ListStore {
  let store = ListStore::new::<BoxedAnyObject>();
  match kind {
    SectionKind::AutoPlaylists => {
      store.append(&BoxedAnyObject::new(PlaylistEntry::Playlist(Playlist {
        name: "All Media".to_string(),
        playlist_type: PlaylistType::AllMedia,
      })));
      store.append(&BoxedAnyObject::new(PlaylistEntry::Playlist(Playlist {
        name: "All Tracks".to_string(),
        playlist_type: PlaylistType::AllTracks,
      })));
      store.append(&BoxedAnyObject::new(PlaylistEntry::Playlist(Playlist {
        name: "All Videos".to_string(),
        playlist_type: PlaylistType::AllVideos,
      })));
      store.append(&BoxedAnyObject::new(PlaylistEntry::Playlist(Playlist {
        name: "Recently added".to_string(),
        playlist_type: PlaylistType::RecentlyAdded,
      })));
      store.append(&BoxedAnyObject::new(PlaylistEntry::Playlist(Playlist {
        name: "Recently played".to_string(),
        playlist_type: PlaylistType::RecentlyPlayed,
      })));
      store.append(&BoxedAnyObject::new(PlaylistEntry::Playlist(Playlist {
        name: "Playback queue".to_string(),
        playlist_type: PlaylistType::PlaybackQueue,
      })));
    }
    SectionKind::UserPlaylists => {
      if let Ok(user_playlists) = get_user_playlists() {
        for playlist in user_playlists {
          store.append(&BoxedAnyObject::new(PlaylistEntry::Playlist(Playlist {
            name: playlist.name.clone(),
            playlist_type: PlaylistType::UserPlaylist(playlist.id, playlist.name.clone()),
          })));
        }
      }
    }
    SectionKind::YouTubeChannels => {
      if let Ok(channels) = get_youtube_channels() {
        for channel in channels {
          store.append(&BoxedAnyObject::new(PlaylistEntry::Playlist(Playlist {
            name: channel.name.clone(),
            playlist_type: PlaylistType::YouTubeChannel(channel.id, channel.name.clone()),
          })));
        }
      }
    }
  }
  store
}

fn get_playlist_from_tree_row(row: &TreeListRow) -> Option<BoxedAnyObject> {
  let obj = row.item()?.downcast::<BoxedAnyObject>().ok()?;
  {
    let entry: Ref<PlaylistEntry> = obj.borrow();
    if matches!(&*entry, PlaylistEntry::SectionHeader(_, _)) {
      return None;
    }
  }
  Some(obj)
}

pub fn create_playlist_manager(
  playlist_mgr_store: &ListStore,
  main_playlist_store: ListStore,
  playback_controller: Rc<PlaybackController>,
  settings: Rc<RefCell<FmlSettings>>,
  current_playlist_id: Rc<RefCell<Option<i32>>>,
  is_viewing_playback_queue: Rc<Cell<bool>>,
) -> (gtk::Box, SingleSelection) {
  populate_playlist_store(playlist_mgr_store);

  let tree_model = TreeListModel::new(playlist_mgr_store.clone(), false, true, |item| {
    let obj = item.downcast_ref::<BoxedAnyObject>()?;
    let entry: Ref<PlaylistEntry> = obj.borrow();
    match &*entry {
      PlaylistEntry::SectionHeader(_, kind) => Some(build_children_store(kind).into()),
      PlaylistEntry::Playlist(_) => None,
    }
  });

  let selection = SingleSelection::builder()
    .model(&tree_model)
    .autoselect(false)
    .build();
  let columnview = ColumnView::builder().model(&selection).build();
  let factory = SignalListItemFactory::new();

  let playback_controller_for_setup = playback_controller.clone();
  let playlist_mgr_store_for_setup = playlist_mgr_store.clone();
  let selection_for_setup = selection.clone();
  let main_store_for_setup = main_playlist_store.clone();
  let current_playlist_id_for_setup = current_playlist_id.clone();
  let tree_model_for_setup = tree_model.clone();
  factory.connect_setup(move |_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let cell = crate::grid_cell::GridCell::new();
    let expander = TreeExpander::new();
    expander.set_child(Some(&cell));
    list_item.set_child(Some(&expander));

    let drop_target = DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY);

    let current_playlist_id: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));

    let pc = playback_controller_for_setup.clone();
    let store = playlist_mgr_store_for_setup.clone();
    let pid = current_playlist_id.clone();
    let pid_for_enter = current_playlist_id.clone();
    let expander_for_enter = expander.clone();
    drop_target.connect_enter(move |_target, _x, _y| {
      if pid_for_enter.borrow().is_some() {
        if let Some(child) = expander_for_enter.child() {
          child.add_css_class("drop-target-hover");
        }
      }
      gdk::DragAction::COPY
    });

    let expander_for_leave = expander.clone();
    drop_target.connect_leave(move |_target| {
      if let Some(child) = expander_for_leave.child() {
        child.remove_css_class("drop-target-hover");
      }
    });

    let expander_for_drop = expander.clone();
    let sel = selection_for_setup.clone();
    let main_store_for_drop = main_store_for_setup.clone();
    let current_pid_for_drop = current_playlist_id_for_setup.clone();
    let tree_model_for_drop = tree_model_for_setup.clone();
    drop_target.connect_drop(move |_target, value, _x, _y| {
      if let Some(child) = expander_for_drop.child() {
        child.remove_css_class("drop-target-hover");
      }

      let Ok(data) = value.get::<String>() else {
        return false;
      };

      if let Some(playlist_id) = *pid.borrow() {
        let result = handle_drop_on_playlist(playlist_id, &data);
        if result {
          for i in 0..tree_model_for_drop.n_items() {
            if let Some(item) = tree_model_for_drop.item(i) {
              if let Some(row) = item.downcast_ref::<TreeListRow>() {
                if let Some(obj) = row.item() {
                  if let Some(obj) = obj.downcast_ref::<BoxedAnyObject>() {
                    let entry: Ref<PlaylistEntry> = obj.borrow();
                    if let PlaylistEntry::Playlist(playlist) = &*entry {
                      if let PlaylistType::UserPlaylist(id, _) = &playlist.playlist_type {
                        if *id == playlist_id {
                          sel.set_selected(i);
                          *current_pid_for_drop.borrow_mut() = Some(playlist_id);
                          main_store_for_drop.remove_all();
                          if let Ok(items) = get_playlist_items(playlist_id) {
                            load_media_items(&items, &main_store_for_drop);
                          }
                          break;
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
        return result;
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

    expander.add_controller(drop_target);

    if let Some(cell) = expander.child().and_then(|c| c.downcast::<crate::grid_cell::GridCell>().ok()) {
      cell.set_playlist_id(current_playlist_id);
    }
  });

  let settings_for_bind = settings.clone();
  factory.connect_bind(move |_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let expander = list_item.child().unwrap().downcast::<TreeExpander>().unwrap();
    let row = list_item.item().unwrap().downcast::<TreeListRow>().unwrap();
    expander.set_list_row(Some(&row));

    let cell = expander.child().unwrap().downcast::<crate::grid_cell::GridCell>().unwrap();
    let row_height = settings_for_bind.borrow().row_height;
    cell.set_row_height(row_height.height_pixels(), row_height.is_compact());

    let obj = row.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let entry: Ref<PlaylistEntry> = obj.borrow();
    match &*entry {
      PlaylistEntry::SectionHeader(name, _) => {
        cell.set_entry(&Entry { name: name.clone() });
        cell.add_css_class("section-header");
        cell.remove_css_class("user-playlist");
        cell.set_playlist_id_value(None);
      }
      PlaylistEntry::Playlist(playlist) => {
        cell.remove_css_class("section-header");
        if let PlaylistType::UserPlaylist(id, _) = &playlist.playlist_type {
          cell.set_entry(&Entry {
            name: playlist.name.clone(),
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
      }
    }
  });

  factory.connect_unbind(move |_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let expander = list_item.child().unwrap().downcast::<TreeExpander>().unwrap();
    expander.set_list_row(None::<&TreeListRow>);
  });

  let column = ColumnViewColumn::builder()
    .title("Playlists")
    .factory(&factory)
    .expand(true)
    .build();

  columnview.append_column(&column);

  let main_playlist_store_clone = main_playlist_store.clone();
  let current_playlist_id_clone = current_playlist_id.clone();
  let playback_controller_clone = playback_controller.clone();
  let is_viewing_playback_queue_clone = is_viewing_playback_queue.clone();
  let main_playlist_store_for_callback = main_playlist_store.clone();
  selection.connect_selection_changed(move |sel, _, _| {
    if let Some(item) = sel.selected_item() {
      let row = item.downcast::<TreeListRow>().unwrap();
      let Some(obj) = get_playlist_from_tree_row(&row) else {
        return;
      };
      let entry: Ref<PlaylistEntry> = obj.borrow();
      let PlaylistEntry::Playlist(playlist) = &*entry else {
        return;
      };

      main_playlist_store_clone.remove_all();

      let is_playback_queue = matches!(&playlist.playlist_type, PlaylistType::PlaybackQueue);
      is_viewing_playback_queue_clone.set(is_playback_queue);

      if is_playback_queue {
        let store = main_playlist_store_for_callback.clone();
        playback_controller_clone.set_on_queue_changed(Some(Rc::new(move || {
          store.remove_all();
          let items = get_queue_items();
          load_media_items(&items, &store);
        })));
      } else {
        playback_controller_clone.set_on_queue_changed(None);
      }

      match &playlist.playlist_type {
        PlaylistType::AllMedia => {
          *current_playlist_id_clone.borrow_mut() = None;
          let items = get_all_media();
          load_media_items(&items, &main_playlist_store_clone);
        }
        PlaylistType::AllTracks => {
          *current_playlist_id_clone.borrow_mut() = None;
          let tracks = load_tracks().unwrap_or_default();
          let items: Vec<MediaItem> = tracks.into_iter().map(MediaItem::Track).collect();
          load_media_items(&items, &main_playlist_store_clone);
        }
        PlaylistType::AllVideos => {
          *current_playlist_id_clone.borrow_mut() = None;
          if let Ok(videos) = get_all_videos() {
            let items: Vec<MediaItem> = videos.into_iter().map(MediaItem::Video).collect();
            load_media_items(&items, &main_playlist_store_clone);
          }
        }
        PlaylistType::RecentlyAdded => {
          *current_playlist_id_clone.borrow_mut() = None;
          let items = load_recently_added_items(0);
          load_media_items(&items, &main_playlist_store_clone);
        }
        PlaylistType::RecentlyPlayed => {
          *current_playlist_id_clone.borrow_mut() = None;
          let items = load_recently_played_items(100);
          load_media_items(&items, &main_playlist_store_clone);
        }
        PlaylistType::PlaybackQueue => {
          *current_playlist_id_clone.borrow_mut() = None;
          let items = get_queue_items();
          load_media_items(&items, &main_playlist_store_clone);
        }
        PlaylistType::YouTubeChannel(id, _) => {
          *current_playlist_id_clone.borrow_mut() = None;
          if let Ok(videos) = get_videos_for_channel(*id) {
            let items: Vec<MediaItem> = videos.into_iter().map(|v| MediaItem::Video(v)).collect();
            load_media_items(&items, &main_playlist_store_clone);
          }
        }
        PlaylistType::UserPlaylist(id, _) => {
          *current_playlist_id_clone.borrow_mut() = Some(*id);
          if let Ok(items) = get_playlist_items(*id) {
            load_media_items(&items, &main_playlist_store_clone);
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
  gesture.connect_pressed(move |gesture, _n_press, x, y| {
    let mut found_playlist: Option<(i32, String)> = None;

    if let Some(item) = sel_for_gesture.selected_item() {
      if let Ok(row) = item.downcast::<TreeListRow>() {
        if let Some(obj) = get_playlist_from_tree_row(&row) {
          let entry: Ref<PlaylistEntry> = obj.borrow();
          if let PlaylistEntry::Playlist(playlist) = &*entry {
            if let PlaylistType::UserPlaylist(id, name) = &playlist.playlist_type {
              found_playlist = Some((*id, name.clone()));
            }
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

  (container, selection)
}

pub fn populate_playlist_store(store: &ListStore) {
  store.append(&BoxedAnyObject::new(PlaylistEntry::SectionHeader(
    "Auto Playlists".to_string(),
    SectionKind::AutoPlaylists,
  )));
  store.append(&BoxedAnyObject::new(PlaylistEntry::SectionHeader(
    "Playlists".to_string(),
    SectionKind::UserPlaylists,
  )));
  store.append(&BoxedAnyObject::new(PlaylistEntry::SectionHeader(
    "YouTube Channels".to_string(),
    SectionKind::YouTubeChannels,
  )));
}

fn load_media_items(items: &[MediaItem], store: &ListStore) {
  for item in items {
    store.append(&BoxedAnyObject::new(item.clone()));
  }
}

fn handle_drop_on_playlist(playlist_id: i32, data: &str) -> bool {
  use fml9000_core::{load_track_by_filename, load_video_by_id};

  let mut success = false;
  for line in data.lines() {
    if let Some(filename) = line.strip_prefix("track:") {
      if let Some(track) = load_track_by_filename(filename) {
        if add_to_playlist(playlist_id, &MediaItem::Track(track)).is_ok() {
          success = true;
        }
      }
    }
    if let Some(video_id_str) = line.strip_prefix("video:") {
      if let Ok(video_id) = video_id_str.parse::<i32>() {
        if let Some(video) = load_video_by_id(video_id) {
          if add_to_playlist(playlist_id, &MediaItem::Video(video)).is_ok() {
            success = true;
          }
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
