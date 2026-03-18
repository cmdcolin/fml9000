mod imp;

use gtk::glib;
use gtk::subclass::prelude::*;

glib::wrapper! {
  pub struct BrowseCard(ObjectSubclass<imp::BrowseCardImp>)
    @extends gtk::Box, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl BrowseCard {
  pub fn new() -> Self {
    glib::Object::new()
  }

  pub fn set_title(&self, title: &str) {
    self.imp().title_label.set_label(title);
  }

  pub fn set_subtitle(&self, subtitle: &str) {
    self.imp().subtitle_label.set_label(subtitle);
  }

  pub fn set_thumbnail_from_file(&self, path: &std::path::Path) {
    self.imp().thumbnail.set_filename(Some(path));
  }

  pub fn clear_thumbnail(&self) {
    self.imp().thumbnail.set_paintable(None::<&gtk::gdk::Texture>);
  }
}
