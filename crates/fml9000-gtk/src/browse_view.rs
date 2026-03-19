use crate::browse_card::BrowseCard;
use crate::grid_cell::{Entry, GridCell};
use crate::playback_controller::{PlayState, PlaybackController};
use crate::settings::FmlSettings;
use crate::source_model::{
  build_section_children, get_distinct_album_items, populate_section_headers,
  try_get_source_from_row, SourceKind, TreeEntry,
};
use crate::youtube_add_dialog;
use fml9000_core::{format_duration_secs, get_channel_name_map, load_tracks_by_album, thumbnail_cache};
use fml9000_core::MediaItem;
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  ColumnView, ColumnViewColumn, ContentFit, MultiSelection, Orientation, Picture, ScrolledWindow,
  SignalListItemFactory, Stack, TreeExpander, TreeListModel, TreeListRow,
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

fn collect_album_items() -> Vec<BrowseItem> {
  get_distinct_album_items()
    .into_iter()
    .map(|(artist, album, filename)| BrowseItem::Album {
      artist,
      album,
      representative_filename: filename,
    })
    .collect()
}

fn collect_items_for_source(source: &SourceKind) -> Vec<BrowseItem> {
  match source {
    SourceKind::AllTracks => collect_album_items(),
    SourceKind::AllMedia => {
      let mut items = collect_album_items();
      for v in fml9000_core::get_all_videos().unwrap_or_default() {
        items.push(BrowseItem::Media(MediaItem::Video(v)));
      }
      items
    }
    _ => {
      source.load_items().into_iter().map(BrowseItem::Media).collect()
    }
  }
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

fn browse_item_play_key(item: &BrowseItem) -> Option<String> {
  match item {
    BrowseItem::Media(media_item) => {
      if let Some(filename) = media_item.track_filename() {
        Some(filename.to_string())
      } else {
        media_item.youtube_video_id().map(|s| s.to_string())
      }
    }
    BrowseItem::Album { .. } => None,
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
  tx: &futures::channel::mpsc::UnboundedSender<String>,
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
      let _ = tx.unbounded_send(key);
    }
  });
}

fn rebind_matching_items(grid_store: &ListStore, keys: &[String]) {
  let n = grid_store.n_items();
  for i in 0..n {
    if let Some(obj) = grid_store.item(i) {
      if let Ok(boxed) = obj.downcast::<BoxedAnyObject>() {
        if let Ok(bi) = boxed.try_borrow::<BrowseItem>() {
          if let Some(k) = browse_item_play_key(&bi) {
            if keys.contains(&k) {
              let cloned = bi.clone();
              drop(bi);
              let replacement: [BoxedAnyObject; 1] = [BoxedAnyObject::new(cloned)];
              grid_store.splice(i, 1, &replacement);
            }
          }
        }
      }
    }
  }
}

const PAGE_SIZE: usize = 200;

fn show_page(grid_store: &ListStore, all_items: &[BrowseItem], show_more_btn: &gtk::Button) {
  let start = grid_store.n_items() as usize;
  let end = (start + PAGE_SIZE).min(all_items.len());
  let page: Vec<BoxedAnyObject> = all_items[start..end]
    .iter()
    .map(|item| BoxedAnyObject::new(item.clone()))
    .collect();
  let pos = grid_store.n_items();
  grid_store.splice(pos, 0, &page);

  let remaining = all_items.len() - end;
  if remaining > 0 {
    show_more_btn.set_label(&format!("Show More ({remaining} remaining)"));
    show_more_btn.set_visible(true);
  } else {
    show_more_btn.set_visible(false);
  }
}

