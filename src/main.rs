mod chunked_iterator;
mod grid_cell;
mod play_track;

use crate::grid_cell::Entry;
use crate::grid_cell::GridCell;
use gdk::Display;
use gtk::glib;
use gtk::glib::closure_local;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  gdk, gio, Application, ApplicationWindow, Box, Button, ColumnView, ColumnViewColumn, CssProvider,
  Image, ListItem, Paned, Scale, ScrolledWindow, SignalListItemFactory, SingleSelection, Statusbar,
  StyleContext, VolumeButton,
};
use lofty::{Accessor, ItemKey, Probe};
use rusqlite::{Connection, Result, Transaction};
use std::cell::Ref;
use std::thread;
use walkdir::WalkDir;

struct Playlist {
  name: String,
}
struct Track {
  album_artist: Option<String>,
  album: Option<String>,
  artist: Option<String>,
  title: Option<String>,
  filename: String,
}
struct Facet {
  album_artist: Option<String>,
  album: Option<String>,
}
struct DbTrack<'a> {
  album_artist: Option<&'a str>,
  album: Option<&'a str>,
  artist: Option<&'a str>,
  title: Option<&'a str>,
  filename: &'a str,
}

fn main() {
  let app = Application::new(Some("com.github.fml9001"), Default::default());
  app.connect_activate(build_ui);
  app.run();
}

fn connect_db(args: rusqlite::OpenFlags) -> Result<Connection, rusqlite::Error> {
  let conn = Connection::open_with_flags("test.db", args)?;

  match conn.execute(
    "CREATE TABLE tracks (
        id INTEGER NOT NULL PRIMARY KEY,
        filename VARCHAR NOT NULL,
        title VARCHAR,
        artist VARCHAR,
        album VARCHAR,
        album_artist VARCHAR
      )",
    (),
  ) {
    Ok(_) => {
      println!("Created new DB")
    }
    Err(e) => {
      println!("{}", e)
    }
  }

  Ok(conn)
}

fn process_file(tx: &Transaction, path: &str) -> Result<(), rusqlite::Error> {
  let tagged_file = Probe::open(path)
    .expect("ERROR: Bad path provided!")
    .read(true);
  match tagged_file {
    Ok(tagged_file) => {
      let tag = match tagged_file.primary_tag() {
        Some(primary_tag) => Some(primary_tag),
        None => tagged_file.first_tag(),
      };

      match tag {
        Some(tag) => {
          let s = DbTrack {
            album_artist: tag.get_string(&ItemKey::AlbumArtist),
            artist: tag.artist(),
            album: tag.album(),
            title: tag.title(),
            filename: path,
          };

          tx.execute(
            "INSERT INTO tracks (filename,artist,album,album_artist,title) VALUES (?1, ?2, ?3, ?4, ?5)",
            (&path, &s.artist, &s.album, &s.album_artist, &s.title),
          )?;

          Ok(())
        }
        None => Ok(()),
      }
    }
    Err(_) => Ok(()),
  }
}

fn init_db() -> Result<Connection, rusqlite::Error> {
  let mut conn = connect_db(rusqlite::OpenFlags::default())?;
  let mut i = 0;
  let transaction_size = 20;

  for chunk in chunked_iterator::ChunkedIterator::new(
    WalkDir::new("/home/cdiesh/Music")
      .into_iter()
      .filter_map(|e| e.ok()),
    transaction_size,
  ) {
    let tx = conn.transaction()?;
    for file in chunk {
      if file.file_type().is_file() && i < 10000 {
        let path = file.path();
        process_file(&tx, &path.display().to_string())?;
        i = i + 1;
      }
    }
    tx.commit()?
  }
  Ok(conn)
}

