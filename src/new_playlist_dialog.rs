use crate::playback_controller::PlaybackController;
use fml9000::create_playlist;
use gtk::prelude::*;
use std::rc::Rc;

pub fn show_dialog(
  playback_controller: Rc<PlaybackController>,
  _dropped_data: String,
  on_playlist_created: impl Fn(i32) + 'static,
) {
  let wnd = playback_controller.window();

  let dialog = gtk::Window::builder()
    .title("New Playlist")
    .default_width(350)
    .default_height(150)
    .modal(true)
    .transient_for(&**wnd)
    .build();

  let content = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .spacing(12)
    .margin_top(24)
    .margin_bottom(24)
    .margin_start(24)
    .margin_end(24)
    .build();

  let name_entry = gtk::Entry::builder()
    .placeholder_text("Playlist name")
    .hexpand(true)
    .build();

  let status_label = gtk::Label::builder()
    .label("")
    .css_classes(["dim-label"])
    .wrap(true)
    .xalign(0.0)
    .build();

  let button_box = gtk::Box::builder()
    .orientation(gtk::Orientation::Horizontal)
    .spacing(12)
    .halign(gtk::Align::End)
    .build();

  let cancel_btn = gtk::Button::builder().label("Cancel").build();

  let create_btn = gtk::Button::builder()
    .label("Create")
    .css_classes(["suggested-action"])
    .sensitive(false)
    .build();

  button_box.append(&cancel_btn);
  button_box.append(&create_btn);

  content.append(&name_entry);
  content.append(&status_label);
  content.append(&button_box);

  dialog.set_child(Some(&content));

  let create_btn_clone = create_btn.clone();
  name_entry.connect_changed(move |entry| {
    let text = entry.text();
    create_btn_clone.set_sensitive(!text.is_empty());
  });

  let dialog_weak = dialog.downgrade();
  cancel_btn.connect_clicked(move |_| {
    if let Some(d) = dialog_weak.upgrade() {
      d.close();
    }
  });

  let dialog_weak = dialog.downgrade();
  let name_entry_clone = name_entry.clone();
  let status_label_clone = status_label.clone();
  create_btn.connect_clicked(move |_| {
    let name = name_entry_clone.text().to_string();

    match create_playlist(&name) {
      Ok(playlist_id) => {
        on_playlist_created(playlist_id);
        if let Some(d) = dialog_weak.upgrade() {
          d.close();
        }
      }
      Err(e) => {
        status_label_clone.set_label(&format!("Error: {e}"));
      }
    }
  });

  dialog.present();
}
