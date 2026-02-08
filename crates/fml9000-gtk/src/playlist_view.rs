use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, setup_col, str_or_unknown};
use crate::video_widget::open_in_browser;
use crate::playback_controller::{PlaybackController, PlaybackSource};
use crate::settings::FmlSettings;
use fml9000_core::{
  get_playlist_items, reorder_playlist_items, remove_from_playlist, MediaItem,
  PlaylistItemIdentifier,
};
use gtk::gdk::{self, Key};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  ColumnView, ColumnViewColumn, CustomSorter, DragSource, DropTarget, EventControllerKey,
  GestureClick, MultiSelection, PopoverMenu, ScrolledWindow, SignalListItemFactory, SortListModel,
};
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

pub type CurrentPlaylistId = Rc<RefCell<Option<i32>>>;
pub type IsViewingPlaybackQueue = Rc<Cell<bool>>;

fn open_folder_in_explorer(file_path: &str) {
  if let Some(parent) = Path::new(file_path).parent() {
    let folder = parent.to_string_lossy();
    #[cfg(target_os = "linux")]
    {
      let _ = std::process::Command::new("xdg-open")
        .arg(folder.as_ref())
        .spawn();
    }
    #[cfg(target_os = "macos")]
    {
      let _ = std::process::Command::new("open")
        .arg(folder.as_ref())
        .spawn();
    }
    #[cfg(target_os = "windows")]
    {
      let _ = std::process::Command::new("explorer")
        .arg(folder.as_ref())
        .spawn();
    }
  }
}

fn try_get_item(obj: &BoxedAnyObject) -> Option<MediaItem> {
  obj.try_borrow::<MediaItem>().ok().map(|item| item.clone())
}

fn create_sorter(extract: impl Fn(&MediaItem) -> String + 'static) -> CustomSorter {
  CustomSorter::new(move |obj1, obj2| {
    let val1 = obj1
      .downcast_ref::<BoxedAnyObject>()
      .and_then(|o| try_get_item(o))
      .map(|item| extract(&item))
      .unwrap_or_default();
    let val2 = obj2
      .downcast_ref::<BoxedAnyObject>()
      .and_then(|o| try_get_item(o))
      .map(|item| extract(&item))
      .unwrap_or_default();
    val1.to_lowercase().cmp(&val2.to_lowercase()).into()
  })
}

fn create_column(
  settings: Rc<RefCell<FmlSettings>>,
  cb: impl Fn(&MediaItem) -> String + 'static,
) -> SignalListItemFactory {
  let factory = SignalListItemFactory::new();
  factory.connect_setup(move |_factory, item| {
    setup_col(item);
  });
  let settings_for_bind = settings.clone();
  factory.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let row_height = settings_for_bind.borrow().row_height;
    cell.set_row_height(row_height.height_pixels(), row_height.is_compact());
    if let Some(media_item) = try_get_item(&obj) {
      cell.set_entry(&Entry {
        name: cb(&media_item),
      });
    }
  });
  factory.connect_unbind(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    if let Some(child) = item.child() {
      if let Some(cell) = child.downcast_ref::<crate::grid_cell::GridCell>() {
        cell.set_entry(&Entry { name: String::new() });
      }
    }
  });
  factory
}

