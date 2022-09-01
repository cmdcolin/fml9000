mod application_row;

use crate::application_row::ApplicationRow;
use crate::application_row::Entry;
use gtk::gio;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use lofty::ItemKey::AlbumArtist;
use lofty::{Accessor, Probe};
use rusqlite::{Connection, Result, Transaction};
use std::cell::Ref;
use std::fs;
use walkdir::WalkDir;

struct Track {
  album_artist: String,
  album: String,
  artist: String,
  title: String,
  filename: String,
}
struct DbTrack<'a> {
  album_artist: Option<&'a str>,
  album: Option<&'a str>,
  artist: Option<&'a str>,
  title: Option<&'a str>,
  filename: &'a str,
}

fn main() {
  match init_db() {
    Ok(conn) => {
      println!("initialized");

      match print_db(&conn) {
        Ok(_) => {
          println!("printed");
        }
        Err(e) => {
          println!("{}", e);
        }
      }
    }
    Err(e) => {
      println!("{}", e);
    }
  }
}

fn connect_db() -> Result<Connection, rusqlite::Error> {
  let conn = Connection::open_with_flags("test.db", rusqlite::OpenFlags::default())?;

  conn.execute(
    "CREATE TABLE tracks (
        id INTEGER NOT NULL PRIMARY KEY,
        filename VARCHAR NOT NULL,
        title VARCHAR,
        artist VARCHAR,
        album VARCHAR,
        album_artist VARCHAR
      )",
    (), // empty list of parameters.
  )?;

  Ok(conn)
}

struct ChunkedIterator<T, R>
where
  T: Iterator<Item = R>,
{
  source: T,
  inner: Vec<R>,
  size: usize,
}
impl<T, R> ChunkedIterator<T, R>
where
  T: Iterator<Item = R>,
{
  fn new(source: T, size: usize) -> Self {
    ChunkedIterator {
      size: size - 1,
      inner: vec![],
      source,
    }
  }
}
impl<T, R> Iterator for ChunkedIterator<T, R>
where
  T: Iterator<Item = R>,
{
  type Item = Vec<R>;

  fn next(&mut self) -> Option<Vec<R>> {
    while let inner_opt = self.source.next() {
      match inner_opt {
        Some(inner_item) => {
          self.inner.push(inner_item);
          if self.inner.len() > self.size {
            return Some(self.inner.split_off(0));
          }
        }
        None => match self.inner.len() {
          0 => return None,
          _ => return Some(self.inner.split_off(0)),
        },
      }
    }
    None
  }
}

fn process_file<'a>(path: &str) -> Result<(), rusqlite::Error> {
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
          println!(
            "{} {}",
            tag.artist().unwrap_or("None"),
            tag.album().unwrap_or("None")
          );
          // tx.execute(
          //   "INSERT INTO tracks (filename,artist,album,album_artist,title) VALUES (?1, ?2, ?3, ?4, ?5)",
          //   (&path, &s.artist, &s.album, &s.album_artist, &s.title),
          // )?;

          Ok(())
        }
        None => Ok(()),
      }
    }
    Err(_) => Ok(()),
  }
}

fn init_db() -> Result<Connection, rusqlite::Error> {
  let mut conn = connect_db()?;
  let mut i = 0;
  let transaction_size = 10000;

  for chunk in ChunkedIterator::new(
    WalkDir::new("/home/cdiesh/Music")
      .into_iter()
      .filter_map(|e| e.ok()),
    transaction_size,
  ) {
    // let tx = conn.transaction()?;
    for file in chunk {
      if file.file_type().is_file() && i < 10000 {
        let path = file.path();
        process_file(&path.display().to_string())?;
        i = i + 1;
        println!("{} {}", i, path.display());
      }
    }
    // tx.commit()?
  }
  Ok(conn)
}

#[derive(Debug)]
struct Temp {
  filename: String,
  title: String,
}

fn print_db(conn: &Connection) -> Result<(), rusqlite::Error> {
  let mut stmt = conn.prepare("SELECT filename,title FROM tracks")?;
  let track_iter = stmt.query_map([], |row| {
    Ok(Temp {
      filename: row.get(0)?,
      title: row.get(1)?,
    })
  })?;

  for track in track_iter {
    match track {
      Ok(t) => println!("Found track {:?}", t),
      Err(e) => println!("{}", e),
    }
  }

  Ok(())
}

