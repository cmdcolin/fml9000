mod imp;
use gtk::glib;
use gtk::subclass::prelude::*;

glib::wrapper! {
  pub struct GridCell(ObjectSubclass<imp::GridCell>)
    @extends gtk::Widget;
}

impl Default for GridCell {
  fn default() -> Self {
    Self::new()
  }
}

pub struct GridEntry {
  pub name: String,
}

impl GridCell {
  pub fn new() -> Self {
    glib::Object::new(&[]).expect("Failed to create GridCell")
  }

  pub fn set_entry(&self, entry: &GridEntry) {
    self.imp().name.set_text(&entry.name);
  }
}
