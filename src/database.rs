use gtk::gio;
use mpd::{Client, Query};
use std::error;
use std::rc::Rc;

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

pub fn add_track_to_recently_played(path: &str) -> Result<(), Box<dyn error::Error>> {
  Ok(())
}

pub fn load_all() -> Result<Vec<Rc<Track>>, Box<dyn error::Error>> {
  let mut c = Client::connect("127.0.0.1:6600").unwrap();
  let mut query = Query::new();
  let query = query.and(mpd::Term::Any, "Trapdoor");
  let songs = c.find(query, None);
  println!("stuff: {:?}", songs);
  let vec = Vec::new();

  Ok(vec)
}

pub fn load_playlist_store<'a, I>(vals: I, store: &gio::ListStore)
where
  I: Iterator<Item = &'a Rc<Track>>,
{
}

pub fn load_facet_store(rows: &[Rc<Track>], facet_store: &gio::ListStore) {}
