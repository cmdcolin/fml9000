mod facet_box;
mod grid_cell;
mod gtk_helpers;
mod header_bar;
mod load_css;
mod new_playlist_dialog;
mod playback_controller;
mod playlist_manager;
mod playlist_view;
mod preferences_dialog;
mod settings;
mod video_widget;
mod youtube;
mod youtube_add_dialog;
mod youtube_api;

use adw::prelude::*;
use adw::Application;
use facet_box::create_facet_box;
use fml9000::{load_facet_store, load_playlist_store, load_tracks};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::gdk::Key;
use gtk::{AlertDialog, ApplicationWindow, ContentFit, CustomFilter, EventControllerKey, Orientation, Paned, Picture, Stack};
use video_widget::VideoWidget;
use header_bar::create_header_bar;
use playback_controller::{PlaybackController, PlaybackSource};
use playlist_manager::create_playlist_manager;
use playlist_view::create_playlist_view;
use rodio::{OutputStream, Sink};
use std::time::Duration;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const APP_ID: &str = "com.github.fml9000";

struct AudioState {
  _stream: OutputStream,
  sink: Sink,
  duration: Option<Duration>,
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
      duration: None,
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

  pub fn play_source<S>(&self, source: S, duration: Option<Duration>) -> bool
  where
    S: rodio::Source + Send + 'static,
    S::Item: rodio::Sample + Send,
    f32: rodio::cpal::FromSample<S::Item>,
  {
    if let Some(audio) = self.inner.borrow_mut().as_mut() {
      audio.sink.stop();
      audio.sink.append(source);
      audio.sink.play();
      audio.duration = duration;
      true
    } else {
      false
    }
  }

  pub fn try_seek(&self, pos: Duration) {
    if let Some(audio) = self.inner.borrow().as_ref() {
      let _ = audio.sink.try_seek(pos);
    }
  }

  pub fn get_pos(&self) -> Duration {
    if let Some(audio) = self.inner.borrow().as_ref() {
      audio.sink.get_pos()
    } else {
      Duration::ZERO
    }
  }

  pub fn get_duration(&self) -> Option<Duration> {
    if let Some(audio) = self.inner.borrow().as_ref() {
      audio.duration
    } else {
      None
    }
  }

  pub fn is_playing(&self) -> bool {
    if let Some(audio) = self.inner.borrow().as_ref() {
      !audio.sink.is_paused() && !audio.sink.empty()
    } else {
      false
    }
  }

  pub fn is_empty(&self) -> bool {
    if let Some(audio) = self.inner.borrow().as_ref() {
      audio.sink.empty()
    } else {
      true
    }
  }

