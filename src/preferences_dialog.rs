use crate::settings::{write_settings, FmlSettings};
use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use gtk::{Button, Entry, FileDialog, Orientation};
use std::cell::RefCell;
use std::rc::Rc;

pub async fn dialog<W: IsA<gtk::Window>>(wnd: Rc<W>, settings: Rc<RefCell<FmlSettings>>) {
  let f = gtk::Box::new(Orientation::Horizontal, 0);

  let open_button = Button::builder().label("Open folder...").build();
  let s = settings.borrow().folder.clone();
  let textbox = Entry::builder()
    .text(s.as_ref().unwrap_or(&"Empty".to_string()))
    .hexpand(true)
    .build();

  f.append(&textbox);
  f.append(&open_button);
  let preferences_dialog = gtk::Window::builder()
    .transient_for(&*wnd)
    .modal(true)
    .default_width(800)
    .default_height(600)
    .title("Preferences")
    .child(&f)
    .build();

  open_button.connect_clicked(glib::clone!(
    #[weak]
    wnd,
    #[weak]
    textbox,
    #[weak]
    settings,
    move |_| {
      let dialog = FileDialog::builder()
        .title("Open File")
        .accept_label("Open")
        .build();

      dialog.open(Some(&*wnd), gio::Cancellable::NONE, move |file| {
        if let Ok(file) = file {
          let p = file.path().expect("Couldn't get file path");
          let folder = &p.to_string_lossy();
          textbox.set_text(folder);
          let mut s = settings.borrow_mut();
          s.folder = Some(folder.to_string());
          write_settings(&s).expect("Failed to write");
        }
      });
    }
  ));
  preferences_dialog.present();
}
