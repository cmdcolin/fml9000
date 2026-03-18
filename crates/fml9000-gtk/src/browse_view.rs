use crate::browse_card::BrowseCard;
use crate::grid_cell::{Entry, GridCell};
use crate::playback_controller::PlaybackController;
use crate::settings::FmlSettings;
use crate::source_model::{
  build_section_children, get_distinct_album_items, populate_section_headers,
  try_get_source_from_row, SourceKind, TreeEntry,
};
use fml9000_core::thumbnail_cache;
use fml9000_core::MediaItem;
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  ColumnView, ColumnViewColumn, MultiSelection, ScrolledWindow, SignalListItemFactory,
  TreeExpander, TreeListModel, TreeListRow,
};
use std::cell::{Ref, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
enum BrowseItem {
  Media(MediaItem),
  Album {
    artist: String,
    album: String,
    representative_filename: String,
  },
}

fn collect_items_for_source(source: &SourceKind) -> Vec<BrowseItem> {
  if *source == SourceKind::Albums {
    return get_distinct_album_items()
      .into_iter()
      .map(|(artist, album, filename)| BrowseItem::Album {
        artist,
        album,
        representative_filename: filename,
      })
      .collect();
  }
  source.load_items().into_iter().map(BrowseItem::Media).collect()
}

fn browse_item_cache_key(item: &BrowseItem) -> Option<String> {
  match item {
    BrowseItem::Media(media_item) => {
      if let Some(url) = media_item.thumbnail_url() {
        Some(url)
      } else {
        media_item.track_filename().map(|s| s.to_string())
      }
    }
    BrowseItem::Album { representative_filename, .. } => {
      Some(representative_filename.clone())
    }
  }
}

fn item_matches_search(item: &BrowseItem, query: &str) -> bool {
  if query.is_empty() {
    return true;
  }
  let q = query.to_lowercase();
  match item {
    BrowseItem::Media(media_item) => {
      media_item.title().to_lowercase().contains(&q)
        || media_item.artist().to_lowercase().contains(&q)
        || media_item.album().to_lowercase().contains(&q)
    }
    BrowseItem::Album { artist, album, .. } => {
      artist.to_lowercase().contains(&q) || album.to_lowercase().contains(&q)
    }
  }
}

enum FetchKind {
  Url,
  AlbumArt,
}

fn spawn_fetch(
  pending: &Arc<Mutex<HashSet<String>>>,
  tx: &Arc<Mutex<std::sync::mpsc::Sender<String>>>,
  key: String,
  kind: FetchKind,
) {
  if let Ok(mut set) = pending.lock() {
    if !set.insert(key.clone()) {
      return;
    }
  }
  let tx = tx.clone();
  std::thread::spawn(move || {
    let result = match kind {
      FetchKind::Url => thumbnail_cache::fetch_and_cache_url(&key),
      FetchKind::AlbumArt => thumbnail_cache::extract_and_cache_album_art(&key),
    };
    if result.is_some() {
      if let Ok(tx) = tx.lock() {
        let _ = tx.send(key);
      }
    }
  });
}

fn reload_grid(grid_store: &ListStore, sources: &[SourceKind], search_query: &str) {
  grid_store.remove_all();

  let mut items: Vec<BrowseItem> = Vec::new();
  for source in sources {
    for item in collect_items_for_source(source) {
      if item_matches_search(&item, search_query) {
        items.push(item);
      }
    }
  }

  for item in &items {
    grid_store.append(&BoxedAnyObject::new(item.clone()));
  }
}

fn get_selected_sources(tree_model: &TreeListModel, selection: &MultiSelection) -> Vec<SourceKind> {
  let mut selected = Vec::new();
  let sel = selection.selection();
  for i in 0..tree_model.n_items() {
    if sel.contains(i) {
      if let Some(item) = tree_model.item(i) {
        if let Some(row) = item.downcast_ref::<TreeListRow>() {
          if let Some(source) = try_get_source_from_row(row) {
            selected.push(source);
          }
        }
      }
    }
  }
  selected
}

pub fn create_browse_view(
  playback_controller: Rc<PlaybackController>,
  settings: Rc<RefCell<FmlSettings>>,
) -> gtk::Box {
  let grid_store = ListStore::new::<BoxedAnyObject>();
  let search_query: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

  let (thumb_tx, thumb_rx) = std::sync::mpsc::channel::<String>();
  let thumb_tx = Arc::new(Mutex::new(thumb_tx));
  let pending_thumbs: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

  let grid_store_for_thumb = grid_store.clone();
  let pending_for_poll = pending_thumbs.clone();
  gtk::glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
    let mut keys: Vec<String> = Vec::new();
    while let Ok(key) = thumb_rx.try_recv() {
      keys.push(key);
    }
    if !keys.is_empty() {
      if let Ok(mut pending) = pending_for_poll.lock() {
        for key in &keys {
          pending.remove(key);
        }
      }
      let n = grid_store_for_thumb.n_items();
      for i in 0..n {
        if let Some(obj) = grid_store_for_thumb.item(i) {
          if let Ok(boxed) = obj.downcast::<BoxedAnyObject>() {
            if let Ok(item) = boxed.try_borrow::<BrowseItem>() {
              if let Some(key) = browse_item_cache_key(&item) {
                if keys.contains(&key) {
                  let cloned = item.clone();
                  drop(item);
                  grid_store_for_thumb.remove(i);
                  grid_store_for_thumb.insert(i, &BoxedAnyObject::new(cloned));
                }
              }
            }
          }
        }
      }
    }
    gtk::glib::ControlFlow::Continue
  });

  // Source tree sidebar
  let source_store = ListStore::new::<BoxedAnyObject>();
  populate_section_headers(&source_store);

  let tree_model = TreeListModel::new(source_store, false, true, |item| {
    let obj = item.downcast_ref::<BoxedAnyObject>()?;
    let entry: Ref<TreeEntry> = obj.borrow();
    match &*entry {
      TreeEntry::SectionHeader(_, kind) => Some(build_section_children(kind).into()),
      TreeEntry::Source(_) => None,
    }
  });

  let source_selection = MultiSelection::new(Some(tree_model.clone()));
  let source_columnview = ColumnView::builder().model(&source_selection).build();

  let source_factory = SignalListItemFactory::new();
  source_factory.connect_setup(|_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let cell = GridCell::new();
    let expander = TreeExpander::new();
    expander.set_child(Some(&cell));
    list_item.set_child(Some(&expander));
  });

  let settings_for_bind = settings.clone();
  source_factory.connect_bind(move |_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let expander = list_item.child().unwrap().downcast::<TreeExpander>().unwrap();
    let row = list_item.item().unwrap().downcast::<TreeListRow>().unwrap();
    expander.set_list_row(Some(&row));

    let cell = expander.child().unwrap().downcast::<GridCell>().unwrap();
    let row_height = settings_for_bind.borrow().row_height;
    cell.set_row_height(row_height.height_pixels(), row_height.is_compact());

    let obj = row.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let entry: Ref<TreeEntry> = obj.borrow();
    match &*entry {
      TreeEntry::SectionHeader(name, _) => {
        cell.set_entry(&Entry { name: name.clone() });
        cell.add_css_class("section-header");
      }
      TreeEntry::Source(source) => {
        cell.set_entry(&Entry { name: source.label() });
        cell.remove_css_class("section-header");
      }
    }
  });

  source_factory.connect_unbind(|_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let expander = list_item.child().unwrap().downcast::<TreeExpander>().unwrap();
    expander.set_list_row(None::<&TreeListRow>);
  });

  let source_column = ColumnViewColumn::builder()
    .title("Sources")
    .factory(&source_factory)
    .expand(true)
    .build();
  source_columnview.append_column(&source_column);

  let grid_store_for_sel = grid_store.clone();
  let tree_model_for_sel = tree_model.clone();
  let search_for_sel = Rc::clone(&search_query);
  source_selection.connect_selection_changed(move |sel, _, _| {
    let selected = get_selected_sources(&tree_model_for_sel, sel);
    reload_grid(&grid_store_for_sel, &selected, &search_for_sel.borrow());
  });

  // Grid view
  let grid_factory = SignalListItemFactory::new();
  grid_factory.connect_setup(|_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let card = BrowseCard::new();
    card.add_css_class("browse-card");
    list_item.set_child(Some(&card));
  });

  let thumb_tx_for_bind = thumb_tx.clone();
  let pending_for_bind = pending_thumbs.clone();
  grid_factory.connect_bind(move |_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let card = list_item.child().unwrap().downcast::<BrowseCard>().unwrap();
    let obj = list_item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let browse_item: Ref<BrowseItem> = obj.borrow();

    match &*browse_item {
      BrowseItem::Media(media_item) => {
        card.set_title(media_item.title());
        card.set_subtitle(media_item.artist());
        if let Some(url) = media_item.thumbnail_url() {
          if let Some(cached) = thumbnail_cache::get_cached_path(&url) {
            card.set_thumbnail_from_file(&cached);
          } else {
            card.clear_thumbnail();
            spawn_fetch(&pending_for_bind, &thumb_tx_for_bind, url.clone(), FetchKind::Url);
          }
        } else if let Some(filename) = media_item.track_filename() {
          if let Some(cached) = thumbnail_cache::get_cached_path(filename) {
            card.set_thumbnail_from_file(&cached);
          } else {
            card.clear_thumbnail();
            spawn_fetch(&pending_for_bind, &thumb_tx_for_bind, filename.to_string(), FetchKind::AlbumArt);
          }
        }
      }
      BrowseItem::Album {
        artist,
        album,
        representative_filename,
      } => {
        card.set_title(album);
        card.set_subtitle(artist);
        if let Some(cached) = thumbnail_cache::get_cached_path(representative_filename) {
          card.set_thumbnail_from_file(&cached);
        } else {
          card.clear_thumbnail();
          spawn_fetch(&pending_for_bind, &thumb_tx_for_bind, representative_filename.clone(), FetchKind::AlbumArt);
        }
      }
    }
  });

  grid_factory.connect_unbind(|_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let card = list_item.child().unwrap().downcast::<BrowseCard>().unwrap();
    card.set_title("");
    card.set_subtitle("");
    card.clear_thumbnail();
  });

  let grid_selection = gtk::SingleSelection::new(Some(grid_store.clone()));
  let grid_view = gtk::GridView::builder()
    .model(&grid_selection)
    .factory(&grid_factory)
    .min_columns(2)
    .max_columns(20)
    .build();

  let grid_store_for_activate = grid_store.clone();
  let pc_for_activate = Rc::clone(&playback_controller);
  grid_view.connect_activate(move |_view, pos| {
    if let Some(obj) = grid_store_for_activate.item(pos) {
      if let Ok(boxed) = obj.downcast::<BoxedAnyObject>() {
        let browse_item: Ref<BrowseItem> = boxed.borrow();
        match &*browse_item {
          BrowseItem::Media(item) => {
            pc_for_activate.play_media_item(item);
          }
          BrowseItem::Album {
            representative_filename,
            ..
          } => {
            if let Some(track) = fml9000_core::load_track_by_filename(representative_filename) {
              pc_for_activate.play_media_item(&MediaItem::Track(track));
            }
          }
        }
      }
    }
  });

  // Download thumbnails button
  let download_btn = gtk::Button::builder()
    .icon_name("folder-download-symbolic")
    .tooltip_text("Download all thumbnails")
    .css_classes(["flat"])
    .build();

  let pc_for_download = Rc::clone(&playback_controller);
  let search_for_download = Rc::clone(&search_query);
  let grid_store_for_refresh = grid_store.clone();
  let tree_model_for_refresh = tree_model.clone();
  let source_sel_for_refresh = source_selection.clone();
  download_btn.connect_clicked(move |btn| {
    btn.set_sensitive(false);

    let progress_dialog = gtk::Window::builder()
      .title("Downloading Thumbnails")
      .modal(true)
      .transient_for(&**pc_for_download.window())
      .default_width(400)
      .default_height(120)
      .resizable(false)
      .deletable(false)
      .build();

    let content = gtk::Box::builder()
      .orientation(gtk::Orientation::Vertical)
      .spacing(12)
      .margin_top(24)
      .margin_bottom(24)
      .margin_start(24)
      .margin_end(24)
      .build();

    let status_label = gtk::Label::builder()
      .label("Downloading video thumbnails...")
      .xalign(0.0)
      .build();

    let progress_bar = gtk::ProgressBar::builder()
      .show_text(true)
      .build();

    content.append(&status_label);
    content.append(&progress_bar);
    progress_dialog.set_child(Some(&content));
    progress_dialog.present();

    #[derive(Debug)]
    enum ThumbnailProgress {
      VideoProgress(usize, usize),
      AlbumProgress(usize, usize),
      Done(usize, usize),
    }

    let (tx, rx) = std::sync::mpsc::channel::<ThumbnailProgress>();
    std::thread::spawn(move || {
      let tx_video = tx.clone();
      let (video_dl, _) = thumbnail_cache::download_all_video_thumbnails(move |done, total| {
        let _ = tx_video.send(ThumbnailProgress::VideoProgress(done, total));
      });

      let tx_album = tx.clone();
      let (album_dl, _) = thumbnail_cache::download_all_album_art(move |done, total| {
        let _ = tx_album.send(ThumbnailProgress::AlbumProgress(done, total));
      });

      let _ = tx.send(ThumbnailProgress::Done(video_dl, album_dl));
    });

    let btn_clone = btn.clone();
    let dialog_clone = progress_dialog.clone();
    let status_clone = status_label.clone();
    let progress_clone = progress_bar.clone();
    let grid_store_done = grid_store_for_refresh.clone();
    let tree_model_done = tree_model_for_refresh.clone();
    let source_sel_done = source_sel_for_refresh.clone();
    let search_done = Rc::clone(&search_for_download);
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
      while let Ok(progress) = rx.try_recv() {
        match progress {
          ThumbnailProgress::VideoProgress(done, total) => {
            if total > 0 {
              progress_clone.set_fraction(done as f64 / total as f64 * 0.5);
              progress_clone.set_text(Some(&format!("Videos: {done}/{total}")));
            }
          }
          ThumbnailProgress::AlbumProgress(done, total) => {
            if total > 0 {
              progress_clone.set_fraction(0.5 + done as f64 / total as f64 * 0.5);
              progress_clone.set_text(Some(&format!("Albums: {done}/{total}")));
            }
          }
          ThumbnailProgress::Done(videos, albums) => {
            status_clone.set_label(&format!("Done: {videos} video + {albums} album thumbnails downloaded"));
            progress_clone.set_fraction(1.0);
            progress_clone.set_text(Some("Complete"));

            let selected = get_selected_sources(&tree_model_done, &source_sel_done);
            reload_grid(&grid_store_done, &selected, &search_done.borrow());

            let dialog = dialog_clone.clone();
            let btn = btn_clone.clone();
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
              dialog.close();
              btn.set_sensitive(true);
            });
            return gtk::glib::ControlFlow::Break;
          }
        }
      }
      gtk::glib::ControlFlow::Continue
    });
  });

  // Search bar
  let search_entry = gtk::SearchEntry::builder()
    .placeholder_text("Filter...")
    .hexpand(true)
    .build();

  let grid_store_for_search = grid_store.clone();
  let tree_model_for_search = tree_model.clone();
  let source_sel_for_search = source_selection.clone();
  let search_for_entry = Rc::clone(&search_query);
  search_entry.connect_search_changed(move |entry| {
    let query = entry.text().to_string();
    *search_for_entry.borrow_mut() = query.clone();
    let selected = get_selected_sources(&tree_model_for_search, &source_sel_for_search);
    reload_grid(&grid_store_for_search, &selected, &query);
  });

  // Layout
  let sidebar_header = gtk::Box::builder()
    .orientation(gtk::Orientation::Horizontal)
    .build();
  sidebar_header.append(
    &gtk::Label::builder()
      .label("Sources")
      .hexpand(true)
      .xalign(0.0)
      .build(),
  );
  sidebar_header.append(&download_btn);

  let sidebar_scrolled = ScrolledWindow::builder()
    .vexpand(true)
    .child(&source_columnview)
    .build();

  let sidebar = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .width_request(200)
    .spacing(4)
    .build();
  sidebar.append(&sidebar_header);
  sidebar.append(&sidebar_scrolled);

  let grid_scrolled = ScrolledWindow::builder()
    .hexpand(true)
    .vexpand(true)
    .child(&grid_view)
    .build();

  let grid_container = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .hexpand(true)
    .vexpand(true)
    .build();
  grid_container.append(&search_entry);
  grid_container.append(&grid_scrolled);

  let paned = gtk::Paned::builder()
    .orientation(gtk::Orientation::Horizontal)
    .start_child(&sidebar)
    .end_child(&grid_container)
    .build();
  paned.set_position(200);

  let main_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
  main_box.set_hexpand(true);
  main_box.set_vexpand(true);
  main_box.append(&paned);
  main_box
}
