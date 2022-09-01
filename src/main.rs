mod application_row;
pub mod models;
pub mod schema;
use crate::application_row::ApplicationRow;
use crate::application_row::Entry;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenvy::dotenv;
use gtk::gio;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use jwalk::WalkDir;
use lofty::ItemKey::AlbumArtist;
use lofty::{Accessor, Probe};
use std::cell::Ref;
use std::env;

use crate::models::*;

struct Song {
  album_artist: String,
  album: String,
  artist: String,
  title: String,
  filename: String,
}

fn main() {
  let app = gtk::Application::new(Some("com.github.fml9001"), Default::default());
  app.connect_activate(build_ui);
  app.run();
}

fn query_db() {
  use self::schema::tracks::dsl::*;

  let connection = &mut establish_connection();
  let results = tracks
    .limit(5)
    .load::<Track>(connection)
    .expect("Error loading tracks");

  println!("Displaying {} tracks", results.len());
  for post in results {
    println!("{}", post.title);
    println!("-----------\n");
    println!("{}", post.artist);
  }
}

pub fn establish_connection() -> SqliteConnection {
  dotenv().ok();

  let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
  SqliteConnection::establish(&database_url)
    .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

fn build_ui(application: &gtk::Application) {
  let window = gtk::ApplicationWindow::builder()
    .default_width(1200)
    .default_height(600)
    .application(application)
    .title("fml9001")
    .build();

  let grid = gtk::Grid::builder().hexpand(true).vexpand(true).build();

  let facet_store = gio::ListStore::new(BoxedAnyObject::static_type());
  let playlist_store = gio::ListStore::new(BoxedAnyObject::static_type());
  let playlist_manager_store = gio::ListStore::new(BoxedAnyObject::static_type());

  let mut i = 0;
  for entry in WalkDir::new("/home/cdiesh/Music") {
    let ent = entry.unwrap();

    if ent.file_type().is_file() && i < 100 {
      let path = ent.path();
      let path2 = path.clone();

      // Use the default options for metadata and format readers.
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
              let b = BoxedAnyObject::new(Song {
                album_artist: tag.get_string(&AlbumArtist).unwrap_or("None").to_string(),
                artist: tag.artist().unwrap_or("None").to_string(),
                album: tag.album().unwrap_or("None").to_string(),
                title: tag.title().unwrap_or("None").to_string(),
                filename: path2.display().to_string(),
              });
              playlist_store.append(&b);
              i = i + 1;
            }
            None => {}
          }
        }
        Err(_) => {
          // println!("{} {}", e, path3.display());
        }
      }
    }
  }
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
    let r: Ref<Song> = app_info.borrow();
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
    let r: Ref<Song> = app_info.borrow();
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
    let r: Ref<Song> = app_info.borrow();
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
    let r: Ref<Song> = app_info.borrow();
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
