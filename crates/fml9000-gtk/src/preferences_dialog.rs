use crate::playback_controller::PlaybackController;
use crate::playlist_manager::populate_playlist_store;
use crate::settings::{write_settings, FmlSettings, RowHeight};
use crate::facet_box::{load_facet_store, load_playlist_store};
use crate::youtube_add_dialog;
use fml9000_core::youtube_api;
use adw::prelude::*;
use fml9000_core::Track;
use fml9000_core::{
  add_youtube_videos, delete_tracks_by_filename, delete_youtube_channel, get_video_count_for_channel,
  get_video_ids_for_channel, get_youtube_channels,
  load_tracks, run_scan_with_progress, update_channel_last_fetched, ScanProgress,
};
use gtk::gio;
use gtk::gio::ListStore;
use gtk::glib;
use gtk::FileDialog;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;

fn refresh_store(store: &ListStore) {
  let items: Vec<_> = (0..store.n_items())
    .filter_map(|i| store.item(i))
    .collect();
  store.remove_all();
  for item in items {
    store.append(&item);
  }
}

fn refresh_stores(playlist_store: &ListStore, facet_store: &ListStore, playlist_mgr_store: &ListStore) {
  refresh_store(playlist_store);
  refresh_store(facet_store);
  refresh_store(playlist_mgr_store);
}

fn create_folder_chip(
  folder: &str,
  flowbox: &gtk::FlowBox,
  settings: Rc<RefCell<FmlSettings>>,
) -> gtk::Box {
  let chip = gtk::Box::builder()
    .orientation(gtk::Orientation::Horizontal)
    .spacing(4)
    .css_classes(["card", "folder-chip"])
    .build();

  let label = gtk::Label::builder()
    .label(folder)
    .ellipsize(gtk::pango::EllipsizeMode::Middle)
    .max_width_chars(40)
    .tooltip_text(folder)
    .build();

  let remove_btn = gtk::Button::builder()
    .icon_name("window-close-symbolic")
    .css_classes(["flat", "circular"])
    .tooltip_text("Remove folder")
    .build();

  chip.append(&label);
  chip.append(&remove_btn);

  let folder_str = folder.to_string();
  let flowbox_clone = flowbox.clone();
  let chip_clone = chip.clone();
  remove_btn.connect_clicked(move |_| {
    let mut s = settings.borrow_mut();
    s.remove_folder(&folder_str);
    if let Err(e) = write_settings(&s) {
      eprintln!("Warning: {e}");
    }
    if let Some(parent) = chip_clone.parent() {
      if let Some(flowbox_child) = parent.downcast_ref::<gtk::FlowBoxChild>() {
        flowbox_clone.remove(flowbox_child);
      }
    }
  });

  chip
}

fn rebuild_folder_list(
  flowbox: &gtk::FlowBox,
  settings: Rc<RefCell<FmlSettings>>,
  placeholder: &gtk::Label,
) {
  while let Some(child) = flowbox.first_child() {
    flowbox.remove(&child);
  }

  let folders = settings.borrow().folders.clone();
  if folders.is_empty() {
    placeholder.set_visible(true);
  } else {
    placeholder.set_visible(false);
    for folder in &folders {
      let chip = create_folder_chip(folder, flowbox, Rc::clone(&settings));
      flowbox.append(&chip);
    }
  }
}

