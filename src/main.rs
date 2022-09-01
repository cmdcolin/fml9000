mod application_row;

use crate::application_row::ApplicationRow;
use crate::application_row::Entry;
use gtk::gio;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use jwalk::WalkDir;
use lofty::ItemKey::AlbumArtist;
use lofty::{Accessor, Probe};
use std::cell::Ref;

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

fn build_ui(application: &gtk::Application) {
  let window = gtk::ApplicationWindow::builder()
    .default_width(1200)
    .default_height(800)
    .application(application)
    .title("fml9001")
    .build();

  let vbox = gtk::Box::new(gtk::Orientation::Vertical, 5);

  let playlist_store = gio::ListStore::new(BoxedAnyObject::static_type());
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
  let playlist_listbox = gtk::ColumnView::new(Some(&playlist_sel));

  let facet_sel = gtk::SingleSelection::new(Some(&playlist_store));
  let facet_listbox = gtk::ColumnView::new(Some(&facet_sel));

  let artistalbum = gtk::SignalListItemFactory::new();
  let title = gtk::SignalListItemFactory::new();
  let filename = gtk::SignalListItemFactory::new();
  let facet = gtk::SignalListItemFactory::new();

  let playlist_col1 = gtk::ColumnViewColumn::new(Some("Artist / Album"), Some(&artistalbum));
  let playlist_col2 = gtk::ColumnViewColumn::new(Some("Title"), Some(&title));
  let playlist_col3 = gtk::ColumnViewColumn::new(Some("Filename"), Some(&filename));
  let facet_col = gtk::ColumnViewColumn::new(Some("X"), Some(&facet));

  playlist_listbox.append_column(&playlist_col1);
  playlist_listbox.append_column(&playlist_col2);
  playlist_listbox.append_column(&playlist_col3);
  facet_listbox.append_column(&facet_col);

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
    .min_content_height(480)
    .min_content_width(360)
    .build();

  let playlist_window = gtk::ScrolledWindow::builder()
    .min_content_height(480)
    .min_content_width(360)
    .build();

  facet_window.set_child(Some(&facet_listbox));
  playlist_window.set_child(Some(&playlist_listbox));

  vbox.append(&facet_window);
  vbox.append(&playlist_window);

  window.set_child(Some(&vbox));
  window.show();
}
