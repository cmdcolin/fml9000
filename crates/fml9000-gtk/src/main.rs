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

use adw::prelude::*;
use adw::Application;
use facet_box::{create_facet_box, load_facet_store, load_playlist_store};
use fml9000_core::{init_db, load_tracks, run_scan_with_progress, delete_tracks_by_filename, AudioPlayer, ScanProgress};
use gtk::gio::ListStore;
use gtk::glib;
use gtk::glib::BoxedAnyObject;
use gtk::gdk::Key;
use gtk::{AlertDialog, ApplicationWindow, ContentFit, CustomFilter, EventControllerKey, Label, Notebook, Orientation, Paned, Picture, Stack};
use video_widget::VideoWidget;
use header_bar::create_header_bar;
use playback_controller::{PlaybackController, PlaybackSource};
use playlist_manager::create_playlist_manager;
use playlist_view::create_playlist_view;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const APP_ID: &str = "com.github.fml9000";

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

  // Initialize database and run migrations once at startup
  if let Err(e) = init_db() {
    eprintln!("Failed to initialize database: {e}");
  }

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

  if settings.borrow().rescan_on_startup {
    let folders = settings.borrow().folders.clone();
    let mut existing_complete: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut existing_incomplete: std::collections::HashSet<String> = std::collections::HashSet::new();
    for track in tracks.iter() {
      if track.duration_seconds.is_some() {
        existing_complete.insert(track.filename.clone());
      } else {
        existing_incomplete.insert(track.filename.clone());
      }
    }

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
      run_scan_with_progress(folders, existing_complete, existing_incomplete, tx);
    });

    let playlist_store_for_scan = playlist_store.clone();
    let facet_store_for_scan = facet_store.clone();
    let window_for_scan = Rc::clone(&window);
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
      while let Ok(progress) = rx.try_recv() {
        if let ScanProgress::Complete(_found, _skipped, added, updated, stale_files) = progress {
          if added > 0 || updated > 0 {
            if let Ok(fresh_tracks) = load_tracks() {
              playlist_store_for_scan.remove_all();
              load_playlist_store(fresh_tracks.iter(), &playlist_store_for_scan);
              facet_store_for_scan.remove_all();
              load_facet_store(&fresh_tracks, &facet_store_for_scan);
            }
          }

          if !stale_files.is_empty() {
            let stale_count = stale_files.len();
            let preview: String = stale_files.iter().take(10)
              .map(|f| {
                std::path::Path::new(f)
                  .file_name()
                  .map(|n| n.to_string_lossy().to_string())
                  .unwrap_or_else(|| f.clone())
              })
              .collect::<Vec<_>>()
              .join("\n");
            let detail = if stale_count > 10 {
              format!("{preview}\n...and {} more", stale_count - 10)
            } else {
              preview
            };

            let confirm = gtk::AlertDialog::builder()
              .modal(true)
              .message(&format!("{stale_count} tracks no longer found on disk. Remove from library?"))
              .detail(&detail)
              .buttons(["Cancel", "Remove"])
              .default_button(0)
              .cancel_button(0)
              .build();

            let ps = playlist_store_for_scan.clone();
            let fs = facet_store_for_scan.clone();
            confirm.choose(
              Some(&*window_for_scan),
              None::<&gtk::gio::Cancellable>,
              move |result| {
                if result == Ok(1) {
                  if let Err(e) = delete_tracks_by_filename(&stale_files) {
                    eprintln!("Warning: Failed to remove stale tracks: {e}");
                  }
                  if let Ok(fresh_tracks) = load_tracks() {
                    ps.remove_all();
                    load_playlist_store(fresh_tracks.iter(), &ps);
                    fs.remove_all();
                    load_facet_store(&fresh_tracks, &fs);
                  }
                }
              },
            );
          }

          return glib::ControlFlow::Break;
        }
      }
      glib::ControlFlow::Continue
    });
  }

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

  // Create Art tab content - a Picture that mirrors the album art
  let art_tab_picture = Picture::builder()
    .vexpand(true)
    .hexpand(true)
    .content_fit(ContentFit::Contain)
    .build();

  // Bind the art tab picture to show the same paintable as album_art
  album_art.bind_property("paintable", &art_tab_picture, "paintable")
    .sync_create()
    .build();

  // Create video widget for art tab
  let art_tab_video = VideoWidget::new();
  video_widget.bind_to_other(&art_tab_video);

  // Create a stack for art tab to switch between picture and video
  let art_tab_stack = Stack::new();
  art_tab_stack.add_named(&art_tab_picture, Some("album_art"));
  art_tab_stack.add_named(art_tab_video.widget(), Some("video"));
  art_tab_stack.set_visible_child_name("album_art");
  art_tab_stack.set_vexpand(true);
  art_tab_stack.set_hexpand(true);

  // Sync the visible child between media_stack and art_tab_stack
  media_stack.bind_property("visible-child-name", &art_tab_stack, "visible-child-name")
    .sync_create()
    .build();

  // Create notebook with tabs
  let notebook = Notebook::builder()
    .vexpand(true)
    .hexpand(true)
    .build();

  notebook.append_page(&main_pane, Some(&Label::new(Some("Main"))));
  notebook.append_page(&art_tab_stack, Some(&Label::new(Some("Art"))));

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
  main_ui.append(&notebook);

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
