mod facet_box;
mod grid_cell;
mod gtk_helpers;
mod header_bar;
mod load_css;
mod playlist_manager;
mod playlist_view;
mod preferences_dialog;
mod settings;

use adw::prelude::*;
use adw::Application;
use facet_box::create_facet_box;
use fml9000::{load_facet_store, load_playlist_store, load_tracks, run_scan};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::{AlertDialog, ApplicationWindow, CustomFilter, Image, Orientation, Paned};
use header_bar::create_header_bar;
use playlist_manager::create_playlist_manager;
use playlist_view::create_playlist_view;
use rodio::{OutputStream, Sink};
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "com.github.fml9000";

pub struct AudioState {
  _stream: OutputStream,
  pub sink: Sink,
}

fn init_audio() -> Result<AudioState, String> {
  let (stream, handle) = OutputStream::try_default()
    .map_err(|e| format!("Failed to initialize audio output: {e}"))?;
  let sink =
    Sink::try_new(&handle).map_err(|e| format!("Failed to create audio sink: {e}"))?;
  Ok(AudioState {
    _stream: stream,
    sink,
  })
}

fn show_error_dialog(window: &ApplicationWindow, title: &str, message: &str) {
  let dialog = AlertDialog::builder()
    .modal(true)
    .message(title)
    .detail(message)
    .buttons(["OK"])
    .build();
  dialog.show(Some(window));
}

fn main() {
  let app = Application::builder().application_id(APP_ID).build();

  app.connect_activate(move |application| {
    app_main(application);
  });
  app.run();
}

fn app_main(application: &Application) {
  load_css::load_css();

  let window = Rc::new(
    ApplicationWindow::builder()
      .default_width(1200)
      .default_height(600)
      .application(application)
      .title("fml9000")
      .build(),
  );

  let audio = match init_audio() {
    Ok(audio) => Some(audio),
    Err(e) => {
      show_error_dialog(&window, "Audio Error", &format!("{e}\n\nPlayback will be disabled."));
      None
    }
  };
  let sink = Rc::new(RefCell::new(audio));
  let settings = Rc::new(RefCell::new(crate::settings::read_settings()));
  let tracks = Rc::new(load_tracks());

  if let Some(folder) = &settings.borrow().folder {
    run_scan(folder, &tracks);
  }

  let filter = CustomFilter::new(|_| true);
  let playlist_store = ListStore::new::<BoxedAnyObject>();
  let playlist_mgr_store = ListStore::new::<BoxedAnyObject>();
  let facet_store = ListStore::new::<BoxedAnyObject>();
  let album_art = Rc::new(Image::builder().vexpand(true).build());

  load_playlist_store(tracks.iter(), &playlist_store);
  load_facet_store(&tracks, &facet_store);

  let playlist_view = create_playlist_view(
    playlist_store.clone(),
    &sink,
    &album_art,
    &window,
  );
  let playlist_mgr_view = create_playlist_manager(
    &playlist_mgr_store,
    playlist_store.clone(),
    Rc::clone(&tracks),
  );
  let facet_box = create_facet_box(playlist_store, facet_store, filter, &tracks);

  let left_pane = Paned::builder()
    .vexpand(true)
    .orientation(Orientation::Vertical)
    .start_child(&facet_box)
    .end_child(&playlist_view)
    .build();

  let right_pane = Paned::builder()
    .vexpand(true)
    .orientation(Orientation::Vertical)
    .start_child(&playlist_mgr_view)
    .end_child(&*album_art)
    .build();

  let main_pane = Paned::builder()
    .hexpand(true)
    .orientation(Orientation::Horizontal)
    .start_child(&left_pane)
    .end_child(&right_pane)
    .build();

  let main_ui = gtk::Box::new(Orientation::Vertical, 0);
  let header = create_header_bar(Rc::clone(&settings), Rc::clone(&sink), &window);

  main_ui.append(&header);
  main_ui.append(&main_pane);
  window.set_child(Some(&main_ui));
  window.present();
}
