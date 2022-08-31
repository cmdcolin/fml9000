mod imp;
use gtk::glib;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct ApplicationRow(ObjectSubclass<imp::ApplicationRow>)
        @extends gtk::Widget, gtk::Box;
}

impl Default for ApplicationRow {
  fn default() -> Self {
    Self::new()
  }
}

pub struct Entry {
  pub name: String,
}

impl ApplicationRow {
  pub fn new() -> Self {
    glib::Object::new(&[]).expect("Failed to create ApplicationRow")
  }

  pub fn set_entry(&self, app_info: &Entry) {
    let imp = self.imp();
    imp.name.set_text(&app_info.name);
  }
}
