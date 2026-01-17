use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, setup_col, str_or_unknown};
use crate::mpv_player::open_in_browser;
use crate::playback_controller::PlaybackController;
use crate::settings::FmlSettings;
use fml9000::models::{Track, YouTubeVideo};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  ColumnView, ColumnViewColumn, GestureClick, MultiSelection, PopoverMenu, ScrolledWindow,
  SignalListItemFactory,
};
use std::cell::RefCell;
use std::rc::Rc;

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

fn create_column(cb: impl Fn(&PlaylistItem) -> String + 'static) -> SignalListItemFactory {
  let factory = SignalListItemFactory::new();
  factory.connect_setup(move |_factory, item| setup_col(item));
  factory.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    if let Some(playlist_item) = try_get_item(&obj) {
      cell.set_entry(&Entry {
        name: cb(&playlist_item),
      });
    }
  });
  factory
}

pub fn create_playlist_view(
  playlist_store: ListStore,
  playback_controller: Rc<PlaybackController>,
  settings: Rc<RefCell<FmlSettings>>,
) -> ScrolledWindow {
  let playlist_sel = MultiSelection::new(Some(playlist_store.clone()));
  let playlist_columnview = ColumnView::builder().model(&playlist_sel).build();

  let artistalbum = create_column(|item| match item {
    PlaylistItem::Track(r) => {
      format!(
        "{} // {}",
        str_or_unknown(&r.album),
        str_or_unknown(&r.artist),
      )
    }
    PlaylistItem::Video(v) => format!("YouTube // {}", format_duration(v.duration_seconds)),
  });

  let track_num = create_column(|item| match item {
    PlaylistItem::Track(r) => r.track.clone().unwrap_or_default(),
    PlaylistItem::Video(_) => String::new(),
  });

  let title = create_column(|item| match item {
    PlaylistItem::Track(r) => r.title.clone().unwrap_or_default(),
    PlaylistItem::Video(v) => v.title.clone(),
  });

  let filename = create_column(|item| match item {
    PlaylistItem::Track(r) => r.filename.clone(),
    PlaylistItem::Video(v) => v.video_id.clone(),
  });

  let playlist_col1 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(400)
    .title("Album / Artist")
    .factory(&artistalbum)
    .build();

  let playlist_col2 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("#")
    .fixed_width(20)
    .factory(&track_num)
    .build();

  let playlist_col3 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("Title")
    .fixed_width(300)
    .factory(&title)
    .build();

  let playlist_col4 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(2000)
    .title("Filename")
    .factory(&filename)
    .build();

  playlist_columnview.append_column(&playlist_col1);
  playlist_columnview.append_column(&playlist_col2);
  playlist_columnview.append_column(&playlist_col3);
  playlist_columnview.append_column(&playlist_col4);

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

  let menu = gtk::gio::Menu::new();
  menu.append(Some("Play (Audio)"), Some("playlist.play-audio"));
  menu.append(Some("Play (Video)"), Some("playlist.play-video"));
  menu.append(Some("Open in Browser"), Some("playlist.open-browser"));

  let popover = PopoverMenu::from_model(Some(&menu));
  popover.set_parent(&playlist_columnview);
  popover.set_has_arrow(false);

  let current_video: Rc<RefCell<Option<Rc<YouTubeVideo>>>> = Rc::new(RefCell::new(None));

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

  playlist_columnview.insert_action_group("playlist", Some(&action_group));

  let gesture = GestureClick::builder().button(3).build();
  let cv = current_video.clone();
  let pop = popover.clone();
  let store = playlist_store.clone();
  let sel = playlist_sel.clone();
  gesture.connect_released(move |gesture, _n_press, x, y| {
    let selection = sel.selection();
    if selection.is_empty() {
      return;
    }
    let pos = selection.minimum();

    if let Some(item) = store.item(pos) {
      if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
        if let Some(PlaylistItem::Video(video)) = try_get_item(&obj) {
          *cv.borrow_mut() = Some(video);
          let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
          pop.set_pointing_to(Some(&rect));
          pop.popup();
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
