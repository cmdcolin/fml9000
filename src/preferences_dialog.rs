use crate::settings::{write_settings, FmlSettings};
use gtk::glib;
use adw::prelude::*;
use gtk::{Button, Entry, FileChooserAction, FileChooserDialog, Orientation, ResponseType};
use std::cell::RefCell;
use std::rc::Rc;

pub async fn dialog<W: IsA<gtk::Window>>(wnd: Rc<W>, settings: Rc<RefCell<FmlSettings>>) {
  let preferences_dialog = gtk::Dialog::builder()
    .transient_for(&*wnd)
    .modal(true)
    .default_width(800)
    .default_height(600)
    .title("Preferences")
    .build();

  let folder_box = gtk::Box::new(Orientation::Horizontal, 0);

  let content_area = preferences_dialog.content_area();
  let open_button = Button::builder().label("Open folder...").build();
  let s = settings.borrow().folder.clone();
  let textbox = Entry::builder()
    .text(s.as_ref().unwrap_or(&"Empty".to_string()))
    .hexpand(true)
    .build();

  folder_box.append(&textbox);
  folder_box.append(&open_button);
  content_area.append(&folder_box);

  let preferences_dialog_rc = Rc::new(preferences_dialog);
  open_button.connect_clicked(
    glib::clone!(@weak wnd, @weak textbox, @weak settings => move |_| {
      let file_chooser = FileChooserDialog::new(
        Some("Open Folder"),
        Some(&*wnd),
        FileChooserAction::SelectFolder,
        &[("Open", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
      );
      file_chooser.set_modal(true);
      file_chooser.connect_response(move |d: &FileChooserDialog, response: ResponseType| {
        if response == ResponseType::Ok {
          let file = d.file().expect("Couldn't get file");
          let p = file.path().expect("Couldn't get file path");
          let folder = &p.to_string_lossy();
          textbox.set_text(folder);
          let mut s = settings.borrow_mut();
          s.folder = Some(folder.to_string());
          write_settings(&s).expect("Failed to write");
        }
        d.close();
      });
      file_chooser.show();
    }),
  );

  preferences_dialog_rc.run_future().await;
}
