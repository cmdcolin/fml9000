use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, setup_col};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::{ColumnView, ColumnViewColumn, ScrolledWindow, SignalListItemFactory, SingleSelection};
use std::cell::Ref;

struct Playlist {
  name: String,
}

pub fn create_playlist_manager(playlist_mgr_store: &ListStore) -> ScrolledWindow {
  let playlist_mgr_sel = SingleSelection::builder().model(playlist_mgr_store).build();
  let playlist_mgr_columnview = ColumnView::builder().model(&playlist_mgr_sel).build();
  let playlist_mgr = SignalListItemFactory::new();

  playlist_mgr.connect_setup(move |_factory, item| setup_col(item));
  playlist_mgr.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let r: Ref<Playlist> = obj.borrow();
    cell.set_entry(&Entry {
      name: format!("{}", r.name),
    });
  });
  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently added".to_string(),
  }));
  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently played".to_string(),
  }));

  let playlist_mgr_col = ColumnViewColumn::builder()
    .title("Playlists")
    .factory(&playlist_mgr)
    .expand(true)
    .build();

  playlist_mgr_columnview.append_column(&playlist_mgr_col);

  let playlist_mgr_wnd = ScrolledWindow::builder()
    .child(&playlist_mgr_columnview)
    .build();

  playlist_mgr_wnd
}
