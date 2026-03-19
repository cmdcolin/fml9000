mod imp;

use gtk::glib;
use gtk::prelude::*;
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

  pub fn set_loading(&self, loading: bool) {
    let spinner = &self.imp().loading_spinner;
    spinner.set_visible(loading);
    spinner.set_spinning(loading);
    if loading {
      self.imp().playing_icon.set_visible(false);
      self.add_css_class("browse-card-active");
    }
  }

  pub fn set_playing(&self, playing: bool) {
    self.imp().playing_icon.set_visible(playing);
    self.imp().loading_spinner.set_visible(false);
    self.imp().loading_spinner.set_spinning(false);
    if playing {
      self.add_css_class("browse-card-active");
    } else {
      self.remove_css_class("browse-card-active");
    }
  }

  pub fn clear_state(&self) {
    self.imp().playing_icon.set_visible(false);
    self.imp().loading_spinner.set_visible(false);
    self.imp().loading_spinner.set_spinning(false);
    self.remove_css_class("browse-card-active");
  }
}
