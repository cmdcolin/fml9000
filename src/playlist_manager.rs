use crate::grid_cell::Entry;
use crate::gtk_helpers::{get_cell, setup_col};
use adw::prelude::*;
use fml9000::models::Track;
use fml9000::{load_playlist_store, load_recently_played};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::{ColumnView, ColumnViewColumn, ScrolledWindow, SignalListItemFactory, SingleSelection};
use std::cell::Ref;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
enum PlaylistType {
  RecentlyAdded,
  RecentlyPlayed,
}

struct Playlist {
  name: String,
  playlist_type: PlaylistType,
}

pub fn create_playlist_manager(
  playlist_mgr_store: &ListStore,
  main_playlist_store: ListStore,
  all_tracks: Rc<Vec<Rc<Track>>>,
) -> ScrolledWindow {
  let selection = SingleSelection::builder().model(playlist_mgr_store).build();
  let columnview = ColumnView::builder().model(&selection).build();
  let factory = SignalListItemFactory::new();

  factory.connect_setup(move |_factory, item| setup_col(item));
  factory.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let playlist: Ref<Playlist> = obj.borrow();
    cell.set_entry(&Entry {
      name: playlist.name.clone(),
    });
  });

  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently added".to_string(),
    playlist_type: PlaylistType::RecentlyAdded,
  }));
  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently played".to_string(),
    playlist_type: PlaylistType::RecentlyPlayed,
  }));

  let column = ColumnViewColumn::builder()
    .title("Playlists")
    .factory(&factory)
    .expand(true)
    .build();

  columnview.append_column(&column);

  selection.connect_selection_changed(move |sel, _, _| {
    if let Some(item) = sel.selected_item() {
      let obj = item.downcast::<BoxedAnyObject>().unwrap();
      let playlist: Ref<Playlist> = obj.borrow();

      main_playlist_store.remove_all();

      match playlist.playlist_type {
        PlaylistType::RecentlyAdded => {
          load_playlist_store(all_tracks.iter(), &main_playlist_store);
        }
        PlaylistType::RecentlyPlayed => {
          let recent = load_recently_played(100);
          load_playlist_store(recent.iter(), &main_playlist_store);
        }
      }
    }
  });

  ScrolledWindow::builder().child(&columnview).build()
}
