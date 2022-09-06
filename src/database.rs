mod chunked_iterator;

use gtk::gio;
use gtk::glib::BoxedAnyObject;
use lofty::{Accessor, ItemKey, Probe};
use rusqlite::{Connection, Result, Transaction};
use std::thread;
use walkdir::WalkDir;

pub struct Track {
  pub album_artist: Option<String>,
  pub album: Option<String>,
  pub artist: Option<String>,
  pub track: Option<String>,
  pub title: Option<String>,
  pub genre: Option<String>,
  pub filename: String,
}

pub struct Facet {
  pub album_artist: Option<String>,
  pub album: Option<String>,
  pub all: bool,
}

pub fn connect_db(args: rusqlite::OpenFlags) -> Result<Connection, rusqlite::Error> {
  let conn = Connection::open_with_flags("test.db", args)?;

  match conn.execute(
    "CREATE TABLE tracks (
        id INTEGER NOT NULL PRIMARY KEY,
        filename VARCHAR NOT NULL,
        title VARCHAR,
        artist VARCHAR,
        track VARCHAR,
        album VARCHAR,
        genre VARCHAR,
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

pub fn process_file(tx: &Transaction, path: &str) -> Result<(), rusqlite::Error> {
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
          tx.execute(
            "INSERT INTO tracks (filename,track,artist,album,album_artist,title,genre) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            (&path, tag.track(), tag.artist(), tag.album(), tag.get_string(&ItemKey::AlbumArtist), tag.title(), tag.genre()),
          )?;

          Ok(())
        }
        None => Ok(()),
      }
    }
    Err(_) => Ok(()),
  }
}

pub fn init_db() -> Result<Connection, rusqlite::Error> {
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

pub fn load_playlist_store_db(store: &gio::ListStore) -> Result<(), rusqlite::Error> {
  let conn = connect_db(rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
  let mut stmt =
    conn.prepare("SELECT filename,title,artist,album_artist,album,genre,track FROM tracks")?;
  let rows = stmt.query_map([], |row| {
    Ok(Track {
      filename: row.get(0)?,
      title: row.get(1)?,
      artist: row.get(2)?,
      album_artist: row.get(3)?,
      album: row.get(4)?,
      genre: row.get(5)?,
      track: row.get(6)?,
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

pub fn load_facet_db(store: &gio::ListStore) -> Result<(), rusqlite::Error> {
  let conn = connect_db(rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
  let mut stmt =
    conn.prepare("SELECT DISTINCT album,album_artist FROM tracks ORDER BY album_artist")?;
  let rows = stmt.query_map([], |row| {
    Ok(Facet {
      album: row.get(0)?,
      album_artist: row.get(1)?,
      all: false,
    })
  })?;

  store.append(&BoxedAnyObject::new(Facet {
    album: None,
    album_artist: None,
    all: true,
  }));

  for t in rows {
    store.append(&BoxedAnyObject::new(t.unwrap()))
  }

  Ok(())
}

pub fn run_scan() {
  thread::spawn(|| match init_db() {
    Ok(conn) => {
      println!("initialized");
    }
    Err(e) => {
      println!("{}", e);
    }
  });
}
