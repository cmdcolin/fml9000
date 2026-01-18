mod imp;
use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

glib::wrapper! {
    pub struct GridCell(ObjectSubclass<imp::GridCell>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
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
    glib::Object::new()
  }

  pub fn set_entry(&self, entry: &Entry) {
    self.imp().name.set_text(&entry.name);
  }

  pub fn set_row_height(&self, height: i32, compact: bool) {
    let label = &self.imp().name;
    label.set_height_request(height);
    if compact {
      label.set_margin_top(0);
      label.set_margin_bottom(0);
      label.set_margin_start(2);
      label.set_margin_end(2);
    } else {
      label.set_margin_top(2);
      label.set_margin_bottom(2);
      label.set_margin_start(4);
      label.set_margin_end(4);
    }
  }

  pub fn set_playlist_id(&self, id: Rc<RefCell<Option<i32>>>) {
    *self.imp().playlist_id.borrow_mut() = Some(id);
  }

  pub fn set_playlist_id_value(&self, value: Option<i32>) {
    if let Some(id_ref) = self.imp().playlist_id.borrow().as_ref() {
      *id_ref.borrow_mut() = value;
    }
  }

  pub fn set_playing(&self, playing: bool) {
    let label = &self.imp().name;
    if playing {
      label.add_css_class("playing");
    } else {
      label.remove_css_class("playing");
    }
  }
}
