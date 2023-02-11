use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, get_playlist_activate_selection, setup_col, str_or_unknown};
use fml9000::add_track_to_recently_played;
use fml9000::models::Track;
use gtk::gio::ListStore;
use gtk::{ColumnView, ColumnViewColumn, MultiSelection, ScrolledWindow, SignalListItemFactory};
use rodio::{Decoder, OutputStreamHandle, Sink};
use std::cell::{Ref, RefCell};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;

fn create_column(cb: impl Fn(Ref<Rc<Track>>) -> String + 'static) -> SignalListItemFactory {
  let col = SignalListItemFactory::new();
  col.connect_setup(move |_factory, item| setup_col(item));
  col.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let r: Ref<Rc<Track>> = obj.borrow();
    cell.set_entry(&Entry { name: cb(r) });
  });
  return col;
}

pub fn create_playlist_view(
  playlist_store: ListStore,
  sink_rc: &Rc<RefCell<Sink>>,
  stream_handle: &Rc<OutputStreamHandle>,
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

  let track = create_column(|r| format!("{}", r.track.as_ref().unwrap_or(&"".to_string())));
  let title = create_column(|r| format!("{}", r.title.as_ref().unwrap_or(&"".to_string())));
  let filename = create_column(|r| format!("{}", r.filename));

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
    .factory(&track)
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

  let stream_handle_rc1 = stream_handle.clone();
  let sink = sink_rc.clone();
  playlist_columnview.connect_activate(move |columnview, pos| {
    let selection = columnview.model().unwrap();
    let item = get_playlist_activate_selection(&selection, pos);
    let r: Ref<Rc<Track>> = item.borrow();
    let f1 = r.filename.clone();
    let f2 = r.filename.clone();
    let f3 = r.filename.clone();

    let file = BufReader::new(File::open(f1).unwrap());
    let source = Decoder::new(file).unwrap();

    let mut sink = sink.borrow_mut();
    if !sink.empty() {
      sink.stop();
    }

    // kill and recreate sink, xref
    // https://github.com/betta-cyber/netease-music-tui/pull/27/
    // https://github.com/RustAudio/rodio/issues/315
    *sink = rodio::Sink::try_new(&stream_handle_rc1).unwrap();
    sink.append(source);
    sink.play();

    add_track_to_recently_played(&f3);

    let mut p = PathBuf::from(f2);
    p.pop();
    p.push("cover.jpg");
    // album_art_rc1.set_from_file(Some(p));

    // wnd_rc1.set_title(Some(&format!(
    //   "fml9000 // {} - {} - {}",
    //   str_or_unknown(&r.artist),
    //   str_or_unknown(&r.album),
    //   str_or_unknown(&r.title),
    // )));
  });

  let playlist_wnd = ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build();

  playlist_wnd
}
