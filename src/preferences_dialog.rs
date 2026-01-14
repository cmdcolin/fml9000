use crate::settings::{write_settings, FmlSettings};
use adw::prelude::*;
use gtk::gio;
use gtk::FileDialog;
use std::cell::RefCell;
use std::rc::Rc;

pub async fn dialog<W: IsA<gtk::Window>>(wnd: Rc<W>, settings: Rc<RefCell<FmlSettings>>) {
  let current_folder = settings.borrow().folder.clone();
  let folder_label = current_folder
    .as_deref()
    .unwrap_or("No folder selected");

  let folder_row = adw::ActionRow::builder()
    .title("Music Library Folder")
    .subtitle(folder_label)
    .activatable(true)
    .build();

  folder_row.add_suffix(
    &gtk::Image::builder()
      .icon_name("folder-open-symbolic")
      .build(),
  );

  let library_group = adw::PreferencesGroup::builder()
    .title("Library")
    .build();
  library_group.add(&folder_row);

  let page = adw::PreferencesPage::new();
  page.add(&library_group);

  let preferences_window = adw::PreferencesWindow::builder()
    .title("Preferences")
    .transient_for(&*wnd)
    .modal(true)
    .build();
  preferences_window.add(&page);

  let settings_for_click = Rc::clone(&settings);
  let prefs_window_for_click = preferences_window.clone();
  folder_row.connect_activated(move |row| {
    let dialog = FileDialog::builder()
      .title("Select Music Folder")
      .accept_label("Select")
      .build();

    let row = row.clone();
    let settings = Rc::clone(&settings_for_click);

    dialog.select_folder(
      Some(&prefs_window_for_click),
      gio::Cancellable::NONE,
      move |folder| {
        if let Ok(folder) = folder {
          let Some(p) = folder.path() else {
            eprintln!("Warning: Could not get folder path");
            return;
          };
          let folder_str = p.to_string_lossy().to_string();
          row.set_subtitle(&folder_str);
          let mut s = settings.borrow_mut();
          s.folder = Some(folder_str);
          if let Err(e) = write_settings(&s) {
            eprintln!("Warning: {e}");
          }
        }
      },
    );
  });

  preferences_window.present();
}