  pub fn clear_duration(&self) {
    if let Some(audio) = self.inner.borrow_mut().as_mut() {
      audio.duration = None;
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

  // Add local img directory to icon search path for dev mode
  let icon_theme = gtk::IconTheme::for_display(&gtk::gdk::Display::default().unwrap());
  if let Ok(exe_path) = std::env::current_exe() {
    if let Some(exe_dir) = exe_path.parent() {
      // Check for img dir relative to executable (release) or in current dir (dev)
      for base in [exe_dir.to_path_buf(), std::env::current_dir().unwrap_or_default()] {
        let img_dir = base.join("img");
        if img_dir.exists() {
          icon_theme.add_search_path(&img_dir);
        }
      }
    }
  }

  let settings = Rc::new(RefCell::new(crate::settings::read_settings()));

  let window = Rc::new(
    ApplicationWindow::builder()
      .default_width(settings.borrow().window_width)
      .default_height(settings.borrow().window_height)
      .application(application)
      .title("fml9000")
      .icon_name("fml9000")
      .build(),
  );

  let (audio, audio_error) = AudioPlayer::new();
  if let Some(e) = audio_error {
    show_error_dialog(&window, "Audio Error", &format!("{e}\n\nPlayback will be disabled."));
  }

  let tracks = match load_tracks() {
    Ok(t) => Rc::new(t),
    Err(e) => {
      show_error_dialog(&window, "Database Error", &format!("{e}\n\nLibrary will be empty."));
      Rc::new(Vec::new())
    }
  };

  if settings.borrow().rescan_on_startup {
    let folders = settings.borrow().folders.clone();
    let existing_files: std::collections::HashSet<String> = tracks.iter().map(|t| t.filename.clone()).collect();
    std::thread::spawn(move || {
      let (tx, _rx) = std::sync::mpsc::channel();
      fml9000::run_scan_with_progress(folders, existing_files, std::collections::HashSet::new(), tx);
    });
  }

  let filter = CustomFilter::new(|_| true);
  let playlist_store = ListStore::new::<BoxedAnyObject>();
  let playlist_mgr_store = ListStore::new::<BoxedAnyObject>();
  let facet_store = ListStore::new::<BoxedAnyObject>();
  let album_art = Picture::builder()
    .vexpand(true)
    .hexpand(true)
    .content_fit(ContentFit::Contain)
    .build();
  let album_art = Rc::new(album_art);

  // Create video widget for embedded GStreamer playback
  let video_widget = VideoWidget::new();

  // Create a stack to switch between album art and video
  let media_stack = Stack::new();
  media_stack.add_named(&*album_art, Some("album_art"));
  media_stack.add_named(video_widget.widget(), Some("video"));
  media_stack.set_visible_child_name("album_art");
  media_stack.set_vexpand(true);
  media_stack.set_hexpand(true);
  let media_stack = Rc::new(media_stack);

  load_playlist_store(tracks.iter(), &playlist_store);
  load_facet_store(&tracks, &facet_store);

  let playback_controller = PlaybackController::new(
    audio.clone(),
    playlist_store.clone(),
    Rc::clone(&album_art),
    Rc::clone(&video_widget),
    Rc::clone(&media_stack),
    Rc::clone(&window),
    settings.borrow().shuffle_enabled,
    settings.borrow().repeat_mode,
  );

  let current_playlist_id: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
  let is_viewing_playback_queue: Rc<Cell<bool>> = Rc::new(Cell::new(false));

  let playlist_view = create_playlist_view(
    playlist_store.clone(),
    Rc::clone(&playback_controller),
    Rc::clone(&settings),
    Rc::clone(&current_playlist_id),
    Rc::clone(&is_viewing_playback_queue),
  );
  let (playlist_mgr_view, playlist_mgr_selection) = create_playlist_manager(
    &playlist_mgr_store,
    playlist_store.clone(),
    Rc::clone(&tracks),
    Rc::clone(&playback_controller),
    Rc::clone(&settings),
    Rc::clone(&current_playlist_id),
    Rc::clone(&is_viewing_playback_queue),
  );
  let playlist_store_for_header = playlist_store.clone();
  let facet_store_for_header = facet_store.clone();
  let (facet_box, facet_selection) = create_facet_box(playlist_store, facet_store, filter, &tracks, Rc::clone(&settings));

  // Wire up mutual deselection between facet and playlist manager
  let playlist_mgr_selection_for_facet = playlist_mgr_selection.clone();
  let is_viewing_queue_for_facet = is_viewing_playback_queue.clone();
  let playback_controller_for_facet = playback_controller.clone();
  facet_selection.connect_selection_changed(move |sel, _, _| {
    if !sel.selection().is_empty() {
      playlist_mgr_selection_for_facet.set_selected(gtk::INVALID_LIST_POSITION);
      is_viewing_queue_for_facet.set(false);
      playback_controller_for_facet.set_on_queue_changed(None);
    }
  });

  let facet_selection_for_playlist = Rc::clone(&facet_selection);
  playlist_mgr_selection.connect_selection_changed(move |sel, _, _| {
    if sel.selected() != gtk::INVALID_LIST_POSITION {
      facet_selection_for_playlist.unselect_all();
    }
  });

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
    .end_child(&*media_stack)
    .build();

  let main_pane = Paned::builder()
    .hexpand(true)
    .orientation(Orientation::Horizontal)
    .start_child(&left_pane)
    .end_child(&right_pane)
    .build();

  {
    let s = settings.borrow();
    if s.main_pane_position > 0 {
      main_pane.set_position(s.main_pane_position);
    }
    if s.left_pane_position > 0 {
      left_pane.set_position(s.left_pane_position);
    }
    if s.right_pane_position > 0 {
      right_pane.set_position(s.right_pane_position);
    }
  }

  let main_ui = gtk::Box::new(Orientation::Vertical, 0);
  let header = create_header_bar(
    Rc::clone(&settings),
    Rc::clone(&playback_controller),
    Rc::clone(&tracks),
    playlist_store_for_header,
    facet_store_for_header,
    playlist_mgr_store,
  );

  main_ui.append(&header);
  main_ui.append(&main_pane);

  let pc_for_keys = Rc::clone(&playback_controller);
  let key_controller = EventControllerKey::new();
  key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
  key_controller.connect_key_pressed(move |_, key, _, _| {
    match key {
      Key::space => {
        match pc_for_keys.playback_source() {
          PlaybackSource::Local => {
            if pc_for_keys.audio().is_playing() {
              pc_for_keys.audio().pause();
            } else {
              pc_for_keys.audio().play();
            }
          }
          PlaybackSource::YouTube => {
            if pc_for_keys.video_widget().is_playing() {
              pc_for_keys.video_widget().pause();
            } else {
              pc_for_keys.video_widget().unpause();
            }
          }
          PlaybackSource::None => {}
        }
        gtk::glib::Propagation::Stop
      }
      Key::n | Key::N => {
        pc_for_keys.play_next();
        gtk::glib::Propagation::Stop
      }
      Key::p | Key::P => {
        pc_for_keys.play_prev();
        gtk::glib::Propagation::Stop
      }
      Key::s | Key::S => {
        pc_for_keys.stop();
        gtk::glib::Propagation::Stop
      }
      Key::r | Key::R => {
        let enabled = !pc_for_keys.shuffle_enabled();
        pc_for_keys.set_shuffle_enabled(enabled);
        gtk::glib::Propagation::Stop
      }
      _ => gtk::glib::Propagation::Proceed,
    }
  });
  window.add_controller(key_controller);

  window.set_child(Some(&main_ui));

  let settings_for_close = Rc::clone(&settings);
  let window_for_close = Rc::clone(&window);
  window.connect_close_request(move |_| {
    let mut s = settings_for_close.borrow_mut();
    s.window_width = window_for_close.width();
    s.window_height = window_for_close.height();
    s.main_pane_position = main_pane.position();
    s.left_pane_position = left_pane.position();
    s.right_pane_position = right_pane.position();
    if let Err(e) = crate::settings::write_settings(&s) {
      eprintln!("Failed to save layout settings: {e}");
    }
    gtk::glib::Propagation::Proceed
  });

  window.present();
}
