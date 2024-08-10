use crate::grid_cell::GridCell;
use adw::prelude::*;
use fml9000::models::Track;
use gtk::gdk;
use gtk::glib::{BoxedAnyObject, Bytes, Object};
use gtk::{Button, Image, ListItem, MultiSelection, SelectionModel};

pub fn str_or_unknown(str: &Option<String>) -> String {
  str.as_ref().unwrap_or(&"(Unknown)".to_string()).to_string()
}

pub fn get_album_artist_or_artist(track: &Track) -> Option<String> {
  return track.album_artist.clone().or(track.artist.clone());
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

pub fn load_img(a: &'static [u8]) -> Image {
  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(&a).unwrap();
  loader.close().unwrap();

  let bytes = Bytes::from_static(&a);
  let logo = gdk::Texture::from_bytes(&bytes).unwrap();
  let img = Image::builder().paintable(&logo).build();
  img
}

pub fn create_button(img: &Image) -> Button {
  Button::builder().child(img).build()
}
