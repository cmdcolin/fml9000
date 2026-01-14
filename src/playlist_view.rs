use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, get_playlist_activate_selection, setup_col, str_or_unknown};
use crate::AudioPlayer;
use adw::prelude::*;
use fml9000::add_track_to_recently_played;
use fml9000::models::Track;
use gtk::gio::ListStore;
use gtk::{
  AlertDialog, ApplicationWindow, ColumnView, ColumnViewColumn, Image, MultiSelection,
  ScrolledWindow, SignalListItemFactory,
};
use rodio::Decoder;
use std::cell::Ref;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;

fn show_error_dialog(window: &ApplicationWindow, title: &str, message: &str) {
  let dialog = AlertDialog::builder()
    .modal(true)
    .message(title)
    .detail(message)
    .buttons(["OK"])
    .build();
  dialog.show(Some(window));
}

fn create_column(cb: impl Fn(Ref<Rc<Track>>) -> String + 'static) -> SignalListItemFactory {
  let factory = SignalListItemFactory::new();
  factory.connect_setup(move |_factory, item| setup_col(item));
  factory.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let track: Ref<Rc<Track>> = obj.borrow();
    cell.set_entry(&Entry { name: cb(track) });
  });
  factory
}

pub fn create_playlist_view(
  playlist_store: ListStore,
  audio: AudioPlayer,
  album_art: &Rc<Image>,
  wnd_rc: &Rc<ApplicationWindow>,
) -> ScrolledWindow {
  let playlist_sel = MultiSelection::new(Some(playlist_store));
  let playlist_columnview = ColumnView::builder().model(&playlist_sel).build();
  let album_art_rc = album_art.clone();
  let artistalbum = create_column(|r| {
    format!(
      "{} // {}",
      str_or_unknown(&r.album),
      str_or_unknown(&r.artist),
    )
  });

  let track_num = create_column(|r| r.track.clone().unwrap_or_default());
  let title = create_column(|r| r.title.clone().unwrap_or_default());
  let filename = create_column(|r| r.filename.clone());

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

  let window = Rc::clone(wnd_rc);

  playlist_columnview.connect_activate(move |columnview, pos| {
    let selection = columnview.model().unwrap();
    let item = get_playlist_activate_selection(&selection, pos);
    let track: Ref<Rc<Track>> = item.borrow();
    let filename = &track.filename;

    if !audio.is_available() {
      show_error_dialog(&window, "No Audio", "Audio playback is not available.");
      return;
    }

    let file = match File::open(filename) {
      Ok(f) => BufReader::new(f),
      Err(e) => {
        show_error_dialog(
          &window,
          "Cannot open file",
          &format!("Failed to open '{filename}':\n{e}"),
        );
        return;
      }
    };

    let source = match Decoder::new(file) {
      Ok(s) => s,
      Err(e) => {
        show_error_dialog(
          &window,
          "Cannot decode file",
          &format!("Failed to decode '{filename}':\n{e}"),
        );
        return;
      }
    };

    audio.play_source(source);
    add_track_to_recently_played(filename);

    let mut cover_path = PathBuf::from(filename);
    cover_path.pop();
    cover_path.push("cover.jpg");
    album_art_rc.set_from_file(Some(cover_path));

    window.set_title(Some(&format!(
      "fml9000 // {} - {} - {}",
      str_or_unknown(&track.artist),
      str_or_unknown(&track.album),
      str_or_unknown(&track.title),
    )));
  });

  ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build()
}