fn load_playlist_store_db(store: &gio::ListStore) -> Result<(), rusqlite::Error> {
  let conn = connect_db(rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
  let mut stmt = conn.prepare("SELECT filename,title,artist,album_artist,album FROM tracks")?;
  let rows = stmt.query_map([], |row| {
    Ok(Track {
      filename: row.get(0)?,
      title: row.get(1)?,
      artist: row.get(2)?,
      album_artist: row.get(3)?,
      album: row.get(4)?,
    })
  })?;

  for t in rows {
    match t {
      Ok(t) => store.append(&BoxedAnyObject::new(t)),
      Err(e) => println!("{}", e),
    }
  }

  Ok(())
}

fn load_facet_db(store: &gio::ListStore) -> Result<(), rusqlite::Error> {
  let conn = connect_db(rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
  let mut stmt =
    conn.prepare("SELECT DISTINCT album,album_artist FROM tracks ORDER BY album_artist")?;
  let rows = stmt.query_map([], |row| {
    Ok(Facet {
      album: row.get(0)?,
      album_artist: row.get(1)?,
    })
  })?;

  for t in rows {
    match t {
      Ok(t) => store.append(&BoxedAnyObject::new(t)),
      Err(e) => println!("{}", e),
    }
  }

  Ok(())
}

fn run_scan() {
  thread::spawn(|| match init_db() {
    Ok(conn) => {
      println!("initialized");
    }
    Err(e) => {
      println!("{}", e);
    }
  });
}

fn load_css() {
  // Load the CSS file and add it to the provider
  let provider = CssProvider::new();
  provider.load_from_data(include_bytes!("style.css"));

  // Add the provider to the default screen
  StyleContext::add_provider_for_display(
    &Display::default().expect("Could not connect to a display."),
    &provider,
    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
  );
}

fn build_ui(application: &Application) {
  let provider = CssProvider::new();
  provider.load_from_data(include_bytes!("style.css"));

  let window = ApplicationWindow::builder()
    .default_width(1200)
    .default_height(600)
    .application(application)
    .title("fml9000")
    .build();

  load_css();

  // run_scan();

  let facet_store = gio::ListStore::new(BoxedAnyObject::static_type());
  let playlist_store = gio::ListStore::new(BoxedAnyObject::static_type());
  let playlist_manager_store = gio::ListStore::new(BoxedAnyObject::static_type());

  let playlist_sel = SingleSelection::builder().model(&playlist_store).build();
  let playlist_columnview = ColumnView::builder().model(&playlist_sel).build();

  let facet_sel = SingleSelection::builder().model(&facet_store).build();
  let facet_columnview = ColumnView::builder().model(&facet_sel).build();

  let playlist_manager_sel = SingleSelection::builder()
    .model(&playlist_manager_store)
    .build();

  let playlist_manager_columnview = ColumnView::builder().model(&playlist_manager_sel).build();

  let artistalbum = SignalListItemFactory::new();
  let title = SignalListItemFactory::new();
  let filename = SignalListItemFactory::new();
  let facet = SignalListItemFactory::new();
  let playlist_manager = SignalListItemFactory::new();

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
    .title("Title")
    .fixed_width(300)
    .factory(&title)
    .build();

  let playlist_col3 = ColumnViewColumn::builder()
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

  let playlist_manager_col = ColumnViewColumn::builder()
    .title("Playlist")
    .factory(&playlist_manager)
    .build();

  playlist_columnview.append_column(&playlist_col1);
  playlist_columnview.append_column(&playlist_col2);
  playlist_columnview.append_column(&playlist_col3);
  facet_columnview.append_column(&facet_col);
  playlist_manager_columnview.append_column(&playlist_manager_col);

  playlist_columnview.connect_activate(|columnview, position| {
    let model = columnview.model().unwrap();
    let item = model
      .item(position)
      .unwrap()
      .downcast::<BoxedAnyObject>()
      .unwrap();
    let r: Ref<Track> = item.borrow();
    let f = r.filename.clone();
    thread::spawn(move || play_track::play_track(&f));
  });

  load_playlist_store_db(&playlist_store);
  load_facet_db(&facet_store);
  playlist_manager_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently added".to_string(),
  }));
  playlist_manager_store.append(&BoxedAnyObject::new(Playlist {
    name: "Recently played".to_string(),
  }));

  facet.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let row = GridCell::new();
    item.set_child(Some(&row));
  });

  facet.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<GridCell>().unwrap();
    let obj = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Facet> = obj.borrow();
    child.set_entry(&Entry {
      name: format!(
        "{} / {}",
        r.album_artist.as_ref().unwrap_or(&"".to_string()),
        r.album.as_ref().unwrap_or(&"".to_string()),
      ),
    });
  });

  artistalbum.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let row = GridCell::new();
    item.set_child(Some(&row));
  });

  artistalbum.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<GridCell>().unwrap();
    let obj = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = obj.borrow();
    child.set_entry(&Entry {
      name: format!(
        "{} / {}",
        r.album.as_ref().unwrap_or(&"".to_string()),
        r.artist.as_ref().unwrap_or(&"".to_string()),
      ),
    });
  });

  title.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let row = GridCell::new();
    item.set_child(Some(&row));
  });

  title.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<GridCell>().unwrap();
    let obj = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = obj.borrow();
    child.set_entry(&Entry {
      name: format!("{}", r.title.as_ref().unwrap_or(&"".to_string())),
    });
  });

  filename.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let row = GridCell::new();
    item.set_child(Some(&row));
  });

  filename.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<GridCell>().unwrap();
    let obj = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = obj.borrow();
    child.set_entry(&Entry {
      name: r.filename.to_string(),
    });
  });

  let facet_window = ScrolledWindow::builder().child(&facet_columnview).build();

  let playlist_window = ScrolledWindow::builder()
    .child(&playlist_columnview)
    .build();

  let playlist_manager_window = ScrolledWindow::builder()
    .child(&playlist_manager_columnview)
    .build();

  let album_art = Image::builder().file("/home/cdiesh/wow.png").build();

  let ltopbottom = Paned::builder()
    .vexpand(true)
    .orientation(gtk::Orientation::Vertical)
    .start_child(&facet_window)
    .end_child(&playlist_window)
    .build();

  let rtopbottom = Paned::builder()
    .vexpand(true)
    .orientation(gtk::Orientation::Vertical)
    .start_child(&playlist_manager_window)
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
  let volume_slider = VolumeButton::new();
  seek_slider.set_hexpand(true);
  button_box.append(&play_btn);
  button_box.append(&pause_btn);
  button_box.append(&prev_btn);
  button_box.append(&next_btn);
  button_box.append(&stop_btn);
  button_box.append(&seek_slider);
  button_box.append(&volume_slider);

  pause_btn.connect_closure(
    "clicked",
    false,
    closure_local!(move |button: Button| {
      // Set the label to "Hello World!" after the button has been clicked on
      button.set_label("Hello World!");
    }),
  );

  let statusbar = Statusbar::new();
  let main_ui = Box::new(gtk::Orientation::Vertical, 0);
  main_ui.append(&button_box);
  main_ui.append(&lrpane);
  main_ui.append(&statusbar);
  window.set_child(Some(&main_ui));
  window.show();
}

#[macro_use]
extern crate time_test;
#[cfg(test)]
mod tests {
  use crate::load_facet_db;
  use crate::load_playlist_store_db;
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
