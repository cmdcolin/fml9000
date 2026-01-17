use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, setup_col, str_or_unknown};
use crate::mpv_player::open_in_browser;
use crate::playback_controller::PlaybackController;
use crate::settings::FmlSettings;
use chrono::NaiveDateTime;
use fml9000::models::{Track, YouTubeVideo};
use fml9000::{
  get_playlist_items, reorder_playlist_items, remove_track_from_playlist,
  remove_video_from_playlist, PlaylistItemIdentifier, UserPlaylistItem,
};
use gtk::gdk;
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  ColumnView, ColumnViewColumn, CustomSorter, DragSource, DropTarget, GestureClick, MultiSelection,
  PopoverMenu, ScrolledWindow, SignalListItemFactory, SortListModel,
};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

pub type CurrentPlaylistId = Rc<RefCell<Option<i32>>>;

fn format_date(dt: Option<NaiveDateTime>) -> String {
  match dt {
    Some(d) => d.format("%Y-%m-%d").to_string(),
    None => String::new(),
  }
}

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

#[derive(Clone)]
enum PlaylistItem {
  Track(Rc<Track>),
  Video(Rc<YouTubeVideo>),
}

fn try_get_item(obj: &BoxedAnyObject) -> Option<PlaylistItem> {
  if let Ok(track) = obj.try_borrow::<Rc<Track>>() {
    return Some(PlaylistItem::Track(track.clone()));
  }
  if let Ok(video) = obj.try_borrow::<Rc<YouTubeVideo>>() {
    return Some(PlaylistItem::Video(video.clone()));
  }
  None
}

