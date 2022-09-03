mod chunked_iterator;
mod grid_cell;

use crate::grid_cell::Entry;
use crate::grid_cell::GridCell;
use gdk::Display;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  gdk, gio, Application, ApplicationWindow, ColumnView, ColumnViewColumn, CssProvider, Image,
  ListItem, Paned, ScrolledWindow, SignalListItemFactory, SingleSelection, StyleContext,
};
use lofty::ItemKey::AlbumArtist;
use lofty::{Accessor, Probe};
use rusqlite::{Connection, Result, Transaction};
use std::cell::Ref;
use std::thread;
use walkdir::WalkDir;

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

fn process_file<'a>(tx: &Transaction, path: &str) -> Result<(), rusqlite::Error> {
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
            album_artist: tag.get_string(&AlbumArtist),
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

  let playlist_sel = SingleSelection::new(Some(&playlist_store));
  let playlist_columnview = ColumnView::new(Some(&playlist_sel));

  let facet_sel = SingleSelection::new(Some(&facet_store));
  let facet_columnview = ColumnView::new(Some(&facet_sel));

  let playlist_manager_sel = SingleSelection::new(Some(&playlist_manager_store));
  let playlist_manager_columnview = ColumnView::new(Some(&playlist_manager_sel));

  let artistalbum = SignalListItemFactory::new();
  let title = SignalListItemFactory::new();
  let filename = SignalListItemFactory::new();
  let facet = SignalListItemFactory::new();
  let playlist_manager = SignalListItemFactory::new();

  let playlist_col1 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("Artist / Album")
    .factory(&artistalbum)
    .build();

  let playlist_col2 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("Title")
    .factory(&title)
    .build();

  let playlist_col3 = ColumnViewColumn::builder()
    .expand(false)
    .resizable(true)
    .title("Filename")
    .factory(&filename)
    .build();

  let facet_col = ColumnViewColumn::new(Some("X"), Some(&facet));
  let playlist_manager_col = ColumnViewColumn::new(Some("Playlists"), Some(&playlist_manager));

  playlist_columnview.append_column(&playlist_col1);
  playlist_columnview.append_column(&playlist_col2);
  playlist_columnview.append_column(&playlist_col3);
  facet_columnview.append_column(&facet_col);
  playlist_manager_columnview.append_column(&playlist_manager_col);

  load_playlist_store_db(&playlist_store);
  load_facet_db(&facet_store);

  facet.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let row = GridCell::new();
    item.set_child(Some(&row));
  });

  facet.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<GridCell>().unwrap();
    let app_info = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Facet> = app_info.borrow();
    let song = Entry {
      name: format!(
        "{} / {}",
        r.album_artist.as_ref().unwrap_or(&"".to_string()),
        r.album.as_ref().unwrap_or(&"".to_string()),
      ),
    };
    child.set_entry(&song);
  });

  artistalbum.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let row = GridCell::new();
    item.set_child(Some(&row));
  });

  artistalbum.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<GridCell>().unwrap();
    let app_info = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = app_info.borrow();
    let song = Entry {
      name: format!(
        "{} / {}",
        r.album.as_ref().unwrap_or(&"".to_string()),
        r.artist.as_ref().unwrap_or(&"".to_string()),
      ),
    };
    child.set_entry(&song);
  });

  title.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let row = GridCell::new();
    item.set_child(Some(&row));
  });

  title.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<GridCell>().unwrap();
    let app_info = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = app_info.borrow();
    let song = Entry {
      name: format!("{}", r.title.as_ref().unwrap_or(&"".to_string())),
    };
    child.set_entry(&song);
  });

  filename.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let row = GridCell::new();
    item.set_child(Some(&row));
  });

  filename.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<GridCell>().unwrap();
    let app_info = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = app_info.borrow();
    let song = Entry {
      name: r.filename.to_string(),
    };
    child.set_entry(&song);
  });

  let facet_window = ScrolledWindow::builder().build();
  let playlist_window = ScrolledWindow::builder().build();
  let playlist_manager_window = ScrolledWindow::builder().build();

  let album_art = Image::builder().file("/home/cdiesh/wow.png").build();

  facet_window.set_child(Some(&facet_columnview));
  playlist_window.set_child(Some(&playlist_columnview));
  playlist_manager_window.set_child(Some(&playlist_manager_columnview));

  let lrpane = Paned::builder()
    .hexpand(true)
    .orientation(gtk::Orientation::Horizontal)
    .build();

  let ltopbottom = Paned::builder()
    .vexpand(true)
    .orientation(gtk::Orientation::Vertical)
    .build();

  let rtopbottom = Paned::builder()
    .vexpand(true)
    .orientation(gtk::Orientation::Vertical)
    .build();

  ltopbottom.set_start_child(Some(&facet_window));
  ltopbottom.set_end_child(Some(&playlist_window));
  rtopbottom.set_start_child(Some(&playlist_manager_window));
  rtopbottom.set_end_child(Some(&album_art));
  lrpane.set_start_child(Some(&ltopbottom));
  lrpane.set_end_child(Some(&rtopbottom));

  window.set_child(Some(&lrpane));
  window.show();
}
