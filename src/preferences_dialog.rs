use crate::settings::{write_settings, FmlSettings};
use adw::prelude::*;
use fml9000::models::Track;
use fml9000::run_scan_folders;
use gtk::gio;
use gtk::FileDialog;
use std::cell::RefCell;
use std::rc::Rc;

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

pub async fn dialog<W: IsA<gtk::Window>>(
  wnd: Rc<W>,
  settings: Rc<RefCell<FmlSettings>>,
  tracks: Rc<Vec<Rc<Track>>>,
) {
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

  let page = adw::PreferencesPage::new();
  page.add(&library_group);
  page.add(&scan_group);

  let preferences_window = adw::PreferencesWindow::builder()
    .title("Preferences")
    .transient_for(&*wnd)
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

  preferences_window.present();
}