fn create_sorter(extract: impl Fn(&PlaylistItem) -> String + 'static) -> CustomSorter {
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
  cb: impl Fn(&PlaylistItem) -> String + 'static,
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
    if let Some(playlist_item) = try_get_item(&obj) {
      cell.set_entry(&Entry {
        name: cb(&playlist_item),
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
) -> ScrolledWindow {
  // Create sorters for each column
  let artistalbum_sorter = create_sorter(|item| match item {
    PlaylistItem::Track(r) => format!(
      "{} // {}",
      str_or_unknown(&r.album),
      str_or_unknown(&r.artist),
    ),
    PlaylistItem::Video(v) => format!("YouTube // {}", format_duration(v.duration_seconds)),
  });

  let track_num_sorter = CustomSorter::new(move |obj1, obj2| {
    let get_track = |obj: &gtk::glib::Object| -> String {
      obj
        .downcast_ref::<BoxedAnyObject>()
        .and_then(|o| try_get_item(o))
        .map(|item| match item {
          PlaylistItem::Track(r) => r.track.clone().unwrap_or_default(),
          PlaylistItem::Video(_) => String::new(),
        })
        .unwrap_or_default()
    };
    let val1 = get_track(obj1);
    let val2 = get_track(obj2);

    // Try numeric comparison first
    match (val1.parse::<i32>(), val2.parse::<i32>()) {
      (Ok(n1), Ok(n2)) => n1.cmp(&n2).into(),
      _ => val1.to_lowercase().cmp(&val2.to_lowercase()).into(),
    }
  });

  let title_sorter = create_sorter(|item| match item {
    PlaylistItem::Track(r) => r.title.clone().unwrap_or_default(),
    PlaylistItem::Video(v) => v.title.clone(),
  });

  let filename_sorter = create_sorter(|item| match item {
    PlaylistItem::Track(r) => r.filename.clone(),
    PlaylistItem::Video(v) => v.video_id.clone(),
  });

  // Wrap store in SortListModel
  let sort_model = SortListModel::new(Some(playlist_store.clone()), None::<gtk::Sorter>);
  let playlist_sel = MultiSelection::new(Some(sort_model.clone()));
  let playlist_columnview = ColumnView::builder()
    .model(&playlist_sel)
    .build();


  let artistalbum = create_column(Rc::clone(&settings), |item| match item {
    PlaylistItem::Track(r) => {
      format!(
        "{} // {}",
        str_or_unknown(&r.album),
        str_or_unknown(&r.artist),
      )
    }
    PlaylistItem::Video(v) => format!("YouTube // {}", format_duration(v.duration_seconds)),
  });

  let track_num = create_column(Rc::clone(&settings), |item| match item {
    PlaylistItem::Track(r) => r.track.clone().unwrap_or_default(),
    PlaylistItem::Video(_) => String::new(),
  });

  let title = create_column(Rc::clone(&settings), |item| match item {
    PlaylistItem::Track(r) => r.title.clone().unwrap_or_default(),
    PlaylistItem::Video(v) => v.title.clone(),
  });

  let filename = create_column(Rc::clone(&settings), |item| match item {
    PlaylistItem::Track(r) => r.filename.clone(),
    PlaylistItem::Video(v) => v.video_id.clone(),
  });

  let date_added = create_column(Rc::clone(&settings), |item| match item {
    PlaylistItem::Track(r) => format_date(r.added),
    PlaylistItem::Video(v) => format_date(Some(v.fetched_at)),
  });

  let date_sorter = create_sorter(|item| match item {
    PlaylistItem::Track(r) => format_date(r.added),
    PlaylistItem::Video(v) => format_date(Some(v.fetched_at)),
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
  playlist_columnview.append_column(&playlist_col3);
  playlist_columnview.append_column(&playlist_col4);
  playlist_columnview.append_column(&playlist_col5);

  // Bind ColumnView sorter to the SortListModel
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
                PlaylistItem::Track(track) => format!("track:{}", track.filename),
                PlaylistItem::Video(video) => format!("video:{}", video.id),
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
              if let Some(PlaylistItem::Track(t)) = try_get_item(&obj) {
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
                if let Some(PlaylistItem::Video(v)) = try_get_item(&obj) {
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
                PlaylistItem::Track(t) => identifiers.push(PlaylistItemIdentifier::Track(t.filename.clone())),
                PlaylistItem::Video(v) => identifiers.push(PlaylistItemIdentifier::Video(v.id)),
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
              PlaylistItem::Track(t) => dragged_items.push(PlaylistItemIdentifier::Track(t.filename.clone())),
              PlaylistItem::Video(v) => dragged_items.push(PlaylistItemIdentifier::Video(v.id)),
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
          match item {
            UserPlaylistItem::Track(track) => {
              store_for_drop.append(&BoxedAnyObject::new(track));
            }
            UserPlaylistItem::Video(video) => {
              store_for_drop.append(&BoxedAnyObject::new(video));
            }
          }
        }
      }
    }

    true
  });
  playlist_columnview.add_controller(drop_target);

  let pc_for_activate = playback_controller.clone();
  let store_for_activate = playlist_store.clone();
  let settings_for_activate = settings.clone();
  playlist_columnview.connect_activate(move |_columnview, pos| {
    if let Some(item) = store_for_activate.item(pos) {
      if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
        if let Some(playlist_item) = try_get_item(&obj) {
          match playlist_item {
            PlaylistItem::Track(_) => {
              pc_for_activate.play_index(pos);
            }
            PlaylistItem::Video(video) => {
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
  video_menu.append(Some("Open in Browser"), Some("playlist.open-browser"));
  video_menu.append(Some("Remove from Playlist"), Some("playlist.remove-video"));

  let video_popover = PopoverMenu::from_model(Some(&video_menu));
  video_popover.set_parent(&playlist_columnview);
  video_popover.set_has_arrow(false);

  let track_menu = gtk::gio::Menu::new();
  track_menu.append(Some("Open Folder"), Some("playlist.open-folder"));
  track_menu.append(Some("Remove from Playlist"), Some("playlist.remove-track"));

  let track_popover = PopoverMenu::from_model(Some(&track_menu));
  track_popover.set_parent(&playlist_columnview);
  track_popover.set_has_arrow(false);

  let current_video: Rc<RefCell<Option<Rc<YouTubeVideo>>>> = Rc::new(RefCell::new(None));
  let current_track: Rc<RefCell<Option<Rc<Track>>>> = Rc::new(RefCell::new(None));

  let action_group = gtk::gio::SimpleActionGroup::new();

  let cv = current_video.clone();
  let pc = playback_controller.clone();
  let play_audio = gtk::gio::SimpleAction::new("play-audio", None);
  play_audio.connect_activate(move |_, _| {
    if let Some(video) = cv.borrow().as_ref() {
      pc.play_youtube_video(video, true);
    }
  });
  action_group.add_action(&play_audio);

  let cv = current_video.clone();
  let pc = playback_controller.clone();
  let play_video = gtk::gio::SimpleAction::new("play-video", None);
  play_video.connect_activate(move |_, _| {
    if let Some(video) = cv.borrow().as_ref() {
      pc.play_youtube_video(video, false);
    }
  });
  action_group.add_action(&play_video);

  let cv = current_video.clone();
  let open_browser = gtk::gio::SimpleAction::new("open-browser", None);
  open_browser.connect_activate(move |_, _| {
    if let Some(video) = cv.borrow().as_ref() {
      open_in_browser(&video.video_id);
    }
  });
  action_group.add_action(&open_browser);

  let ct = current_track.clone();
  let open_folder = gtk::gio::SimpleAction::new("open-folder", None);
  open_folder.connect_activate(move |_, _| {
    if let Some(track) = ct.borrow().as_ref() {
      open_folder_in_explorer(&track.filename);
    }
  });
  action_group.add_action(&open_folder);

  let ct = current_track.clone();
  let cpid = current_playlist_id.clone();
  let store_for_remove = playlist_store.clone();
  let remove_track = gtk::gio::SimpleAction::new("remove-track", None);
  remove_track.connect_activate(move |_, _| {
    if let Some(playlist_id) = *cpid.borrow() {
      if let Some(track) = ct.borrow().as_ref() {
        if remove_track_from_playlist(playlist_id, &track.filename).is_ok() {
          for i in 0..store_for_remove.n_items() {
            if let Some(item) = store_for_remove.item(i) {
              if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
                if let Some(PlaylistItem::Track(t)) = try_get_item(&obj) {
                  if t.filename == track.filename {
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

  let cv = current_video.clone();
  let cpid = current_playlist_id.clone();
  let store_for_remove = playlist_store.clone();
  let remove_video = gtk::gio::SimpleAction::new("remove-video", None);
  remove_video.connect_activate(move |_, _| {
    if let Some(playlist_id) = *cpid.borrow() {
      if let Some(video) = cv.borrow().as_ref() {
        if remove_video_from_playlist(playlist_id, video.id).is_ok() {
          for i in 0..store_for_remove.n_items() {
            if let Some(item) = store_for_remove.item(i) {
              if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
                if let Some(PlaylistItem::Video(v)) = try_get_item(&obj) {
                  if v.id == video.id {
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

  playlist_columnview.insert_action_group("playlist", Some(&action_group));

  let gesture = GestureClick::builder().button(3).build();
  let cv = current_video.clone();
  let ct = current_track.clone();
  let video_pop = video_popover.clone();
  let track_pop = track_popover.clone();
  let store = playlist_store.clone();
  let sel_for_gesture = playlist_sel.clone();
  let colview_for_gesture = playlist_columnview.clone();
  gesture.connect_released(move |gesture, _n_press, x, y| {
    colview_for_gesture.grab_focus();

    let mut found_pos: Option<u32> = None;
    if let Some(picked) = colview_for_gesture.pick(x, y, gtk::PickFlags::DEFAULT) {
      let mut widget = Some(picked);
      while let Some(w) = widget {
        if let Some(row_y) = w.compute_point(&colview_for_gesture, &gtk::graphene::Point::new(0.0, 0.0)) {
          let height = w.height();
          if height > 0 && height < 100 {
            let widget_top = row_y.y();
            for i in 0..store.n_items() {
              let expected_top = 24.0 + (i as f32 * height as f32);
              if (widget_top - expected_top).abs() < 5.0 {
                found_pos = Some(i);
                break;
              }
            }
          }
        }
        if found_pos.is_some() {
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
        if let Some(playlist_item) = try_get_item(&obj) {
          let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
          match playlist_item {
            PlaylistItem::Video(video) => {
              *cv.borrow_mut() = Some(video);
              video_pop.set_pointing_to(Some(&rect));
              video_pop.popup();
            }
            PlaylistItem::Track(track) => {
              *ct.borrow_mut() = Some(track);
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

  ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build()
}

fn format_duration(seconds: Option<i32>) -> String {
  match seconds {
    Some(s) => {
      let mins = s / 60;
      let secs = s % 60;
      format!("{mins}:{secs:02}")
    }
    None => "?:??".to_string(),
  }
}
