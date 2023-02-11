use crate::grid_cell::Entry;
use crate::gtk_helpers::{
  get_album_artist_or_artist, get_cell, get_selection, setup_col, str_or_unknown,
};
use fml9000::models::Track;
use fml9000::{load_playlist_store, Facet};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  ColumnView, ColumnViewColumn, CustomFilter, CustomSorter, FilterListModel, MultiSelection,
  Orientation, ScrolledWindow, SearchEntry, SignalListItemFactory, SortListModel,
};
use regex::Regex;
use std::cell::Ref;
use std::rc::Rc;

pub fn create_facet_box(
  playlist_store: ListStore,
  facet_store: ListStore,
  filter: CustomFilter,
  tracks: &Rc<Vec<Rc<Track>>>,
) -> gtk::Box {
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

  let facet_sel_rc = Rc::new(facet_sel);
  let facet_sel_rc1 = facet_sel_rc.clone();
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
  let playlist_store_rc1 = playlist_store.clone();

  let tracks_rc = tracks.clone();
  facet_sel_rc.connect_selection_changed(move |_, _, _| {
    let selection = facet_sel_rc1.selection();
    match gtk::BitsetIter::init_first(&selection) {
      Some(result) => {
        let (iter, first_pos) = result;
        playlist_store_rc1.remove_all();
        let item = get_selection(&facet_sel_rc1, first_pos);
        let r: Ref<Facet> = item.borrow();
        let con = tracks_rc.iter().filter(|x| {
          get_album_artist_or_artist(x) == r.album_artist_or_artist && x.album == r.album
        });

        load_playlist_store(con, &playlist_store_rc1);

        for pos in iter {
          let item = get_selection(&facet_sel_rc1, pos);
          let r: Ref<Facet> = item.borrow();
          let con = tracks_rc.iter().filter(|x| {
            get_album_artist_or_artist(x) == r.album_artist_or_artist && x.album == r.album
          });

          load_playlist_store(con, &playlist_store_rc1);
        }
      }
      None => { /* empty selection */ }
    }
  });

  facet.connect_setup(|_factory, item| setup_col(item));
  facet.connect_bind(move |_factory, item| {
    let (cell, obj) = get_cell(item);
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
        Some(s) => re.is_match(&s),
        None => false,
      };
      let k2 = match &k.album_artist_or_artist {
        Some(s) => re.is_match(&s),
        None => false,
      };
      k0 || k1 || k2
    });
    facet_filter.set_filter(Some(&filter))
  });
  facet_box.append(&search_bar);
  facet_box.append(&facet_wnd);
  facet_box
}
