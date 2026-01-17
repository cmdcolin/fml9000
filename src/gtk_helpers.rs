use crate::grid_cell::GridCell;
use adw::prelude::*;
use fml9000::models::Track;
use gtk::gdk;
use gtk::glib::{BoxedAnyObject, Bytes, Object};
use gtk::{Button, Image, ListItem, MultiSelection, SelectionModel};

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

pub fn get_playlist_activate_selection(sel: &SelectionModel, pos: u32) -> BoxedAnyObject {
  sel.item(pos).unwrap().downcast::<BoxedAnyObject>().unwrap()
}

pub fn load_img(svg_data: &'static [u8]) -> Image {
  let bytes = Bytes::from_static(svg_data);
  let texture = gdk::Texture::from_bytes(&bytes).unwrap();
  Image::builder().paintable(&texture).build()
}

pub fn create_button(img: &Image) -> Button {
  let button = Button::builder().child(img).build();
  button.add_css_class("flat");
  button
}