fn build_ui(application: &gtk::Application) {
  let window = gtk::ApplicationWindow::builder()
    .default_width(1200)
    .default_height(600)
    .application(application)
    .title("fml9000")
    .build();

  match init_db() {
    Ok(conn) => {
      println!("initialized");

      match print_db(&conn) {
        Ok(_) => {
          println!("printed");
        }
        Err(e) => {
          println!("{}", e);
        }
      }
    }
    Err(e) => {
      println!("{}", e);
    }
  }

  let grid = gtk::Grid::builder().hexpand(true).vexpand(true).build();

  let facet_store = gio::ListStore::new(BoxedAnyObject::static_type());
  let playlist_store = gio::ListStore::new(BoxedAnyObject::static_type());
  let playlist_manager_store = gio::ListStore::new(BoxedAnyObject::static_type());

  let playlist_sel = gtk::SingleSelection::new(Some(&playlist_store));
  let playlist_columnview = gtk::ColumnView::new(Some(&playlist_sel));

  let facet_sel = gtk::SingleSelection::new(Some(&facet_store));
  let facet_columnview = gtk::ColumnView::new(Some(&facet_sel));

  let playlist_manager_sel = gtk::SingleSelection::new(Some(&playlist_manager_store));
  let playlist_manager_columnview = gtk::ColumnView::new(Some(&playlist_manager_sel));

  let artistalbum = gtk::SignalListItemFactory::new();
  let title = gtk::SignalListItemFactory::new();
  let filename = gtk::SignalListItemFactory::new();
  let facet = gtk::SignalListItemFactory::new();
  let playlist_manager = gtk::SignalListItemFactory::new();

  let playlist_col1 = gtk::ColumnViewColumn::new(Some("Artist / Album"), Some(&artistalbum));
  let playlist_col2 = gtk::ColumnViewColumn::new(Some("Title"), Some(&title));
  let playlist_col3 = gtk::ColumnViewColumn::new(Some("Filename"), Some(&filename));
  let facet_col = gtk::ColumnViewColumn::new(Some("X"), Some(&facet));
  let playlist_manager_col = gtk::ColumnViewColumn::new(Some("Playlists"), Some(&playlist_manager));

  playlist_columnview.append_column(&playlist_col1);
  playlist_columnview.append_column(&playlist_col2);
  playlist_columnview.append_column(&playlist_col3);
  facet_columnview.append_column(&facet_col);
  playlist_manager_columnview.append_column(&playlist_manager_col);

  facet.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let row = ApplicationRow::new();
    item.set_child(Some(&row));
  });

  facet.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<ApplicationRow>().unwrap();
    let app_info = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = app_info.borrow();
    let song = Entry {
      name: format!("{} / {}", r.album_artist, r.album),
    };
    child.set_entry(&song);
  });

  artistalbum.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let row = ApplicationRow::new();
    item.set_child(Some(&row));
  });

  artistalbum.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<ApplicationRow>().unwrap();
    let app_info = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = app_info.borrow();
    let song = Entry {
      name: format!("{} / {}", r.album, r.artist),
    };
    child.set_entry(&song);
  });

  title.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let row = ApplicationRow::new();
    item.set_child(Some(&row));
  });

  title.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<ApplicationRow>().unwrap();
    let app_info = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = app_info.borrow();
    let song = Entry {
      name: r.title.to_string(),
    };
    child.set_entry(&song);
  });

  filename.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let row = ApplicationRow::new();
    item.set_child(Some(&row));
  });

  filename.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<ApplicationRow>().unwrap();
    let app_info = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Track> = app_info.borrow();
    let song = Entry {
      name: r.filename.to_string(),
    };
    child.set_entry(&song);
  });

  let facet_window = gtk::ScrolledWindow::builder()
    .min_content_height(390)
    .min_content_width(600)
    .build();

  let playlist_window = gtk::ScrolledWindow::builder()
    .min_content_height(390)
    .min_content_width(600)
    .build();

  let playlist_manager_window = gtk::ScrolledWindow::builder()
    .min_content_width(600)
    .min_content_height(390)
    .build();

  let album_art = gtk::Image::builder().file("/home/cdiesh/wow.png").build();

  facet_window.set_child(Some(&facet_columnview));
  playlist_window.set_child(Some(&playlist_columnview));
  playlist_manager_window.set_child(Some(&playlist_manager_columnview));

  grid.attach(&facet_window, 0, 0, 1, 1);
  grid.attach(&playlist_window, 0, 1, 1, 1);
  grid.attach(&playlist_manager_window, 1, 0, 1, 1);
  grid.attach(&album_art, 1, 1, 1, 1);

  window.set_child(Some(&grid));
  window.show();
}
