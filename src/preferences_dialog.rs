use crate::playback_controller::PlaybackController;
use crate::settings::{write_settings, FmlSettings, RowHeight};
use adw::prelude::*;
use fml9000::models::Track;
use fml9000::run_scan_folders;
use gtk::gio;
use gtk::gio::ListStore;
use gtk::FileDialog;
use std::cell::RefCell;
use std::rc::Rc;

fn refresh_store(store: &ListStore) {
  // Collect all items, clear, and re-add to force complete rebind
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
  tracks: Rc<Vec<Rc<Track>>>,
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

  let page = adw::PreferencesPage::new();
  page.add(&library_group);
  page.add(&scan_group);
  page.add(&appearance_group);

  let preferences_window = adw::PreferencesWindow::builder()
    .title("Preferences")
    .transient_for(&**wnd)
    .modal(true)
    .build();
  preferences_window.add(&page);

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
  rescan_button.connect_clicked(move |btn| {
    btn.set_sensitive(false);
    btn.set_label("Scanning...");

    let folders = settings_for_rescan.borrow().folders.clone();
    run_scan_folders(&folders, &tracks_for_rescan);

    btn.set_label("Rescan Now");
    btn.set_sensitive(true);
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

  preferences_window.present();
}
