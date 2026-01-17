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
use fml9000::{load_facet_store, load_playlist_store, load_tracks, run_scan_folders};
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

struct AudioState {
  _stream: OutputStream,
  sink: Sink,
}

#[derive(Clone)]
pub struct AudioPlayer {
  inner: Rc<RefCell<Option<AudioState>>>,
}

impl AudioPlayer {
  pub fn new() -> (Self, Option<String>) {
    let (inner, error) = match Self::init_audio() {
      Ok(state) => (Some(state), None),
      Err(e) => (None, Some(e)),
    };
    (
      Self {
        inner: Rc::new(RefCell::new(inner)),
      },
      error,
    )
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

  pub fn is_available(&self) -> bool {
    self.inner.borrow().is_some()
  }

  pub fn play(&self) {
    if let Some(audio) = self.inner.borrow().as_ref() {
      audio.sink.play();
    }
  }

  pub fn pause(&self) {
    if let Some(audio) = self.inner.borrow().as_ref() {
      audio.sink.pause();
    }
  }

  pub fn stop(&self) {
    if let Some(audio) = self.inner.borrow().as_ref() {
      audio.sink.stop();
    }
  }

  pub fn set_volume(&self, volume: f32) {
    if let Some(audio) = self.inner.borrow().as_ref() {
      audio.sink.set_volume(volume);
    }
  }

  pub fn play_source<S>(&self, source: S) -> bool
  where
    S: rodio::Source + Send + 'static,
    S::Item: rodio::Sample + Send,
    f32: rodio::cpal::FromSample<S::Item>,
  {
    if let Some(audio) = self.inner.borrow().as_ref() {
      audio.sink.stop();
      audio.sink.append(source);
      audio.sink.play();
      true
    } else {
      false
    }
  }
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

  let (audio, audio_error) = AudioPlayer::new();
  if let Some(e) = audio_error {
    show_error_dialog(&window, "Audio Error", &format!("{e}\n\nPlayback will be disabled."));
  }

  let settings = Rc::new(RefCell::new(crate::settings::read_settings()));

  let tracks = match load_tracks() {
    Ok(t) => Rc::new(t),
    Err(e) => {
      show_error_dialog(&window, "Database Error", &format!("{e}\n\nLibrary will be empty."));
      Rc::new(Vec::new())
    }
  };

  if settings.borrow().rescan_on_startup {
    let folders = settings.borrow().folders.clone();
    run_scan_folders(&folders, &tracks);
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
    audio.clone(),
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
  let header = create_header_bar(Rc::clone(&settings), audio, &window, Rc::clone(&tracks));

  main_ui.append(&header);
  main_ui.append(&main_pane);
  window.set_child(Some(&main_ui));
  window.present();
}
