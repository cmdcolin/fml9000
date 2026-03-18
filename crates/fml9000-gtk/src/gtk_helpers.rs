use crate::grid_cell::GridCell;
use adw::prelude::*;
use fml9000_core::Track;
use gtk::glib::{BoxedAnyObject, Object};
use gtk::{AlertDialog, ApplicationWindow, ListItem, MultiSelection};

pub fn show_error_dialog(window: &ApplicationWindow, title: &str, message: &str) {
  let dialog = AlertDialog::builder()
    .modal(true)
    .message(title)
    .detail(message)
    .buttons(["OK"])
    .build();
  dialog.show(Some(window));
}

const UNKNOWN: &str = "(Unknown)";

pub fn str_or_unknown(s: &Option<String>) -> String {
  s.as_deref().unwrap_or(UNKNOWN).to_string()
}

pub fn get_album_artist_or_artist(track: &Track) -> Option<String> {
  track.album_artist.clone().or_else(|| track.artist.clone())
}

pub fn setup_col(item: &Object) {
  item
    .downcast_ref::<ListItem>()
    .unwrap()
    .set_child(Some(&GridCell::new()));
}

pub fn get_cell(item: &Object) -> (GridCell, BoxedAnyObject) {
  let item = item.downcast_ref::<ListItem>().unwrap();
  let child = item.child().unwrap().downcast::<GridCell>().unwrap();
  let obj = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
  (child, obj)
}

pub fn get_selection(sel: &MultiSelection, pos: u32) -> BoxedAnyObject {
  sel.item(pos).unwrap().downcast::<BoxedAnyObject>().unwrap()
}