pub async fn dialog(
  playback_controller: Rc<PlaybackController>,
  settings: Rc<RefCell<FmlSettings>>,
  tracks: Rc<Vec<Arc<Track>>>,
  playlist_store: gtk::gio::ListStore,
  facet_store: gtk::gio::ListStore,
  playlist_mgr_store: gtk::gio::ListStore,
) {
  let wnd = playback_controller.window();
  let folder_flowbox = gtk::FlowBox::builder()
    .selection_mode(gtk::SelectionMode::None)
    .homogeneous(false)
    .row_spacing(6)
    .column_spacing(6)
    .min_children_per_line(1)
    .max_children_per_line(10)
    .build();

  let placeholder_label = gtk::Label::builder()
    .label("No folders added")
    .css_classes(["dim-label"])
    .build();

  let folders_container = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .spacing(12)
    .margin_top(12)
    .margin_bottom(12)
    .margin_start(12)
    .margin_end(12)
    .build();
  folders_container.append(&placeholder_label);
  folders_container.append(&folder_flowbox);

  rebuild_folder_list(&folder_flowbox, Rc::clone(&settings), &placeholder_label);

  let add_folder_row = adw::ActionRow::builder()
    .title("Add Folder...")
    .activatable(true)
    .build();
  add_folder_row.add_prefix(
    &gtk::Image::builder()
      .icon_name("folder-new-symbolic")
      .build(),
  );
  add_folder_row.add_suffix(
    &gtk::Image::builder()
      .icon_name("go-next-symbolic")
      .build(),
  );

  let rescan_button = gtk::Button::builder()
    .label("Rescan Now")
    .css_classes(["suggested-action"])
    .valign(gtk::Align::Center)
    .build();

  let rescan_row = adw::ActionRow::builder()
    .title("Rescan Library")
    .subtitle("Scan all folders for new music files")
    .build();
  rescan_row.add_suffix(&rescan_button);

  let rescan_on_startup_switch = gtk::Switch::builder()
    .active(settings.borrow().rescan_on_startup)
    .valign(gtk::Align::Center)
    .build();

  let rescan_on_startup_row = adw::ActionRow::builder()
    .title("Rescan on startup")
    .subtitle("Automatically scan folders when the app starts")
    .build();
  rescan_on_startup_row.add_suffix(&rescan_on_startup_switch);
  rescan_on_startup_row.set_activatable_widget(Some(&rescan_on_startup_switch));

  let library_group = adw::PreferencesGroup::builder()
    .title("Library Folders")
    .build();
  library_group.add(&folders_container);
  library_group.add(&add_folder_row);

  let scan_group = adw::PreferencesGroup::builder()
    .title("Scanning")
    .build();
  scan_group.add(&rescan_row);
  scan_group.add(&rescan_on_startup_row);

  let current_row_height = settings.borrow().row_height;

  let normal_radio = gtk::CheckButton::builder()
    .active(current_row_height == RowHeight::Normal)
    .valign(gtk::Align::Center)
    .build();

  let compact_radio = gtk::CheckButton::builder()
    .active(current_row_height == RowHeight::Compact)
    .group(&normal_radio)
    .valign(gtk::Align::Center)
    .build();

  let ultra_compact_radio = gtk::CheckButton::builder()
    .active(current_row_height == RowHeight::UltraCompact)
    .group(&normal_radio)
    .valign(gtk::Align::Center)
    .build();

  let normal_row = adw::ActionRow::builder()
    .title("Normal")
    .subtitle("Standard row height")
    .build();
  normal_row.add_prefix(&normal_radio);
  normal_row.set_activatable_widget(Some(&normal_radio));

  let compact_row = adw::ActionRow::builder()
    .title("Compact")
    .subtitle("Smaller row height")
    .build();
  compact_row.add_prefix(&compact_radio);
  compact_row.set_activatable_widget(Some(&compact_radio));

  let ultra_compact_row = adw::ActionRow::builder()
    .title("Ultra Compact")
    .subtitle("Smallest row height")
    .build();
  ultra_compact_row.add_prefix(&ultra_compact_radio);
  ultra_compact_row.set_activatable_widget(Some(&ultra_compact_radio));

  let appearance_group = adw::PreferencesGroup::builder()
    .title("Row Height")
    .build();
  appearance_group.add(&normal_row);
  appearance_group.add(&compact_row);
  appearance_group.add(&ultra_compact_row);

  let youtube_group = adw::PreferencesGroup::builder()
    .title("YouTube Channels")
    .build();

  let channel_rows: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::new(RefCell::new(Vec::new()));

  let library_page = adw::PreferencesPage::builder()
    .title("Library")
    .icon_name("folder-music-symbolic")
    .build();
  library_page.add(&library_group);
  library_page.add(&scan_group);

  let appearance_page = adw::PreferencesPage::builder()
    .title("Appearance")
    .icon_name("applications-graphics-symbolic")
    .build();
  appearance_page.add(&appearance_group);

  let youtube_page = adw::PreferencesPage::builder()
    .title("YouTube")
    .icon_name("video-display-symbolic")
    .build();
  youtube_page.add(&youtube_group);

  let preferences_window = adw::PreferencesWindow::builder()
    .title("Preferences")
    .transient_for(&**wnd)
    .modal(true)
    .build();
  preferences_window.add(&library_page);
  preferences_window.add(&appearance_page);
  preferences_window.add(&youtube_page);

  let settings_for_add = Rc::clone(&settings);
  let flowbox_for_add = folder_flowbox.clone();
  let placeholder_for_add = placeholder_label.clone();
  let prefs_window_for_add = preferences_window.clone();
  add_folder_row.connect_activated(move |_| {
    let dialog = FileDialog::builder()
      .title("Select Music Folder")
      .accept_label("Add")
      .build();

    let settings = Rc::clone(&settings_for_add);
    let flowbox = flowbox_for_add.clone();
    let placeholder = placeholder_for_add.clone();

    dialog.select_folder(
      Some(&prefs_window_for_add),
      gio::Cancellable::NONE,
      move |folder| {
        if let Ok(folder) = folder {
          let Some(p) = folder.path() else {
            eprintln!("Warning: Could not get folder path");
            return;
          };
          let folder_str = p.to_string_lossy().to_string();
          {
            let mut s = settings.borrow_mut();
            s.add_folder(folder_str);
            if let Err(e) = write_settings(&s) {
              eprintln!("Warning: {e}");
            }
          }
          rebuild_folder_list(&flowbox, Rc::clone(&settings), &placeholder);
        }
      },
    );
  });

  let settings_for_rescan = Rc::clone(&settings);
  let tracks_for_rescan = Rc::clone(&tracks);
  let prefs_window_for_rescan = preferences_window.clone();
  let playlist_store_for_rescan = playlist_store.clone();
  let facet_store_for_rescan = facet_store.clone();
  rescan_button.connect_clicked(move |btn| {
    btn.set_sensitive(false);

    let folders = settings_for_rescan.borrow().folders.clone();
    if folders.is_empty() {
      btn.set_sensitive(true);
      return;
    }

    let mut existing_complete: HashSet<String> = HashSet::new();
    let mut existing_incomplete: HashSet<String> = HashSet::new();
    for track in tracks_for_rescan.iter() {
      if track.duration_seconds.is_some() {
        existing_complete.insert(track.filename.clone());
      } else {
        existing_incomplete.insert(track.filename.clone());
      }
    }

    let progress_dialog = gtk::Window::builder()
      .title("Scanning Library")
      .modal(true)
      .transient_for(&prefs_window_for_rescan)
      .default_width(450)
      .default_height(150)
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
      .label("Starting scan...")
      .xalign(0.0)
      .wrap(true)
      .build();

    let progress_bar = gtk::ProgressBar::builder()
      .show_text(true)
      .build();
    progress_bar.set_text(Some("Scanning..."));

    let file_label = gtk::Label::builder()
      .label("")
      .xalign(0.0)
      .ellipsize(gtk::pango::EllipsizeMode::Middle)
      .css_classes(["dim-label"])
      .build();

    content.append(&status_label);
    content.append(&progress_bar);
    content.append(&file_label);

    progress_dialog.set_child(Some(&content));
    progress_dialog.present();

    let (tx, rx) = mpsc::channel::<ScanProgress>();

    std::thread::spawn(move || {
      run_scan_with_progress(folders, existing_complete, existing_incomplete, tx);
    });

    let btn_clone = btn.clone();
    let dialog_clone = progress_dialog.clone();
    let status_label_clone = status_label.clone();
    let progress_bar_clone = progress_bar.clone();
    let file_label_clone = file_label.clone();
    let playlist_store_clone = playlist_store_for_rescan.clone();
    let facet_store_clone = facet_store_for_rescan.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
      while let Ok(progress) = rx.try_recv() {
        match progress {
          ScanProgress::StartingFolder(folder) => {
            status_label_clone.set_label(&format!("Scanning: {}", folder));
          }
          ScanProgress::FoundFile(found, skipped, file) => {
            progress_bar_clone.pulse();
            progress_bar_clone.set_text(Some(&format!("Found {} files ({} existing)...", found, skipped)));
            if let Some(name) = std::path::Path::new(&file).file_name() {
              file_label_clone.set_label(&name.to_string_lossy());
            }
          }
          ScanProgress::ScannedFile(found, skipped, added, updated, file) => {
            if updated > 0 {
              progress_bar_clone.set_text(Some(&format!("{} files, {} skip, {} new, {} updated", found, skipped, added, updated)));
            } else {
              progress_bar_clone.set_text(Some(&format!("{} files, {} existing, {} new", found, skipped, added)));
            }
            if let Some(name) = std::path::Path::new(&file).file_name() {
              file_label_clone.set_label(&name.to_string_lossy());
            }
          }
          ScanProgress::Complete(found, skipped, added, updated, stale_files) => {
            if updated > 0 {
              status_label_clone.set_label(&format!(
                "Complete: {} found, {} existing, {} added, {} updated",
                found, skipped, added, updated
              ));
            } else {
              status_label_clone.set_label(&format!(
                "Complete: {} found, {} existing, {} added",
                found, skipped, added
              ));
            }
            progress_bar_clone.set_fraction(1.0);
            progress_bar_clone.set_text(Some("Complete"));
            file_label_clone.set_label("");

            let playlist_store_final = playlist_store_clone.clone();
            let facet_store_final = facet_store_clone.clone();
            let reload_stores = move || {
              if let Ok(fresh_tracks) = load_tracks() {
                playlist_store_final.remove_all();
                load_playlist_store(fresh_tracks.iter(), &playlist_store_final);
                facet_store_final.remove_all();
                load_facet_store(&fresh_tracks, &facet_store_final);
              }
            };

            if !stale_files.is_empty() {
              let stale_count = stale_files.len();
              let preview: String = stale_files.iter().take(10)
                .map(|f| {
                  std::path::Path::new(f)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| f.clone())
                })
                .collect::<Vec<_>>()
                .join("\n");
              let detail = if stale_count > 10 {
                format!("{preview}\n...and {} more", stale_count - 10)
              } else {
                preview
              };

              let confirm = gtk::AlertDialog::builder()
                .modal(true)
                .message(&format!("{stale_count} tracks no longer found on disk. Remove from library?"))
                .detail(&detail)
                .buttons(["Cancel", "Remove"])
                .default_button(0)
                .cancel_button(0)
                .build();

              let dialog_for_parent = dialog_clone.clone();
              let dialog_for_close = dialog_clone.clone();
              let btn_for_confirm = btn_clone.clone();
              confirm.choose(
                Some(&dialog_for_parent),
                None::<&gio::Cancellable>,
                move |result| {
                  if result == Ok(1) {
                    if let Err(e) = delete_tracks_by_filename(&stale_files) {
                      eprintln!("Warning: Failed to remove stale tracks: {e}");
                    }
                  }
                  reload_stores();
                  glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                    dialog_for_close.close();
                    btn_for_confirm.set_sensitive(true);
                  });
                },
              );
            } else {
              reload_stores();
              let dialog = dialog_clone.clone();
              let btn = btn_clone.clone();
              glib::timeout_add_local_once(std::time::Duration::from_millis(2000), move || {
                dialog.close();
                btn.set_sensitive(true);
              });
            }

            return glib::ControlFlow::Break;
          }
        }
      }
      glib::ControlFlow::Continue
    });
  });

  let settings_for_toggle = Rc::clone(&settings);
  rescan_on_startup_switch.connect_active_notify(move |switch: &gtk::Switch| {
    let mut s = settings_for_toggle.borrow_mut();
    s.rescan_on_startup = switch.is_active();
    if let Err(e) = write_settings(&s) {
      eprintln!("Warning: {e}");
    }
  });

  let settings_for_normal = Rc::clone(&settings);
  let ps1 = playlist_store.clone();
  let fs1 = facet_store.clone();
  let pms1 = playlist_mgr_store.clone();
  normal_radio.connect_active_notify(move |btn: &gtk::CheckButton| {
    if btn.is_active() {
      let mut s = settings_for_normal.borrow_mut();
      s.row_height = RowHeight::Normal;
      if let Err(e) = write_settings(&s) {
        eprintln!("Warning: {e}");
      }
      drop(s);
      refresh_stores(&ps1, &fs1, &pms1);
    }
  });

  let settings_for_compact = Rc::clone(&settings);
  let ps2 = playlist_store.clone();
  let fs2 = facet_store.clone();
  let pms2 = playlist_mgr_store.clone();
  compact_radio.connect_active_notify(move |btn: &gtk::CheckButton| {
    if btn.is_active() {
      let mut s = settings_for_compact.borrow_mut();
      s.row_height = RowHeight::Compact;
      if let Err(e) = write_settings(&s) {
        eprintln!("Warning: {e}");
      }
      drop(s);
      refresh_stores(&ps2, &fs2, &pms2);
    }
  });

  let settings_for_ultra = Rc::clone(&settings);
  let ps3 = playlist_store.clone();
  let fs3 = facet_store.clone();
  let pms3 = playlist_mgr_store.clone();
  ultra_compact_radio.connect_active_notify(move |btn: &gtk::CheckButton| {
    if btn.is_active() {
      let mut s = settings_for_ultra.borrow_mut();
      s.row_height = RowHeight::UltraCompact;
      if let Err(e) = write_settings(&s) {
        eprintln!("Warning: {e}");
      }
      drop(s);
      refresh_stores(&ps3, &fs3, &pms3);
    }
  });

  build_channel_rows(
    &youtube_group,
    &channel_rows,
    &preferences_window,
    &playlist_mgr_store,
    &playback_controller,
  );

  let add_channels_row = adw::ActionRow::builder()
    .title("Add Channels...")
    .activatable(true)
    .build();
  add_channels_row.add_prefix(
    &gtk::Image::builder()
      .icon_name("list-add-symbolic")
      .build(),
  );
  add_channels_row.add_suffix(
    &gtk::Image::builder()
      .icon_name("go-next-symbolic")
      .build(),
  );
  youtube_group.add(&add_channels_row);

  let yt_group_for_add = youtube_group.clone();
  let cr_for_add = channel_rows.clone();
  let prefs_window_for_add_yt = preferences_window.clone();
  let pms_for_add_yt = playlist_mgr_store.clone();
  let pc_for_add_yt = playback_controller.clone();
  add_channels_row.connect_activated(move |_| {
    let yt_group = yt_group_for_add.clone();
    let cr = cr_for_add.clone();
    let prefs_window = prefs_window_for_add_yt.clone();
    let pms = pms_for_add_yt.clone();
    let pc = pc_for_add_yt.clone();
    youtube_add_dialog::show_dialog(pc.clone(), move || {
      build_channel_rows(&yt_group, &cr, &prefs_window, &pms, &pc);
      pms.remove_all();
      populate_playlist_store(&pms);
    });
  });

  let refresh_yt_button = gtk::Button::builder()
    .label("Refresh All")
    .css_classes(["suggested-action"])
    .valign(gtk::Align::Center)
    .build();

  let refresh_yt_row = adw::ActionRow::builder()
    .title("Refresh All Channels")
    .subtitle("Fetch new videos from all YouTube channels")
    .build();
  refresh_yt_row.add_suffix(&refresh_yt_button);
  youtube_group.add(&refresh_yt_row);

  let full_refresh_yt_button = gtk::Button::builder()
    .label("Full Refresh All")
    .valign(gtk::Align::Center)
    .build();

  let full_refresh_yt_row = adw::ActionRow::builder()
    .title("Full Refresh All Channels")
    .subtitle("Re-fetch all videos, not just new ones")
    .build();
  full_refresh_yt_row.add_suffix(&full_refresh_yt_button);
  youtube_group.add(&full_refresh_yt_row);

  for (btn, fetch_all) in [(&refresh_yt_button, false), (&full_refresh_yt_button, true)] {
    let prefs_window_for_yt = preferences_window.clone();
    let playlist_mgr_store_for_yt = playlist_mgr_store.clone();
    let yt_group_for_refresh = youtube_group.clone();
    let cr_for_refresh = channel_rows.clone();
    let pc_for_refresh = playback_controller.clone();
    btn.connect_clicked(move |btn| {
      btn.set_sensitive(false);

      let channels = match get_youtube_channels() {
        Ok(c) => c,
        Err(e) => {
          eprintln!("Failed to get YouTube channels: {e}");
          btn.set_sensitive(true);
          return;
        }
      };

      if channels.is_empty() {
        btn.set_sensitive(true);
        return;
      }

      let progress_dialog = gtk::Window::builder()
        .title("Refreshing YouTube Channels")
        .modal(true)
        .transient_for(&prefs_window_for_yt)
        .default_width(450)
        .default_height(150)
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
        .label("Starting refresh...")
        .xalign(0.0)
        .wrap(true)
        .build();

      let progress_bar = gtk::ProgressBar::builder()
        .show_text(true)
        .build();

      content.append(&status_label);
      content.append(&progress_bar);
      progress_dialog.set_child(Some(&content));
      progress_dialog.present();

      #[derive(Debug)]
      enum YtRefreshProgress {
        StartingChannel(String),
        FoundVideos(String, usize),
        ChannelDone(String, usize),
        ChannelError(String, String),
        AllDone(usize, usize),
      }

      let (tx, rx) = mpsc::channel::<YtRefreshProgress>();
      let total_channels = channels.len();

      let channel_data: Vec<_> = channels
        .iter()
        .map(|c| (c.id, c.name.clone(), c.handle.clone()))
        .collect();

      std::thread::spawn(move || {
        let mut total_new_videos = 0usize;
        let mut channels_updated = 0usize;

        for (channel_id, channel_name, channel_handle) in &channel_data {
          let _ = tx.send(YtRefreshProgress::StartingChannel(channel_name.clone()));

          let handle = match channel_handle {
            Some(h) => h.clone(),
            None => {
              let _ = tx.send(YtRefreshProgress::ChannelError(
                channel_name.clone(),
                "No handle available".to_string(),
              ));
              std::thread::sleep(std::time::Duration::from_secs(1));
              continue;
            }
          };

          let existing_ids = match get_video_ids_for_channel(*channel_id) {
            Ok(ids) => ids,
            Err(e) => {
              let _ = tx.send(YtRefreshProgress::ChannelError(channel_name.clone(), e));
              std::thread::sleep(std::time::Duration::from_secs(1));
              continue;
            }
          };

          let playlist_id = match youtube_api::get_playlist_id_for_handle(&handle) {
            Ok(id) => id,
            Err(e) => {
              let _ = tx.send(YtRefreshProgress::ChannelError(channel_name.clone(), e));
              std::thread::sleep(std::time::Duration::from_secs(1));
              continue;
            }
          };

          let name_for_progress = channel_name.clone();
          let tx_for_progress = tx.clone();
          let new_videos = match youtube_api::fetch_new_videos(&playlist_id, &existing_ids, fetch_all, move |found, _total| {
            let _ = tx_for_progress.send(YtRefreshProgress::FoundVideos(name_for_progress.clone(), found));
          }) {
            Ok(v) => v,
            Err(e) => {
              let _ = tx.send(YtRefreshProgress::ChannelError(channel_name.clone(), e));
              std::thread::sleep(std::time::Duration::from_secs(1));
              continue;
            }
          };

          let new_count = new_videos.len();
          if new_count > 0 {
            let video_tuples: Vec<_> = new_videos
              .iter()
              .map(|v| (v.video_id.clone(), v.title.clone(), None, v.thumbnail_url.clone(), v.published_at))
              .collect();
            let _ = add_youtube_videos(*channel_id, &video_tuples);
            total_new_videos += new_count;
          }

          let _ = update_channel_last_fetched(*channel_id);
          channels_updated += 1;

          let _ = tx.send(YtRefreshProgress::ChannelDone(channel_name.clone(), new_count));
          std::thread::sleep(std::time::Duration::from_secs(3));
        }

        let _ = tx.send(YtRefreshProgress::AllDone(channels_updated, total_new_videos));
      });

      let btn_clone = btn.clone();
      let dialog_clone = progress_dialog.clone();
      let status_clone = status_label.clone();
      let progress_clone = progress_bar.clone();
      let pms_clone = playlist_mgr_store_for_yt.clone();
      let yt_group_clone = yt_group_for_refresh.clone();
      let cr_clone = cr_for_refresh.clone();
      let prefs_window_clone = prefs_window_for_yt.clone();
      let pc_clone = pc_for_refresh.clone();
      let channels_done = Rc::new(RefCell::new(0usize));

      glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        while let Ok(progress) = rx.try_recv() {
          match progress {
            YtRefreshProgress::StartingChannel(name) => {
              let done = *channels_done.borrow();
              status_clone.set_label(&format!("Checking {} ({}/{})", name, done + 1, total_channels));
              progress_clone.set_fraction(done as f64 / total_channels as f64);
            }
            YtRefreshProgress::FoundVideos(name, count) => {
              status_clone.set_label(&format!("{}: found {} new videos...", name, count));
            }
            YtRefreshProgress::ChannelDone(name, count) => {
              *channels_done.borrow_mut() += 1;
              let done = *channels_done.borrow();
              if count > 0 {
                status_clone.set_label(&format!("{}: added {} new videos", name, count));
              } else {
                status_clone.set_label(&format!("{}: up to date", name));
              }
              progress_clone.set_fraction(done as f64 / total_channels as f64);
            }
            YtRefreshProgress::ChannelError(name, err) => {
              *channels_done.borrow_mut() += 1;
              let done = *channels_done.borrow();
              status_clone.set_label(&format!("{}: error - {}", name, err));
              progress_clone.set_fraction(done as f64 / total_channels as f64);
            }
            YtRefreshProgress::AllDone(channels, videos) => {
              progress_clone.set_fraction(1.0);
              status_clone.set_label(&format!(
                "Complete: {} channels checked, {} new videos added",
                channels, videos
              ));

              build_channel_rows(&yt_group_clone, &cr_clone, &prefs_window_clone, &pms_clone, &pc_clone);
              pms_clone.remove_all();
              populate_playlist_store(&pms_clone);

              let dialog = dialog_clone.clone();
              let btn = btn_clone.clone();
              glib::timeout_add_local_once(std::time::Duration::from_millis(2000), move || {
                dialog.close();
                btn.set_sensitive(true);
              });

              return glib::ControlFlow::Break;
            }
          }
        }
        glib::ControlFlow::Continue
      });
    });
  }

  preferences_window.present();
}

