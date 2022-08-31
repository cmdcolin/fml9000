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
    .default_width(800)
    .default_height(600)
    .application(application)
    .title("fml9001")
    .build();

  let vbox = gtk::Box::new(gtk::Orientation::Vertical, 5);

  let store = gio::ListStore::new(BoxedAnyObject::static_type());
  let mut i = 0;
  for entry in WalkDir::new("/home/cdiesh/Music") {
    let ent = entry.unwrap();

    if ent.file_type().is_file() && i < 100 {
      let path = ent.path();
      let path2 = path.clone();
      let path3 = path.clone();

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
              let val = tag.get_string(&AlbumArtist).unwrap();
              let b = BoxedAnyObject::new(Song {
                album_artist: val.to_string(),
                artist: tag.artist().unwrap_or("None").to_string(),
                album: tag.album().unwrap_or("None").to_string(),
                title: tag.title().unwrap_or("None").to_string(),
                filename: path2.display().to_string(),
              });
              store.append(&b);
              i = i + 1;
            }
            None => {}
          }
        }
        Err(e) => {
          println!("{} {}", e, path3.display());
        }
      }
    }
  }
  let sel = gtk::SingleSelection::new(Some(&store));
  let listbox = gtk::ColumnView::new(Some(&sel));

  let artistalbum = gtk::SignalListItemFactory::new();
  let title = gtk::SignalListItemFactory::new();
  let filename = gtk::SignalListItemFactory::new();

  let col1 = gtk::ColumnViewColumn::new(Some("Artist / Album"), Some(&artistalbum));
  let col2 = gtk::ColumnViewColumn::new(Some("Title"), Some(&title));
  let col3 = gtk::ColumnViewColumn::new(Some("Filename"), Some(&filename));

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

  listbox.append_column(&col1);
  listbox.append_column(&col2);
  listbox.append_column(&col3);

  let scrolled_window = gtk::ScrolledWindow::builder()
    .min_content_height(480)
    .min_content_width(360)
    .build();

  scrolled_window.set_child(Some(&listbox));
  vbox.append(&scrolled_window);

  window.set_child(Some(&vbox));
  window.show();
}
