mod facet_box;
mod grid_cell;
mod gtk_helpers;
mod header_bar;
mod load_css;
mod playlist_manager;
mod playlist_view;
mod preferences_dialog;
mod settings;

use facet_box::create_facet_box;
use fml9000::models::Track;
use fml9000::{load_facet_store, load_playlist_store, load_tracks, run_scan};
use grid_cell::Entry;
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{
  Application, ApplicationWindow, CustomFilter, Image, Notebook, Orientation, Paned,
  SignalListItemFactory,
};
use gtk_helpers::{create_widget, get_cell, setup_col};
use header_bar::create_header_bar;
use playlist_manager::create_playlist_manager;
use playlist_view::create_playlist_view;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use std::cell::{Ref, RefCell};
use std::rc::Rc;

struct Playlist {
  name: String,
}

const APP_ID: &str = "com.github.fml9000";

fn main() {
  let app = Application::builder().application_id(APP_ID).build();
  let (_stream, stream_handle) = OutputStream::try_default().unwrap();

  let stream_handle_rc = Rc::new(stream_handle);
  app.connect_activate(move |application| {
    app_main(&application, &stream_handle_rc);
  });
  app.run();
}

fn app_main(application: &Application, stream_handle: &Rc<OutputStreamHandle>) {
  let wnd = ApplicationWindow::builder()
    .default_width(1200)
    .default_height(600)
    .application(application)
    .title("fml9000")
    .build();

  let wnd_rc = Rc::new(wnd);
  let wnd_rc1 = wnd_rc.clone();
  let stream_handle_clone = stream_handle.clone();
  let sink_refcell_rc = Rc::new(RefCell::new(Sink::try_new(&stream_handle).unwrap()));
  let sink_refcell_rc1 = sink_refcell_rc.clone();

  let settings_rc = Rc::new(RefCell::new(crate::settings::read_settings()));

  load_css::load_css();

  let filter = CustomFilter::new(|_| true);

  let playlist_store = ListStore::new(BoxedAnyObject::static_type());
  let playlist_mgr_store = ListStore::new(BoxedAnyObject::static_type());

  let source = gtk::DragSource::new();
  source.connect_drag_begin(|_, _| {
    println!("k1");
  });

  source.connect_drag_end(|_, _, _| {
    println!("k2");
  });

  let album_art = Image::builder().vexpand(true).build();

  let album_art_rc = Rc::new(album_art);
  let album_art_rc1 = album_art_rc.clone();

  let playlist_store_rc = Rc::new(playlist_store);
  let playlist_store_rc2 = playlist_store_rc.clone();

  let rows_rc = Rc::new(load_tracks());
  let rows_rc1 = rows_rc.clone();
  let rows_rc2 = rows_rc.clone();

  use std::time::Instant;
  let now = Instant::now();

  {
    let s = settings_rc.borrow();
    match &s.folder {
      Some(folder) => {
        run_scan(&folder, &rows_rc2);
      }
      None => {}
    }
  }

  let elapsed = now.elapsed();
  println!("Elapsed: {:.2?}", elapsed);

  let facet_store = ListStore::new(BoxedAnyObject::static_type());
  load_playlist_store(rows_rc.iter(), &playlist_store_rc);
  load_facet_store(&rows_rc1, &facet_store);

  let playlist_wnd =
    create_playlist_view(&playlist_store_rc2, &sink_refcell_rc, &stream_handle_clone);
  let playlist_mgr_wnd = create_playlist_manager(&playlist_mgr_store);
  let facet_box = create_facet_box(&playlist_store_rc, &facet_store, &filter, &rows_rc);

  let ltopbottom = Paned::builder()
    .vexpand(true)
    .orientation(Orientation::Vertical)
    .start_child(&facet_box)
    .end_child(&playlist_wnd)
    .build();

  let rtopbottom = Paned::builder()
    .vexpand(true)
    .orientation(Orientation::Vertical)
    .start_child(&playlist_mgr_wnd)
    .end_child(&*album_art_rc)
    .build();

  let lrpane = Paned::builder()
    .hexpand(true)
    .orientation(Orientation::Horizontal)
    .start_child(&ltopbottom)
    .end_child(&rtopbottom)
    .build();

  let main_ui = gtk::Box::new(Orientation::Vertical, 0);
  let rss_ui = gtk::Box::new(Orientation::Vertical, 0);

  let button_box = create_header_bar(settings_rc, sink_refcell_rc1, &wnd_rc);

  main_ui.append(&button_box);
  main_ui.append(&lrpane);
  let notebook = Notebook::new();
  let lab1 = create_widget("Library");
  let lab2 = create_widget("RSS");
  notebook.append_page(&main_ui, Some(&lab1));
  notebook.append_page(&rss_ui, Some(&lab2));
  wnd_rc.set_child(Some(&notebook));
  wnd_rc.show();
}

// non working pause/play with spacebar
//   let pauseplay_action = SimpleAction::new("pauseplay", None);
//   pauseplay_action.connect_activate(|a, b| {
//     println!("pauseplay {:?} {:?}", a, b);
//   });
//   wnd_rc.add_action(&pauseplay_action);

//   let pauseplay_shortcut = ShortcutAction::parse_string("action(win.pauseplay)").unwrap();
//   pauseplay_action.connect_activate(|_, _| {});
//   let trigger = KeyvalTrigger::new(gdk::Key::space, gdk::ModifierType::empty());
//   let shortcut = Shortcut::builder()
//     .trigger(&trigger)
//     .action(&pauseplay_shortcut)
//     .build();
//   let shortcut_controller = gtk::ShortcutController::new();
//   shortcut_controller.add_shortcut(&shortcut);
//   shortcut_controller.connect_scope_notify(|_| {
//     println!("here");
//   });

//   shortcut_controller.connect_mnemonic_modifiers_notify(|_| {
//     println!("here2");
//   });
//   wnd_rc.add_controller(&shortcut_controller);
