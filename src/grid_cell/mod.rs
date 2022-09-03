mod imp;
use gtk::glib;
use gtk::subclass::prelude::*;

glib::wrapper! {
  pub struct GridCell(ObjectSubclass<imp::GridCell>)
    @extends gtk::Widget, gtk::Box;
}

impl Default for GridCell {
  fn default() -> Self {
    Self::new()
  }
}

pub struct Entry {
  pub name: String,
}

impl GridCell {
  pub fn new() -> Self {
    glib::Object::new(&[]).expect("Failed to create GridCell")
  }

  pub fn set_entry(&self, app_info: &Entry) {
    self.imp().name.set_text(&app_info.name);
  }
}
