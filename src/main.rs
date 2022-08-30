mod application_row;

use crate::application_row::ApplicationRow;
use crate::application_row::Song;
use gtk::gio;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;

use std::cell::Ref;

fn main() {
  let app = gtk::Application::new(Some("com.github.fml9001"), Default::default());
  app.connect_activate(build_ui);
  app.run();
}

fn build_ui(application: &gtk::Application) {
  let window = gtk::ApplicationWindow::builder()
    .default_width(320)
    .default_height(480)
    .application(application)
    .title("fml9001")
    .build();

  let vbox = gtk::Box::new(gtk::Orientation::Vertical, 5);

  let store = gio::ListStore::new(BoxedAnyObject::static_type());

  let b1 = BoxedAnyObject::new(Song {
    name: "hej".to_string(),
  });
  let b2 = BoxedAnyObject::new(Song {
    name: "hupp".to_string(),
  });
  let b3 = BoxedAnyObject::new(Song {
    name: "hoop".to_string(),
  });
  store.append(&b1);
  store.append(&b2);
  store.append(&b3);
  let sel = gtk::SingleSelection::new(Some(&store));
  let listbox = gtk::ColumnView::new(Some(&sel));

  let factory = gtk::SignalListItemFactory::new();
  let col = gtk::ColumnViewColumn::new(Some("Artist"), Some(&factory));
  factory.connect_setup(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let row = ApplicationRow::new();
    item.set_child(Some(&row));
  });

  factory.connect_bind(move |_factory, item| {
    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
    let child = item.child().unwrap().downcast::<ApplicationRow>().unwrap();
    let app_info = item.item().unwrap().downcast::<BoxedAnyObject>().unwrap();
    let r: Ref<Song> = app_info.borrow();
    let song = Song {
      name: r.name.to_string(),
    };
    child.set_app_info(&song);
  });
  listbox.append_column(&col);

  let scrolled_window = gtk::ScrolledWindow::builder()
    .hscrollbar_policy(gtk::PolicyType::Never) // Disable horizontal scrolling
    .min_content_height(480)
    .min_content_width(360)
    .build();

  scrolled_window.set_child(Some(&listbox));
  vbox.append(&scrolled_window);

  window.set_child(Some(&vbox));
  window.show();
}