fn build_channel_rows(
  youtube_group: &adw::PreferencesGroup,
  channel_rows: &Rc<RefCell<Vec<adw::ActionRow>>>,
  preferences_window: &adw::PreferencesWindow,
  playlist_mgr_store: &ListStore,
  playback_controller: &Rc<PlaybackController>,
) {
  for row in channel_rows.borrow().iter() {
    youtube_group.remove(row);
  }
  channel_rows.borrow_mut().clear();

  let channels = match get_youtube_channels() {
    Ok(c) => c,
    Err(e) => {
      eprintln!("Failed to get YouTube channels: {e}");
      return;
    }
  };

  for channel in &channels {
    let video_count = get_video_count_for_channel(channel.id).unwrap_or(0);
    let subtitle = if let Some(last_fetched) = channel.last_fetched {
      format!(
        "{} videos \u{00b7} Last fetched: {}",
        video_count,
        last_fetched.format("%Y-%m-%d %H:%M")
      )
    } else {
      format!("{} videos \u{00b7} Never fetched", video_count)
    };

    let row = adw::ActionRow::builder()
      .title(&channel.name)
      .subtitle(&subtitle)
      .css_classes(["yt-channel-row"])
      .build();

    let refresh_btn = gtk::Button::builder()
      .icon_name("view-refresh-symbolic")
      .css_classes(["flat", "circular"])
      .tooltip_text("Refresh this channel")
      .valign(gtk::Align::Center)
      .build();

    let delete_btn = gtk::Button::builder()
      .icon_name("user-trash-symbolic")
      .css_classes(["flat", "circular"])
      .tooltip_text("Delete this channel")
      .valign(gtk::Align::Center)
      .build();

    row.add_suffix(&refresh_btn);
    row.add_suffix(&delete_btn);

    let channel_id = channel.id;
    let channel_name = channel.name.clone();
    let channel_handle = channel.handle.clone();
    let yt_group = youtube_group.clone();
    let cr = channel_rows.clone();
    let prefs_window = preferences_window.clone();
    let pms = playlist_mgr_store.clone();
    let pc = playback_controller.clone();
    refresh_btn.connect_clicked(move |btn| {
      btn.set_sensitive(false);

      let handle = match &channel_handle {
        Some(h) => h.clone(),
        None => {
          eprintln!("No handle for channel {}", channel_name);
          btn.set_sensitive(true);
          return;
        }
      };

      let progress_dialog = gtk::Window::builder()
        .title(&format!("Refreshing {}", channel_name))
        .modal(true)
        .transient_for(&prefs_window)
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
        .label(&format!("Refreshing {}...", channel_name))
        .xalign(0.0)
        .wrap(true)
        .build();

      let spinner = gtk::Spinner::builder().spinning(true).build();

      content.append(&status_label);
      content.append(&spinner);
      progress_dialog.set_child(Some(&content));
      progress_dialog.present();

      enum SingleChannelProgress {
        FoundVideos(usize),
        Done(usize),
        Error(String),
      }

      let (tx, rx) = mpsc::channel::<SingleChannelProgress>();
      let name_clone = channel_name.clone();

      std::thread::spawn(move || {
        let existing_ids = match get_video_ids_for_channel(channel_id) {
          Ok(ids) => ids,
          Err(e) => {
            let _ = tx.send(SingleChannelProgress::Error(e));
            return;
          }
        };

        let playlist_id = match youtube_api::get_playlist_id_for_handle(&handle) {
          Ok(id) => id,
          Err(e) => {
            let _ = tx.send(SingleChannelProgress::Error(e));
            return;
          }
        };

        let tx_for_progress = tx.clone();
        let new_videos = match youtube_api::fetch_new_videos(&playlist_id, &existing_ids, false, move |found, _total| {
          let _ = tx_for_progress.send(SingleChannelProgress::FoundVideos(found));
        }) {
          Ok(v) => v,
          Err(e) => {
            let _ = tx.send(SingleChannelProgress::Error(e));
            return;
          }
        };

        let new_count = new_videos.len();
        if new_count > 0 {
          let video_tuples: Vec<_> = new_videos
            .iter()
            .map(|v| (v.video_id.clone(), v.title.clone(), None, v.thumbnail_url.clone(), v.published_at))
            .collect();
          let _ = add_youtube_videos(channel_id, &video_tuples);
        }

        let _ = update_channel_last_fetched(channel_id);
        let _ = tx.send(SingleChannelProgress::Done(new_count));
      });

      let btn_clone = btn.clone();
      let dialog_clone = progress_dialog.clone();
      let status_clone = status_label.clone();
      let yt_group_clone = yt_group.clone();
      let cr_clone = cr.clone();
      let prefs_window_clone = prefs_window.clone();
      let pms_clone = pms.clone();
      let pc_clone = pc.clone();
      let name_for_done = name_clone;

      glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        while let Ok(progress) = rx.try_recv() {
          match progress {
            SingleChannelProgress::FoundVideos(count) => {
              status_clone.set_label(&format!("{}: found {} new videos...", name_for_done, count));
            }
            SingleChannelProgress::Done(count) => {
              if count > 0 {
                status_clone.set_label(&format!("{}: added {} new videos", name_for_done, count));
              } else {
                status_clone.set_label(&format!("{}: up to date", name_for_done));
              }

              build_channel_rows(&yt_group_clone, &cr_clone, &prefs_window_clone, &pms_clone, &pc_clone);
              pms_clone.remove_all();
              populate_playlist_store(&pms_clone);

              let dialog = dialog_clone.clone();
              let btn = btn_clone.clone();
              glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
                dialog.close();
                btn.set_sensitive(true);
              });
              return glib::ControlFlow::Break;
            }
            SingleChannelProgress::Error(err) => {
              status_clone.set_label(&format!("Error: {}", err));

              let dialog = dialog_clone.clone();
              let btn = btn_clone.clone();
              glib::timeout_add_local_once(std::time::Duration::from_millis(3000), move || {
                dialog.close();
                btn.set_sensitive(true);
              });
              return glib::ControlFlow::Break;
            }
          }
        }
        glib::ControlFlow::Continue
      });
    });

    let channel_id_for_delete = channel.id;
    let channel_name_for_delete = channel.name.clone();
    let video_count_for_delete = video_count;
    let yt_group_for_delete = youtube_group.clone();
    let cr_for_delete = channel_rows.clone();
    let prefs_window_for_delete = preferences_window.clone();
    let pms_for_delete = playlist_mgr_store.clone();
    let pc_for_delete = playback_controller.clone();
    delete_btn.connect_clicked(move |_| {
      let dialog = gtk::AlertDialog::builder()
        .modal(true)
        .message("Delete Channel?")
        .detail(&format!(
          "Delete channel \u{201c}{}\u{201d} and all its {} videos?",
          channel_name_for_delete, video_count_for_delete
        ))
        .buttons(["Cancel", "Delete"])
        .cancel_button(0)
        .default_button(0)
        .build();

      let yt_group = yt_group_for_delete.clone();
      let cr = cr_for_delete.clone();
      let prefs_window = prefs_window_for_delete.clone();
      let pms = pms_for_delete.clone();
      let pc = pc_for_delete.clone();
      dialog.choose(
        Some(&prefs_window_for_delete),
        gio::Cancellable::NONE,
        move |response| {
          if response == Ok(1) {
            if let Err(e) = delete_youtube_channel(channel_id_for_delete) {
              eprintln!("Failed to delete channel: {e}");
              return;
            }
            build_channel_rows(&yt_group, &cr, &prefs_window, &pms, &pc);
            pms.remove_all();
            populate_playlist_store(&pms);
          }
        },
      );
    });

    channel_rows.borrow_mut().push(row.clone());
    youtube_group.add(&row);
  }
}
