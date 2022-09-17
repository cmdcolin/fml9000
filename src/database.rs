mod chunked_iterator;

use gtk::gio;
use gtk::glib::BoxedAnyObject;
use lofty::{Accessor, ItemKey, Probe};
use rusqlite::{Connection, Result, Transaction};
use std::collections::HashSet;
use std::rc::Rc;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct Track {
  pub album_artist: Option<String>,
  pub album: Option<String>,
  pub artist: Option<String>,
  pub track: Option<String>,
  pub title: Option<String>,
  pub genre: Option<String>,
  pub filename: String,
}

#[derive(Hash, Eq, Ord, PartialEq, PartialOrd, Debug)]
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
    Ok(_) => println!("Created new DB"),
    Err(e) => eprintln!("{}", e),
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

pub fn load_all() -> Result<Vec<Rc<Track>>, rusqlite::Error> {
  let conn = connect_db(rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
  let mut stmt =
    conn.prepare("SELECT filename,title,artist,album_artist,album,genre,track FROM tracks")?;

  let mut names = Vec::new();
  let rows = stmt.query_map([], |row| {
    Ok(Rc::new(Track {
      filename: row.get(0)?,
      title: row.get(1)?,
      artist: row.get(2)?,
      album_artist: row.get(3)?,
      album: row.get(4)?,
      genre: row.get(5)?,
      track: row.get(6)?,
    }))
  })?;
  for row in rows {
    names.push(row.unwrap());
  }

  Ok(names)
}

pub fn load_playlist_store<'a, I>(vals: I, store: &gio::ListStore)
where
  I: Iterator<Item = &'a Rc<Track>>,
{
  for row in vals {
    store.append(&BoxedAnyObject::new(row.clone()));
  }
}

pub fn load_facet_store(rows: &[Rc<Track>], facet_store: &gio::ListStore) {
  let mut facets = HashSet::new();
  for row in rows {
    facets.insert(Facet {
      album: row.album.clone(),
      album_artist: row.album_artist.clone(),
      all: false,
    });
  }
  facet_store.append(&BoxedAnyObject::new(Facet {
    album: None,
    album_artist: None,
    all: true,
  }));
  let mut v = Vec::from_iter(facets);
  v.sort();
  for uniq in v {
    facet_store.append(&BoxedAnyObject::new(uniq))
  }
}

pub fn run_scan() {
  init_db();
}
