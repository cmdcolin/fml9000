mod database;
mod grid_cell;
mod load_css;
mod play_track;

use crate::grid_cell::Entry;
use crate::grid_cell::GridCell;
use database::{Facet, Track};
use gtk::glib;
use gtk::glib::closure_local;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  gdk, gio, Application, ApplicationWindow, Box, Button, ColumnView, ColumnViewColumn, Image,
  ListItem, Paned, Scale, ScrolledWindow, SearchEntry, SignalListItemFactory, SingleSelection,
  VolumeButton,
};
use std::cell::Ref;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

struct Playlist {
  name: String,
}

fn main() {
  let app = Application::new(Some("com.github.fml9000"), Default::default());

  app.connect_activate(app_main);
  app.run();
}

fn setup_col(item: &ListItem) {
  item
    .downcast_ref::<ListItem>()
    .unwrap()
    .set_child(Some(&GridCell::new()));
}

fn get_obj(item: &ListItem) -> (GridCell, BoxedAnyObject) {
  let item = item.downcast_ref::<ListItem>().unwrap();
  let child = item.child().unwrap().downcast::<GridCell>().unwrap();
  let obj = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
  (child, obj)
}

fn app_main(application: &Application) {
  let (tx, rx) = mpsc::channel();
  let thread = thread::Builder::new()
    .name("music_player_thread".to_string())
    .spawn(move || match rx.recv() {
      Ok(received) => play_track::play_track(received),
      Err(e) => println!("{}", e),
    });

  let wnd = ApplicationWindow::builder()
    .default_width(1200)
    .default_height(600)
    .application(application)
    .title("fml9000")
    .build();

  // database::run_scan();
  load_css::load_css();

  let facet_store = gio::ListStore::new(BoxedAnyObject::static_type());
  let playlist_store = gio::ListStore::new(BoxedAnyObject::static_type());
  let playlist_mgr_store = gio::ListStore::new(BoxedAnyObject::static_type());

  let playlist_sel = SingleSelection::builder().model(&playlist_store).build();
  let playlist_columnview = ColumnView::builder().model(&playlist_sel).build();

  let facet_sel = SingleSelection::builder().model(&facet_store).build();
  let facet_columnview = ColumnView::builder().model(&facet_sel).build();

  let playlist_mgr_sel = SingleSelection::builder()
    .model(&playlist_mgr_store)
    .build();

  let playlist_mgr_columnview = ColumnView::builder().model(&playlist_mgr_sel).build();

  let artistalbum = SignalListItemFactory::new();
  let title = SignalListItemFactory::new();
  let filename = SignalListItemFactory::new();
  let track = SignalListItemFactory::new();
  let facet = SignalListItemFactory::new();
  let playlist_mgr = SignalListItemFactory::new();

  let playlist_col1 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(400)
    .title("Artist / Album")
    .factory(&artistalbum)
    .build();

  let playlist_col2 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("#")
    .fixed_width(20)
    .factory(&track)
    .build();

  let playlist_col3 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("Title")
    .fixed_width(300)
    .factory(&title)
    .build();

  let playlist_col4 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .fixed_width(200)
    .title("Filename")
    .factory(&filename)
    .build();

  let facet_col = ColumnViewColumn::builder()
    .title("X")
    .factory(&facet)
    .expand(true)
    .build();

  let playlist_mgr_col = ColumnViewColumn::builder()
    .title("Playlists")
    .factory(&playlist_mgr)
    .expand(true)
    .build();

  playlist_columnview.append_column(&playlist_col1);
  playlist_columnview.append_column(&playlist_col2);
  playlist_columnview.append_column(&playlist_col3);
  playlist_columnview.append_column(&playlist_col4);
  facet_columnview.append_column(&facet_col);
  playlist_mgr_columnview.append_column(&playlist_mgr_col);

  playlist_columnview.connect_activate(move |columnview, position| {
    let model = columnview.model().unwrap();
    let item = model
      .item(position)
      .unwrap()
      .downcast::<BoxedAnyObject>()
      .unwrap();
    let r: Ref<Track> = item.borrow();
    let f = r.filename.clone();
    tx.send(f.to_string());
  });

  // let five = Rc::new(playlist_store);
  // let cl = five.clone();
  // let cl2 = five.clone();

  facet_columnview.connect_activate(move |columnview, position| {
    let model = columnview.model().unwrap();
    let item = model
      .item(position)
      .unwrap()
      .downcast::<BoxedAnyObject>()
      .unwrap();
    let r: Ref<Facet> = item.borrow();
    // database::load_facet_db(&cl, &r);
    // let f = r.album;
    // playlist_columnview.set_model(new_model);
  });

  let rows = database::load_all().unwrap();
  database::load_stores(rows, &playlist_store, &facet_store);
  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently added".to_string(),
  }));
  playlist_mgr_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently played".to_string(),
  }));

  facet.connect_setup(move |_factory, item| setup_col(item));
  facet.connect_bind(move |_factory, item| {
    let (cell, obj) = get_obj(item);
    let r: Ref<Facet> = obj.borrow();
    cell.set_entry(&Entry {
      name: if r.all {
        "(All)".to_string()
      } else {
        format!(
          "{} / {}",
          r.album_artist.as_ref().unwrap_or(&"(Unknown)".to_string()),
          r.album.as_ref().unwrap_or(&"(Unknown)".to_string()),
        )
      },
    });
  });

  artistalbum.connect_setup(move |_factory, item| setup_col(item));
  artistalbum.connect_bind(move |_factory, item| {
    let (cell, obj) = get_obj(item);
    let r: Ref<Track> = obj.borrow();
    cell.set_entry(&Entry {
      name: format!(
        "{} / {}",
        r.album.as_ref().unwrap_or(&"".to_string()),
        r.artist.as_ref().unwrap_or(&"".to_string()),
      ),
    });
  });

  track.connect_setup(move |_factory, item| setup_col(item));
  track.connect_bind(move |_factory, item| {
    let (cell, obj) = get_obj(item);
    let r: Ref<Track> = obj.borrow();
    cell.set_entry(&Entry {
      name: format!("{}", r.track.as_ref().unwrap_or(&"".to_string()),),
    });
  });

  title.connect_setup(move |_factory, item| setup_col(item));
  title.connect_bind(move |_factory, item| {
    let (cell, obj) = get_obj(item);
    let r: Ref<Track> = obj.borrow();
    cell.set_entry(&Entry {
      name: format!("{}", r.title.as_ref().unwrap_or(&"".to_string())),
    });
  });

  filename.connect_setup(move |_factory, item| setup_col(item));
  filename.connect_bind(move |_factory, item| {
    let (cell, obj) = get_obj(item);
    let r: Ref<Track> = obj.borrow();
    cell.set_entry(&Entry {
      name: r.filename.to_string(),
    });
  });

  playlist_mgr.connect_setup(move |_factory, item| setup_col(item));
  playlist_mgr.connect_bind(move |_factory, item| {
    let (cell, obj) = get_obj(item);
    let r: Ref<Playlist> = obj.borrow();
    cell.set_entry(&Entry {
      name: r.name.to_string(),
    });
  });

  let facet_wnd = ScrolledWindow::builder()
    .child(&facet_columnview)
    .vexpand(true)
    .build();

  let facet_box = Box::new(gtk::Orientation::Vertical, 0);
  let search_bar = SearchEntry::builder().build();
  facet_box.append(&search_bar);
  facet_box.append(&facet_wnd);

  let playlist_wnd = ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build();

  let playlist_mgr_wnd = ScrolledWindow::builder()
    .child(&playlist_mgr_columnview)
    .build();

  let album_art = Image::builder().file("/home/cdiesh/wow.png").build();

  let ltopbottom = Paned::builder()
    .vexpand(true)
    .orientation(gtk::Orientation::Vertical)
    .start_child(&facet_box)
    .end_child(&playlist_wnd)
    .build();

  let rtopbottom = Paned::builder()
    .vexpand(true)
    .orientation(gtk::Orientation::Vertical)
    .start_child(&playlist_mgr_wnd)
    .end_child(&album_art)
    .build();

  let lrpane = Paned::builder()
    .hexpand(true)
    .orientation(gtk::Orientation::Horizontal)
    .start_child(&ltopbottom)
    .end_child(&rtopbottom)
    .build();

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/play.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let play_img = Image::new();
  play_img.set_from_pixbuf(Some(&pixbuf));

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/pause.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let pause_img = Image::new();
  pause_img.set_from_pixbuf(Some(&pixbuf));

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/next.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let next_img = Image::new();
  next_img.set_from_pixbuf(Some(&pixbuf));

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/prev.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let prev_img = Image::new();
  prev_img.set_from_pixbuf(Some(&pixbuf));

  let loader = gdk::gdk_pixbuf::PixbufLoader::with_type("svg").unwrap();
  loader.write(include_bytes!("img/stop.svg")).unwrap();
  loader.close().unwrap();
  let pixbuf = loader.pixbuf().unwrap();
  let stop_img = Image::new();
  stop_img.set_from_pixbuf(Some(&pixbuf));

  let play_btn = Button::builder().child(&play_img).build();
  let pause_btn = Button::builder().child(&pause_img).build();
  let next_btn = Button::builder().child(&next_img).build();
  let prev_btn = Button::builder().child(&prev_img).build();
  let stop_btn = Button::builder().child(&stop_img).build();
  let button_box = Box::new(gtk::Orientation::Horizontal, 0);
  let seek_slider = Scale::new(
    gtk::Orientation::Horizontal,
    Some(&gtk::Adjustment::new(0.0, 0.0, 1.0, 0.01, 0.0, 0.0)),
  );
  let volume_button = VolumeButton::new();
  seek_slider.set_hexpand(true);

  button_box.append(&seek_slider);
  button_box.append(&play_btn);
  button_box.append(&pause_btn);
  button_box.append(&prev_btn);
  button_box.append(&next_btn);
  button_box.append(&stop_btn);
  button_box.append(&volume_button);

  pause_btn.connect_closure(
    "clicked",
    false,
    closure_local!(move |button: Button| {
      // Set the label to "Hello World!" after the button has been clicked on
      button.set_label("Hello World!");
    }),
  );

  let main_ui = Box::new(gtk::Orientation::Vertical, 0);
  main_ui.append(&button_box);
  main_ui.append(&lrpane);
  wnd.set_child(Some(&main_ui));
  wnd.show();
}

#[macro_use]
extern crate time_test;
#[cfg(test)]
mod tests {
  use crate::database::load_facet_db;
  use crate::database::load_playlist_store_db;
  use gtk::gio;
  use gtk::glib::BoxedAnyObject;
  use gtk::prelude::*;

  #[test]
  fn load_playlist_store() {
    time_test!();
    let playlist_store = gio::ListStore::new(BoxedAnyObject::static_type());
    match load_playlist_store_db(&playlist_store) {
      Ok(_) => println!("h1"),
      Err(e) => println!("{}", e),
    };
    println!("{}", playlist_store.n_items());
    assert_eq!(playlist_store.n_items(), 30940);
  }

  #[test]
  fn load_facet() {
    time_test!();
    let playlist_store = gio::ListStore::new(BoxedAnyObject::static_type());
    match load_facet_db(&playlist_store) {
      Ok(_) => println!("h1"),
      Err(e) => println!("{}", e),
    };
    println!("{}", playlist_store.n_items());
    assert_eq!(playlist_store.n_items(), 1272);
  }
}
