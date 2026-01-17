use crate::grid_cell::Entry;
use crate::gtk_helpers::{
  get_album_artist_or_artist, get_cell, get_selection, setup_col, str_or_unknown,
};
use crate::settings::FmlSettings;
use fml9000::models::Track;
use fml9000::{load_playlist_store, Facet};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use adw::prelude::*;
use gtk::{
  ColumnView, ColumnViewColumn, CustomFilter, CustomSorter, FilterListModel, MultiSelection,
  Orientation, ScrolledWindow, SearchEntry, SignalListItemFactory, SortListModel,
};
use regex::Regex;
use std::cell::{Ref, RefCell};
use std::rc::Rc;

pub fn create_facet_box(
  playlist_store: ListStore,
  facet_store: ListStore,
  filter: CustomFilter,
  tracks: &Rc<Vec<Rc<Track>>>,
  settings: Rc<RefCell<FmlSettings>>,
) -> (gtk::Box, Rc<MultiSelection>) {
  let case_insensitive_sorter = CustomSorter::new(|obj1, obj2| {
    let k1: Ref<Facet> = obj1.downcast_ref::<BoxedAnyObject>().unwrap().borrow();
    let k2: Ref<Facet> = obj2.downcast_ref::<BoxedAnyObject>().unwrap().borrow();
    let emp = "".to_string();
    let t1 = k1.album_artist_or_artist.as_ref().unwrap_or(&emp);
    let t2 = k2.album_artist_or_artist.as_ref().unwrap_or(&emp);
    t1.to_lowercase().cmp(&t2.to_lowercase()).into()
  });
  let facet_filter = FilterListModel::new(Some(facet_store), Some(filter));
  let facet_sort = SortListModel::new(
    Some(facet_filter.clone()),
    Some(case_insensitive_sorter.clone()),
  );

  let facet_sel = MultiSelection::new(Some(facet_sort));
  let facet_columnview = ColumnView::builder().model(&facet_sel).build();

  let facet_selection = Rc::new(facet_sel);
  let facet_selection_for_handler = Rc::clone(&facet_selection);
  let facet = SignalListItemFactory::new();

  let facet_wnd = ScrolledWindow::builder()
    .child(&facet_columnview)
    .vexpand(true)
    .build();

  let facet_col = ColumnViewColumn::builder()
    .title("Album Artist / Album")
    .factory(&facet)
    .expand(true)
    .sorter(&case_insensitive_sorter)
    .build();
  facet_columnview.append_column(&facet_col);
  let playlist_store_for_handler = playlist_store.clone();
  let tracks_for_handler = Rc::clone(tracks);

  facet_selection.connect_selection_changed(move |_, _, _| {
    let bitset = facet_selection_for_handler.selection();
    if let Some((iter, first_pos)) = gtk::BitsetIter::init_first(&bitset) {
      playlist_store_for_handler.remove_all();

      // Collect all selected facets and check for "(All)"
      let mut selected_facets = Vec::new();
      let mut has_all = false;

      let item = get_selection(&facet_selection_for_handler, first_pos);
      let facet: Ref<Facet> = item.borrow();
      if facet.all {
        has_all = true;
      } else {
        selected_facets.push((facet.album_artist_or_artist.clone(), facet.album.clone()));
      }
      drop(facet);

      for pos in iter {
        let item = get_selection(&facet_selection_for_handler, pos);
        let facet: Ref<Facet> = item.borrow();
        if facet.all {
          has_all = true;
        } else {
          selected_facets.push((facet.album_artist_or_artist.clone(), facet.album.clone()));
        }
      }

      if has_all {
        load_playlist_store(tracks_for_handler.iter(), &playlist_store_for_handler);
      } else {
        let matching = tracks_for_handler.iter().filter(|t| {
          selected_facets.iter().any(|(artist, album)| {
            get_album_artist_or_artist(t) == *artist && t.album == *album
          })
        });
        load_playlist_store(matching, &playlist_store_for_handler);
      }
    }
  });

  facet.connect_setup(move |_factory, item| setup_col(item));
  let settings_for_bind = settings.clone();
  facet.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
    let row_height = settings_for_bind.borrow().row_height;
    cell.set_row_height(row_height.height_pixels(), row_height.is_compact());
    let r: Ref<Facet> = obj.borrow();
    cell.set_entry(&Entry {
      name: if r.all {
        "(All)".to_string()
      } else {
        format!(
          "{} // {}",
          str_or_unknown(&r.album_artist_or_artist),
          str_or_unknown(&r.album),
        )
      },
    });
  });

  let facet_box = gtk::Box::new(Orientation::Vertical, 0);
  let search_bar = SearchEntry::builder().build();

  search_bar.connect_search_changed(move |s| {
    let text = s.text();
    let re = Regex::new(&format!("(?i){}", regex::escape(text.as_str()))).unwrap();
    let filter = CustomFilter::new(move |obj| {
      let r = obj.downcast_ref::<BoxedAnyObject>().unwrap();
      let k: Ref<Facet> = r.borrow();
      let k0 = k.all;
      let k1 = match &k.album {
        Some(s) => re.is_match(s),
        None => false,
      };
      let k2 = match &k.album_artist_or_artist {
        Some(s) => re.is_match(s),
        None => false,
      };
      k0 || k1 || k2
    });
    facet_filter.set_filter(Some(&filter))
  });
  facet_box.append(&search_bar);
  facet_box.append(&facet_wnd);
  (facet_box, facet_selection)
}