fn reload_grid(
  grid_store: &ListStore,
  sources: &[SourceKind],
  search_query: &str,
  all_items_buf: &Rc<RefCell<Vec<BrowseItem>>>,
  show_more_btn: &gtk::Button,
) {
  grid_store.splice(0, grid_store.n_items(), &[] as &[BoxedAnyObject]);
  show_more_btn.set_visible(false);

  let sources = sources.to_vec();
  let query = search_query.to_string();
  let grid_store = grid_store.clone();
  let buf = Rc::clone(all_items_buf);
  let btn = show_more_btn.clone();

  let (tx, rx) = futures::channel::oneshot::channel::<Vec<BrowseItem>>();
  std::thread::spawn(move || {
    let mut items: Vec<BrowseItem> = Vec::new();
    for source in &sources {
      for item in collect_items_for_source(source) {
        if item_matches_search(&item, &query) {
          items.push(item);
        }
      }
    }
    let _ = tx.send(items);
  });

  gtk::glib::spawn_future_local(async move {
    if let Ok(items) = rx.await {
      *buf.borrow_mut() = items;
      show_page(&grid_store, &buf.borrow(), &btn);
    }
  });
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

fn build_album_detail(
  artist: &str,
  album: &str,
  representative_filename: &str,
  playback_controller: &Rc<PlaybackController>,
  content_stack: &Stack,
) -> gtk::Box {
  let tracks = load_tracks_by_album(artist, album);

  let detail = gtk::Box::builder()
    .orientation(Orientation::Vertical)
    .spacing(0)
    .hexpand(true)
    .vexpand(true)
    .build();

  // Header with back button
  let header = gtk::Box::builder()
    .orientation(Orientation::Horizontal)
    .spacing(8)
    .margin_start(8)
    .margin_end(8)
    .margin_top(4)
    .margin_bottom(4)
    .build();

  let back_btn = gtk::Button::builder()
    .icon_name("go-previous-symbolic")
    .tooltip_text("Back to browse")
    .css_classes(["flat"])
    .build();

  let stack_for_back = content_stack.clone();
  back_btn.connect_clicked(move |_| {
    stack_for_back.set_visible_child_name("grid");
  });

  header.append(&back_btn);
  header.append(
    &gtk::Label::builder()
      .label(&format!("{} — {}", artist, album))
      .hexpand(true)
      .xalign(0.0)
      .ellipsize(gtk::pango::EllipsizeMode::End)
      .css_classes(["heading"])
      .build(),
  );
  detail.append(&header);
  detail.append(&gtk::Separator::new(Orientation::Horizontal));

  // Album info row: art + track list
  let info_row = gtk::Box::builder()
    .orientation(Orientation::Horizontal)
    .spacing(16)
    .margin_start(16)
    .margin_end(16)
    .margin_top(12)
    .vexpand(true)
    .build();

  // Album art
  let art = Picture::builder()
    .width_request(250)
    .height_request(250)
    .content_fit(ContentFit::Contain)
    .valign(gtk::Align::Start)
    .build();

  if let Some(cached) = thumbnail_cache::get_cached_path(representative_filename) {
    art.set_filename(Some(cached));
  } else {
    // Try to extract inline
    if let Some(cached) = thumbnail_cache::extract_and_cache_album_art(representative_filename) {
      art.set_filename(Some(cached));
    }
  }

  let art_and_buttons = gtk::Box::builder()
    .orientation(Orientation::Vertical)
    .spacing(8)
    .build();
  art_and_buttons.append(&art);

  // Action buttons
  let button_box = gtk::Box::builder()
    .orientation(Orientation::Horizontal)
    .spacing(8)
    .halign(gtk::Align::Center)
    .build();

  let play_all_btn = gtk::Button::builder()
    .label("Play All")
    .css_classes(["suggested-action"])
    .build();

  let queue_all_btn = gtk::Button::builder()
    .label("Queue All")
    .build();

  let tracks_for_play = tracks.clone();
  let pc_for_play = Rc::clone(playback_controller);
  play_all_btn.connect_clicked(move |_| {
    if let Some(first) = tracks_for_play.first() {
      pc_for_play.play_media_item(&MediaItem::Track(first.clone()));
    }
  });

  let tracks_for_queue = tracks.clone();
  let pc_for_queue = Rc::clone(playback_controller);
  queue_all_btn.connect_clicked(move |_| {
    for track in &tracks_for_queue {
      pc_for_queue.queue_item(&MediaItem::Track(track.clone()));
    }
  });

  button_box.append(&play_all_btn);
  button_box.append(&queue_all_btn);
  art_and_buttons.append(&button_box);

  info_row.append(&art_and_buttons);

  // Track list
  let track_list = gtk::Box::builder()
    .orientation(Orientation::Vertical)
    .spacing(2)
    .hexpand(true)
    .valign(gtk::Align::Start)
    .build();

  for track in &tracks {
    let row = gtk::Box::builder()
      .orientation(Orientation::Horizontal)
      .spacing(8)
      .css_classes(["browse-track-row"])
      .build();

    let track_num = track.track.as_deref().unwrap_or("");
    let title = track.title.as_deref().unwrap_or("Unknown");
    let duration = track
      .duration_seconds
      .map(format_duration_secs)
      .unwrap_or_default();

    if !track_num.is_empty() {
      row.append(
        &gtk::Label::builder()
          .label(track_num)
          .width_chars(3)
          .xalign(1.0)
          .css_classes(["dim-label", "monospace"])
          .build(),
      );
    }

    row.append(
      &gtk::Label::builder()
        .label(title)
        .hexpand(true)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build(),
    );

    row.append(
      &gtk::Label::builder()
        .label(&duration)
        .css_classes(["dim-label", "monospace"])
        .build(),
    );

    let track_clone = track.clone();
    let pc = Rc::clone(playback_controller);
    let gesture = gtk::GestureClick::new();
    gesture.connect_released(move |_, _, _, _| {
      pc.play_media_item(&MediaItem::Track(track_clone.clone()));
    });
    row.add_controller(gesture);

    track_list.append(&row);
  }

  let track_scrolled = ScrolledWindow::builder()
    .hexpand(true)
    .vexpand(true)
    .child(&track_list)
    .build();
  info_row.append(&track_scrolled);

  detail.append(&info_row);
  detail
}

pub fn create_browse_view(
  playback_controller: Rc<PlaybackController>,
  settings: Rc<RefCell<FmlSettings>>,
) -> gtk::Box {
  let grid_store = ListStore::new::<BoxedAnyObject>();
  let search_query: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
  let all_items_buf: Rc<RefCell<Vec<BrowseItem>>> = Rc::new(RefCell::new(Vec::new()));

  let show_more_btn = gtk::Button::builder()
    .css_classes(["flat"])
    .visible(false)
    .build();

  let grid_store_for_more = grid_store.clone();
  let buf_for_more = Rc::clone(&all_items_buf);
  let btn_for_more = show_more_btn.clone();
  show_more_btn.connect_clicked(move |_| {
    show_page(&grid_store_for_more, &buf_for_more.borrow(), &btn_for_more);
  });

  let content_stack = Stack::new();
  content_stack.set_hexpand(true);
  content_stack.set_vexpand(true);

  let (thumb_tx, thumb_rx) = futures::channel::mpsc::unbounded::<String>();
  let pending_thumbs: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

  let grid_store_for_thumb = grid_store.clone();
  let pending_for_recv = pending_thumbs.clone();
  gtk::glib::spawn_future_local(async move {
    use futures::stream::StreamExt;
    let mut thumb_rx = thumb_rx;
    while let Some(key) = thumb_rx.next().await {
      if let Ok(mut pending) = pending_for_recv.lock() {
        pending.remove::<String>(&key);
      }
      let n = grid_store_for_thumb.n_items();
      for i in 0..n {
        if let Some(obj) = grid_store_for_thumb.item(i) {
          if let Ok(boxed) = obj.downcast::<BoxedAnyObject>() {
            if let Ok(item) = boxed.try_borrow::<BrowseItem>() {
              if browse_item_cache_key(&item) == Some(key.clone()) {
                let cloned = item.clone();
                drop(item);
                let replacement: [BoxedAnyObject; 1] = [BoxedAnyObject::new(cloned)];
                grid_store_for_thumb.splice(i, 1, &replacement);
              }
            }
          }
        }
      }
    }
  });

  // Source tree sidebar
  let source_store = ListStore::new::<BoxedAnyObject>();
  populate_section_headers(&source_store);
  let source_store_ref = source_store.clone();

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
  let content_stack_for_sel = content_stack.clone();
  let buf_for_sel = Rc::clone(&all_items_buf);
  let btn_for_sel = show_more_btn.clone();
  source_selection.connect_selection_changed(move |sel, _, _| {
    let selected = get_selected_sources(&tree_model_for_sel, sel);
    reload_grid(&grid_store_for_sel, &selected, &search_for_sel.borrow(), &buf_for_sel, &btn_for_sel);
    content_stack_for_sel.set_visible_child_name("grid");
  });

  // Grid view
  let channel_names: Rc<std::collections::HashMap<i32, String>> = Rc::new(get_channel_name_map());

  let grid_factory = SignalListItemFactory::new();
  grid_factory.connect_setup(|_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let card = BrowseCard::new();
    card.add_css_class("browse-card");
    list_item.set_child(Some(&card));
  });

  let thumb_tx_for_bind = thumb_tx.clone();
  let pending_for_bind = pending_thumbs.clone();
  let channel_names_for_bind = Rc::clone(&channel_names);
  let pc_for_bind = Rc::clone(&playback_controller);
  grid_factory.connect_bind(move |_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let card = list_item.child().unwrap().downcast::<BrowseCard>().unwrap();
    let obj = list_item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let browse_item: Ref<BrowseItem> = obj.borrow();

    let play_key = browse_item_play_key(&browse_item);
    match &*browse_item {
      BrowseItem::Media(media_item) => {
        card.set_title(media_item.title());
        let subtitle = media_item.artist_with_channel_names(&channel_names_for_bind);
        card.set_subtitle(&subtitle);
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

    match (&play_key, &pc_for_bind.play_state()) {
      (Some(key), PlayState::Loading(k)) if key == k => card.set_loading(true),
      (Some(key), PlayState::Playing(k)) if key == k => card.set_playing(true),
      _ => card.clear_state(),
    }
  });

  grid_factory.connect_unbind(|_factory, item| {
    let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let card = list_item.child().unwrap().downcast::<BrowseCard>().unwrap();
    card.set_title("");
    card.set_subtitle("");
    card.clear_thumbnail();
    card.clear_state();
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
  let content_stack_for_activate = content_stack.clone();
  grid_view.connect_activate(move |_view, pos| {
    if let Some(obj) = grid_store_for_activate.item(pos) {
      if let Ok(boxed) = obj.downcast::<BoxedAnyObject>() {
        let browse_item: Ref<BrowseItem> = boxed.borrow();
        match &*browse_item {
          BrowseItem::Media(item) => {
            let item = item.clone();
            drop(browse_item);
            let mut rebind_keys: Vec<String> = Vec::new();
            match pc_for_activate.play_state() {
              PlayState::Loading(k) | PlayState::Playing(k) => rebind_keys.push(k),
              PlayState::Idle => {}
            }
            if let Some(key) = browse_item_play_key(&BrowseItem::Media(item.clone())) {
              pc_for_activate.set_play_state(PlayState::Loading(key.clone()));
              rebind_keys.push(key);
            }
            rebind_matching_items(&grid_store_for_activate, &rebind_keys);
            pc_for_activate.play_media_item(&item);
          }
          BrowseItem::Album {
            artist,
            album,
            representative_filename,
          } => {
            let artist = artist.clone();
            let album = album.clone();
            let representative_filename = representative_filename.clone();
            drop(browse_item);
            if let Some(old) = content_stack_for_activate.child_by_name("detail") {
              content_stack_for_activate.remove(&old);
            }
            let detail = build_album_detail(
              &artist,
              &album,
              &representative_filename,
              &pc_for_activate,
              &content_stack_for_activate,
            );
            content_stack_for_activate.add_named(&detail, Some("detail"));
            content_stack_for_activate.set_visible_child_name("detail");
          }
        }
      }
    }
  });

  // Refresh browse cards when play state changes (Loading -> Playing)
  let grid_store_for_state = grid_store.clone();
  playback_controller.set_on_play_state_changed(Some(Rc::new(move |state| {
    match state {
      PlayState::Loading(k) | PlayState::Playing(k) => {
        rebind_matching_items(&grid_store_for_state, &[k.clone()]);
      }
      PlayState::Idle => {}
    }
  })));

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
  let buf_for_download = Rc::clone(&all_items_buf);
  let btn_for_download = show_more_btn.clone();
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

    let (tx, rx) = futures::channel::mpsc::unbounded::<ThumbnailProgress>();
    std::thread::spawn(move || {
      let tx_video = tx.clone();
      let (video_dl, _) = thumbnail_cache::download_all_video_thumbnails(move |done, total| {
        let _ = tx_video.unbounded_send(ThumbnailProgress::VideoProgress(done, total));
      });

      let tx_album = tx.clone();
      let (album_dl, _) = thumbnail_cache::download_all_album_art(move |done, total| {
        let _ = tx_album.unbounded_send(ThumbnailProgress::AlbumProgress(done, total));
      });

      let _ = tx.unbounded_send(ThumbnailProgress::Done(video_dl, album_dl));
    });

    let btn_clone = btn.clone();
    let dialog_clone = progress_dialog.clone();
    let grid_store_done = grid_store_for_refresh.clone();
    let tree_model_done = tree_model_for_refresh.clone();
    let source_sel_done = source_sel_for_refresh.clone();
    let search_done = Rc::clone(&search_for_download);
    let buf_done = Rc::clone(&buf_for_download);
    let btn_done = btn_for_download.clone();
    gtk::glib::spawn_future_local(async move {
      use futures::stream::StreamExt;
      let mut rx = rx;
      while let Some(progress) = rx.next().await {
        match progress {
          ThumbnailProgress::VideoProgress(done, total) => {
            if total > 0 {
              progress_bar.set_fraction(done as f64 / total as f64 * 0.5);
              progress_bar.set_text(Some(&format!("Videos: {done}/{total}")));
            }
          }
          ThumbnailProgress::AlbumProgress(done, total) => {
            if total > 0 {
              progress_bar.set_fraction(0.5 + done as f64 / total as f64 * 0.5);
              progress_bar.set_text(Some(&format!("Albums: {done}/{total}")));
            }
          }
          ThumbnailProgress::Done(videos, albums) => {
            status_label.set_label(&format!("Done: {videos} video + {albums} album thumbnails downloaded"));
            progress_bar.set_fraction(1.0);
            progress_bar.set_text(Some("Complete"));

            let selected = get_selected_sources(&tree_model_done, &source_sel_done);
            reload_grid(&grid_store_done, &selected, &search_done.borrow(), &buf_done, &btn_done);

            let dialog = dialog_clone.clone();
            let btn = btn_clone.clone();
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
              dialog.close();
              btn.set_sensitive(true);
            });
            break;
          }
        }
      }
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
  let buf_for_search = Rc::clone(&all_items_buf);
  let btn_for_search = show_more_btn.clone();
  search_entry.connect_search_changed(move |entry| {
    let query = entry.text().to_string();
    *search_for_entry.borrow_mut() = query.clone();
    let selected = get_selected_sources(&tree_model_for_search, &source_sel_for_search);
    reload_grid(&grid_store_for_search, &selected, &query, &buf_for_search, &btn_for_search);
  });

  // Add YouTube channel button
  let add_channel_btn = gtk::Button::builder()
    .icon_name("list-add-symbolic")
    .tooltip_text("Add YouTube channel or playlist")
    .css_classes(["flat"])
    .build();

  let pc_for_add = Rc::clone(&playback_controller);
  let source_store_for_add = source_store_ref.clone();
  add_channel_btn.connect_clicked(move |_| {
    let source_store = source_store_for_add.clone();
    youtube_add_dialog::show_dialog(Rc::clone(&pc_for_add), move || {
      source_store.remove_all();
      populate_section_headers(&source_store);
    });
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
  sidebar_header.append(&add_channel_btn);
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

  let grid_page = gtk::Box::builder()
    .orientation(Orientation::Vertical)
    .hexpand(true)
    .vexpand(true)
    .build();
  grid_page.append(&search_entry);
  grid_page.append(&grid_scrolled);
  grid_page.append(&show_more_btn);

  content_stack.add_named(&grid_page, Some("grid"));
  content_stack.set_visible_child_name("grid");

  let paned = gtk::Paned::builder()
    .orientation(Orientation::Horizontal)
    .start_child(&sidebar)
    .end_child(&content_stack)
    .build();
  paned.set_position(200);

  let main_box = gtk::Box::new(Orientation::Horizontal, 0);
  main_box.set_hexpand(true);
  main_box.set_vexpand(true);
  main_box.append(&paned);
  main_box
}
