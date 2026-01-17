use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, setup_col, str_or_unknown};
use crate::playback_controller::PlaybackController;
use fml9000::models::Track;
use gtk::gio::ListStore;
use gtk::{ColumnView, ColumnViewColumn, MultiSelection, ScrolledWindow, SignalListItemFactory};
use std::cell::Ref;
use std::rc::Rc;

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
  playback_controller: Rc<PlaybackController>,
) -> ScrolledWindow {
  let playlist_sel = MultiSelection::new(Some(playlist_store));
  let playlist_columnview = ColumnView::builder().model(&playlist_sel).build();
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

  playlist_columnview.connect_activate(move |_columnview, pos| {
    playback_controller.play_index(pos);
  });

  ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build()
}