pub fn create_playlist_view(
  playlist_store: ListStore,
  playback_controller: Rc<PlaybackController>,
  settings: Rc<RefCell<FmlSettings>>,
  current_playlist_id: CurrentPlaylistId,
  is_viewing_playback_queue: IsViewingPlaybackQueue,
) -> ScrolledWindow {
  let artistalbum_sorter = create_sorter(|item| match item {
    MediaItem::Track(r) => format!(
      "{} // {}",
      str_or_unknown(&r.album),
      str_or_unknown(&r.artist),
    ),
    MediaItem::Video(v) => {
      let (artist, album) = parse_youtube_title(&v.title);
      format!("{} // {}", album, artist)
    }
  });

  let track_num_sorter = CustomSorter::new(move |obj1, obj2| {
    let get_track = |obj: &gtk::glib::Object| -> String {
      obj
        .downcast_ref::<BoxedAnyObject>()
        .and_then(|o| try_get_item(o))
        .map(|item| match &item {
          MediaItem::Track(r) => r.track.clone().unwrap_or_default(),
          MediaItem::Video(_) => String::new(),
        })
        .unwrap_or_default()
    };
    let val1 = get_track(obj1);
    let val2 = get_track(obj2);

    match (val1.parse::<i32>(), val2.parse::<i32>()) {
      (Ok(n1), Ok(n2)) => n1.cmp(&n2).into(),
      _ => val1.to_lowercase().cmp(&val2.to_lowercase()).into(),
    }
  });

  let title_sorter = create_sorter(|item| item.title().to_string());

  let filename_sorter = create_sorter(|item| match item {
    MediaItem::Track(r) => r.filename.clone(),
    MediaItem::Video(v) => v.video_id.clone(),
  });

  let sort_model = SortListModel::new(Some(playlist_store.clone()), None::<gtk::Sorter>);
  let playlist_sel = MultiSelection::new(Some(sort_model.clone()));
  let playlist_columnview = ColumnView::builder()
    .model(&playlist_sel)
    .build();

  let artistalbum = create_column(Rc::clone(&settings), |item| match item {
    MediaItem::Track(r) => {
      format!(
        "{} // {}",
        str_or_unknown(&r.album),
        str_or_unknown(&r.artist),
      )
    }
    MediaItem::Video(v) => {
      let (artist, album) = parse_youtube_title(&v.title);
      format!("{} // {}", album, artist)
    }
  });

  let track_num = create_column(Rc::clone(&settings), |item| match item {
    MediaItem::Track(r) => r.track.clone().unwrap_or_default(),
    MediaItem::Video(_) => String::new(),
  });

  let duration = create_column(Rc::clone(&settings), |_item| _item.duration_str());

  let duration_sorter = CustomSorter::new(move |obj1, obj2| {
    let get_duration = |obj: &gtk::glib::Object| -> i32 {
      obj
        .downcast_ref::<BoxedAnyObject>()
        .and_then(|o| try_get_item(o))
        .map(|item| item.duration_seconds().unwrap_or(0))
        .unwrap_or(0)
    };
    get_duration(obj1).cmp(&get_duration(obj2)).into()
  });

  let title = create_column(Rc::clone(&settings), |item| item.title().to_string());

  let filename = create_column(Rc::clone(&settings), |item| match item {
    MediaItem::Track(r) => r.filename.clone(),
    MediaItem::Video(v) => v.video_id.clone(),
  });

  let date_added = create_column(Rc::clone(&settings), |item| item.added_str());

  let date_sorter = create_sorter(|item| item.added_str());

  let last_played = create_column(Rc::clone(&settings), |item| item.last_played_str());

  let last_played_sorter = create_sorter(|item| item.last_played_str());

  let play_count = create_column(Rc::clone(&settings), |item| {
    let count = item.play_count();
    if count > 0 { count.to_string() } else { String::new() }
  });

  let play_count_sorter = CustomSorter::new(move |obj1, obj2| {
    let get_count = |obj: &gtk::glib::Object| -> i32 {
      obj
        .downcast_ref::<BoxedAnyObject>()
        .and_then(|o| try_get_item(o))
        .map(|item| item.play_count())
        .unwrap_or(0)
    };
    get_count(obj1).cmp(&get_count(obj2)).into()
  });

  let playlist_col1 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(400)
    .title("Album / Artist")
    .factory(&artistalbum)
    .sorter(&artistalbum_sorter)
    .build();

  let playlist_col2 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("#")
    .fixed_width(20)
    .sorter(&track_num_sorter)
    .factory(&track_num)
    .build();

  let duration_col = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("Duration")
    .fixed_width(60)
    .sorter(&duration_sorter)
    .factory(&duration)
    .build();

  let playlist_col3 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("Title")
    .fixed_width(300)
    .factory(&title)
    .sorter(&title_sorter)
    .build();

  let playlist_col4 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(100)
    .title("Date Added")
    .factory(&date_added)
    .sorter(&date_sorter)
    .build();

  let last_played_col = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(100)
    .title("Last Played")
    .factory(&last_played)
    .sorter(&last_played_sorter)
    .build();

  let play_count_col = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(50)
    .title("Plays")
    .factory(&play_count)
    .sorter(&play_count_sorter)
    .build();

  let playlist_col5 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(2000)
    .title("Filename")
    .factory(&filename)
    .sorter(&filename_sorter)
    .build();

  playlist_columnview.append_column(&playlist_col1);
  playlist_columnview.append_column(&playlist_col2);
  playlist_columnview.append_column(&duration_col);
  playlist_columnview.append_column(&playlist_col3);
  playlist_columnview.append_column(&playlist_col4);
  playlist_columnview.append_column(&last_played_col);
  playlist_columnview.append_column(&play_count_col);
  playlist_columnview.append_column(&playlist_col5);

  playlist_columnview
    .bind_property("sorter", &sort_model, "sorter")
    .sync_create()
    .build();

  let drag_source = DragSource::new();
  drag_source.set_actions(gdk::DragAction::COPY);
  let sel_for_drag = playlist_sel.clone();
  let store_for_drag = playlist_store.clone();
  drag_source.connect_prepare(move |_source, _x, _y| {
    let selection = sel_for_drag.selection();
    let mut items: Vec<String> = Vec::new();

    let n_items = store_for_drag.n_items();
    for pos in 0..n_items {
      if selection.contains(pos) {
        if let Some(item) = store_for_drag.item(pos) {
          if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
            if let Some(playlist_item) = try_get_item(&obj) {
              items.push(match playlist_item {
                MediaItem::Track(track) => format!("track:{}", track.filename),
                MediaItem::Video(video) => format!("video:{}", video.id),
              });
            }
          }
        }
      }
    }

    if items.is_empty() {
      return None;
    }

    let data = items.join("\n");
    Some(gdk::ContentProvider::for_value(&data.to_value()))
  });
  playlist_columnview.add_controller(drag_source);

  let drop_target = DropTarget::new(gtk::glib::Type::STRING, gdk::DragAction::MOVE);
  let cpid_for_drop = current_playlist_id.clone();
  let store_for_drop = playlist_store.clone();
  drop_target.connect_drop(move |_target, value, _x, y| {
    let Some(playlist_id) = *cpid_for_drop.borrow() else {
      return false;
    };

    let Ok(data) = value.get::<String>() else {
      return false;
    };

    let n_items = store_for_drop.n_items();
    if n_items == 0 {
      return false;
    }

    let row_height = 24.0_f64;
    let header_height = 24.0_f64;
    let drop_index = ((y - header_height) / row_height).floor().max(0.0) as u32;
    let drop_index = drop_index.min(n_items.saturating_sub(1));

    let mut identifiers: Vec<PlaylistItemIdentifier> = Vec::new();
    let mut dragged_indices: Vec<u32> = Vec::new();

    for line in data.lines() {
      if let Some(filename) = line.strip_prefix("track:") {
        for i in 0..n_items {
          if let Some(item) = store_for_drop.item(i) {
            if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
              if let Some(MediaItem::Track(t)) = try_get_item(&obj) {
                if t.filename == filename {
                  dragged_indices.push(i);
                  break;
                }
              }
            }
          }
        }
      }
      if let Some(video_id_str) = line.strip_prefix("video:") {
        if let Ok(vid_id) = video_id_str.parse::<i32>() {
          for i in 0..n_items {
            if let Some(item) = store_for_drop.item(i) {
              if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
                if let Some(MediaItem::Video(v)) = try_get_item(&obj) {
                  if v.id == vid_id {
                    dragged_indices.push(i);
                    break;
                  }
                }
              }
            }
          }
        }
      }
    }

    if dragged_indices.is_empty() {
      return false;
    }

    dragged_indices.sort();

    for i in 0..n_items {
      if !dragged_indices.contains(&i) {
        if let Some(item) = store_for_drop.item(i) {
          if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
            if let Some(playlist_item) = try_get_item(&obj) {
              match playlist_item {
                MediaItem::Track(t) => identifiers.push(PlaylistItemIdentifier::Track(t.filename.clone())),
                MediaItem::Video(v) => identifiers.push(PlaylistItemIdentifier::Video(v.id)),
              }
            }
          }
        }
      }
    }

    let insert_pos = if drop_index == 0 {
      0
    } else {
      let mut pos = 0;
      for i in 0..=drop_index {
        if !dragged_indices.contains(&i) {
          pos += 1;
        }
      }
      pos
    };

    let mut dragged_items: Vec<PlaylistItemIdentifier> = Vec::new();
    for &idx in &dragged_indices {
      if let Some(item) = store_for_drop.item(idx) {
        if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
          if let Some(playlist_item) = try_get_item(&obj) {
            match playlist_item {
              MediaItem::Track(t) => dragged_items.push(PlaylistItemIdentifier::Track(t.filename.clone())),
              MediaItem::Video(v) => dragged_items.push(PlaylistItemIdentifier::Video(v.id)),
            }
          }
        }
      }
    }

    for item in dragged_items.into_iter().rev() {
      identifiers.insert(insert_pos, item);
    }

    if reorder_playlist_items(playlist_id, &identifiers).is_ok() {
      store_for_drop.remove_all();
      if let Ok(items) = get_playlist_items(playlist_id) {
        for item in items {
          store_for_drop.append(&BoxedAnyObject::new(item));
        }
      }
    }

    true
  });
  playlist_columnview.add_controller(drop_target);

  let pc_for_keys = playback_controller.clone();
  let key_controller = EventControllerKey::new();
  key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
  key_controller.connect_key_pressed(move |_, key, _, _| {
    match key {
      Key::space => {
        match pc_for_keys.playback_source() {
          PlaybackSource::Local => {
            if pc_for_keys.audio().is_playing() {
              pc_for_keys.audio().pause();
            } else {
              pc_for_keys.audio().play();
            }
          }
          PlaybackSource::YouTube => {
            if pc_for_keys.video_widget().is_playing() {
              pc_for_keys.video_widget().pause();
            } else {
              pc_for_keys.video_widget().unpause();
            }
          }
          PlaybackSource::None => {}
        }
        gtk::glib::Propagation::Stop
      }
      Key::n | Key::N => {
        pc_for_keys.play_next();
        gtk::glib::Propagation::Stop
      }
      Key::p | Key::P => {
        pc_for_keys.play_prev();
        gtk::glib::Propagation::Stop
      }
      Key::s | Key::S => {
        pc_for_keys.stop();
        gtk::glib::Propagation::Stop
      }
      Key::r | Key::R => {
        let enabled = !pc_for_keys.shuffle_enabled();
        pc_for_keys.set_shuffle_enabled(enabled);
        gtk::glib::Propagation::Stop
      }
      _ => gtk::glib::Propagation::Proceed,
    }
  });
  playlist_columnview.add_controller(key_controller);

  let pc_for_activate = playback_controller.clone();
  let store_for_activate = playlist_store.clone();
  let settings_for_activate = settings.clone();
  playlist_columnview.connect_activate(move |_columnview, pos| {
    if let Some(item) = store_for_activate.item(pos) {
      if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
        if let Some(playlist_item) = try_get_item(&obj) {
          match playlist_item {
            MediaItem::Track(_) => {
              pc_for_activate.play_index(pos);
            }
            MediaItem::Video(video) => {
              let audio_only = settings_for_activate.borrow().youtube_audio_only;
              pc_for_activate.play_youtube_video(&video, audio_only);
            }
          }
        }
      }
    }
  });

  let video_menu = gtk::gio::Menu::new();
  video_menu.append(Some("Play (Audio)"), Some("playlist.play-audio"));
  video_menu.append(Some("Play (Video)"), Some("playlist.play-video"));
  video_menu.append(Some("Play Next"), Some("playlist.queue-video"));
  video_menu.append(Some("Open in Browser"), Some("playlist.open-browser"));
  video_menu.append(Some("Remove from Playlist"), Some("playlist.remove-video"));
  video_menu.append(Some("Remove from Queue"), Some("playlist.remove-video-from-queue"));

  let video_popover = PopoverMenu::from_model(Some(&video_menu));
  video_popover.set_parent(&playlist_columnview);
  video_popover.set_has_arrow(false);

  let track_menu = gtk::gio::Menu::new();
  track_menu.append(Some("Play Next"), Some("playlist.queue-track"));
  track_menu.append(Some("Open Folder"), Some("playlist.open-folder"));
  track_menu.append(Some("Remove from Playlist"), Some("playlist.remove-track"));
  track_menu.append(Some("Remove from Queue"), Some("playlist.remove-track-from-queue"));

  let track_popover = PopoverMenu::from_model(Some(&track_menu));
  track_popover.set_parent(&playlist_columnview);
  track_popover.set_has_arrow(false);

  let current_item: Rc<RefCell<Option<MediaItem>>> = Rc::new(RefCell::new(None));

  let action_group = gtk::gio::SimpleActionGroup::new();

  let ci = current_item.clone();
  let pc = playback_controller.clone();
  let play_audio = gtk::gio::SimpleAction::new("play-audio", None);
  play_audio.connect_activate(move |_, _| {
    if let Some(MediaItem::Video(video)) = ci.borrow().as_ref() {
      pc.play_youtube_video(video, true);
    }
  });
  action_group.add_action(&play_audio);

  let ci = current_item.clone();
  let pc = playback_controller.clone();
  let play_video = gtk::gio::SimpleAction::new("play-video", None);
  play_video.connect_activate(move |_, _| {
    if let Some(MediaItem::Video(video)) = ci.borrow().as_ref() {
      pc.play_youtube_video(video, false);
    }
  });
  action_group.add_action(&play_video);

  let ci = current_item.clone();
  let open_browser = gtk::gio::SimpleAction::new("open-browser", None);
  open_browser.connect_activate(move |_, _| {
    if let Some(vid_id) = ci.borrow().as_ref().and_then(|i| i.youtube_video_id()) {
      open_in_browser(vid_id);
    }
  });
  action_group.add_action(&open_browser);

  let ci = current_item.clone();
  let open_folder = gtk::gio::SimpleAction::new("open-folder", None);
  open_folder.connect_activate(move |_, _| {
    if let Some(filename) = ci.borrow().as_ref().and_then(|i| i.track_filename()) {
      open_folder_in_explorer(filename);
    }
  });
  action_group.add_action(&open_folder);

  let ci = current_item.clone();
  let pc = playback_controller.clone();
  let queue_track = gtk::gio::SimpleAction::new("queue-track", None);
  queue_track.connect_activate(move |_, _| {
    if let Some(item) = ci.borrow().as_ref() {
      pc.queue_item(item);
    }
  });
  action_group.add_action(&queue_track);

  let ci = current_item.clone();
  let pc = playback_controller.clone();
  let queue_video = gtk::gio::SimpleAction::new("queue-video", None);
  queue_video.connect_activate(move |_, _| {
    if let Some(item) = ci.borrow().as_ref() {
      pc.queue_item(item);
    }
  });
  action_group.add_action(&queue_video);

  let ci = current_item.clone();
  let cpid = current_playlist_id.clone();
  let store_for_remove = playlist_store.clone();
  let remove_track = gtk::gio::SimpleAction::new("remove-track", None);
  remove_track.connect_activate(move |_, _| {
    if let Some(playlist_id) = *cpid.borrow() {
      if let Some(item) = ci.borrow().as_ref() {
        if remove_from_playlist(playlist_id, item).is_ok() {
          let filename = item.track_filename().map(|s| s.to_string());
          for i in 0..store_for_remove.n_items() {
            if let Some(store_item) = store_for_remove.item(i) {
              if let Ok(obj) = store_item.downcast::<BoxedAnyObject>() {
                if let Some(mi) = try_get_item(&obj) {
                  if mi.track_filename().map(|s| s.to_string()) == filename {
                    store_for_remove.remove(i);
                    break;
                  }
                }
              }
            }
          }
        }
      }
    }
  });
  action_group.add_action(&remove_track);

  let ci = current_item.clone();
  let cpid = current_playlist_id.clone();
  let store_for_remove = playlist_store.clone();
  let remove_video = gtk::gio::SimpleAction::new("remove-video", None);
  remove_video.connect_activate(move |_, _| {
    if let Some(playlist_id) = *cpid.borrow() {
      if let Some(item) = ci.borrow().as_ref() {
        if remove_from_playlist(playlist_id, item).is_ok() {
          let vid_id = item.video_db_id();
          for i in 0..store_for_remove.n_items() {
            if let Some(store_item) = store_for_remove.item(i) {
              if let Ok(obj) = store_item.downcast::<BoxedAnyObject>() {
                if let Some(mi) = try_get_item(&obj) {
                  if mi.video_db_id() == vid_id {
                    store_for_remove.remove(i);
                    break;
                  }
                }
              }
            }
          }
        }
      }
    }
  });
  action_group.add_action(&remove_video);

  let ci = current_item.clone();
  let pc = playback_controller.clone();
  let store_for_queue_remove = playlist_store.clone();
  let is_queue = is_viewing_playback_queue.clone();
  let remove_track_from_queue = gtk::gio::SimpleAction::new("remove-track-from-queue", None);
  remove_track_from_queue.connect_activate(move |_, _| {
    if is_queue.get() {
      if let Some(item) = ci.borrow().as_ref() {
        pc.remove_item_from_queue(item);
        let filename = item.track_filename().map(|s| s.to_string());
        for i in 0..store_for_queue_remove.n_items() {
          if let Some(store_item) = store_for_queue_remove.item(i) {
            if let Ok(obj) = store_item.downcast::<BoxedAnyObject>() {
              if let Some(mi) = try_get_item(&obj) {
                if mi.track_filename().map(|s| s.to_string()) == filename {
                  store_for_queue_remove.remove(i);
                  break;
                }
              }
            }
          }
        }
      }
    }
  });
  action_group.add_action(&remove_track_from_queue);

  let ci = current_item.clone();
  let pc = playback_controller.clone();
  let store_for_queue_remove = playlist_store.clone();
  let is_queue = is_viewing_playback_queue.clone();
  let remove_video_from_queue = gtk::gio::SimpleAction::new("remove-video-from-queue", None);
  remove_video_from_queue.connect_activate(move |_, _| {
    if is_queue.get() {
      if let Some(item) = ci.borrow().as_ref() {
        pc.remove_item_from_queue(item);
        let vid_id = item.video_db_id();
        for i in 0..store_for_queue_remove.n_items() {
          if let Some(store_item) = store_for_queue_remove.item(i) {
            if let Ok(obj) = store_item.downcast::<BoxedAnyObject>() {
              if let Some(mi) = try_get_item(&obj) {
                if mi.video_db_id() == vid_id {
                  store_for_queue_remove.remove(i);
                  break;
                }
              }
            }
          }
        }
      }
    }
  });
  action_group.add_action(&remove_video_from_queue);

  playlist_columnview.insert_action_group("playlist", Some(&action_group));

  let gesture = GestureClick::builder().button(3).build();
  let ci = current_item.clone();
  let video_pop = video_popover.clone();
  let track_pop = track_popover.clone();
  let store = playlist_store.clone();
  let sel_for_gesture = playlist_sel.clone();
  let colview_for_gesture = playlist_columnview.clone();
  gesture.connect_pressed(move |gesture, _n_press, x, y| {
    colview_for_gesture.grab_focus();

    let mut found_pos: Option<u32> = None;
    if let Some(picked) = colview_for_gesture.pick(x, y, gtk::PickFlags::DEFAULT) {
      let mut widget = Some(picked);
      while let Some(w) = widget {
        if w.type_().name() == "GtkColumnViewRow" {
          found_pos = Some(w.property::<u32>("position"));
          break;
        }
        widget = w.parent();
      }
    }

    let pos = found_pos.or_else(|| {
      let selection = sel_for_gesture.selection();
      if !selection.is_empty() {
        Some(selection.minimum())
      } else {
        None
      }
    });

    let Some(pos) = pos else {
      return;
    };

    sel_for_gesture.select_item(pos, true);

    if let Some(item) = store.item(pos) {
      if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
        if let Some(media_item) = try_get_item(&obj) {
          let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
          *ci.borrow_mut() = Some(media_item.clone());
          match &media_item {
            MediaItem::Video(_) => {
              video_pop.set_pointing_to(Some(&rect));
              video_pop.popup();
            }
            MediaItem::Track(_) => {
              track_pop.set_pointing_to(Some(&rect));
              track_pop.popup();
            }
          }
          gesture.set_state(gtk::EventSequenceState::Claimed);
        }
      }
    }
  });

  playlist_columnview.add_controller(gesture);

  let columnview_for_scroll = playlist_columnview.clone();
  let selection_for_scroll = playlist_sel.clone();
  playback_controller.set_on_track_changed(Some(Rc::new(move |index| {
    selection_for_scroll.select_item(index, true);
    columnview_for_scroll.scroll_to(index, None::<&ColumnViewColumn>, gtk::ListScrollFlags::FOCUS, None);
  })));

  let store_for_selection = playlist_store.clone();
  let pc_for_selection = playback_controller.clone();
  playlist_sel.connect_selection_changed(move |sel, _, _| {
    let selection = sel.selection();
    if selection.is_empty() {
      return;
    }
    let pos = selection.minimum();
    if let Some(item) = store_for_selection.item(pos) {
      if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
        if let Some(playlist_item) = try_get_item(&obj) {
          if let MediaItem::Track(track) = playlist_item {
            pc_for_selection.show_track_album_art(&track);
          }
        }
      }
    }
  });

  ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build()
}

fn parse_youtube_title(title: &str) -> (String, String) {
  if let Some((artist, rest)) = title.split_once(" - ") {
    let album = rest
      .trim()
      .split('[')
      .next()
      .unwrap_or(rest)
      .trim()
      .trim_end_matches(|c| c == ')' || c == ' ')
      .split('(')
      .next()
      .unwrap_or(rest)
      .trim();

    (artist.trim().to_string(), album.to_string())
  } else {
    ("Unknown".to_string(), title.to_string())
  }
}
