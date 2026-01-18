use crate::playback_controller::{PlaybackController, PlaybackSource};
use crate::settings::{FmlSettings, RepeatMode};
use adw::prelude::*;
use fml9000::models::Track;
use gtk::glib::{self, ControlFlow, MainContext};
use gtk::{Adjustment, Button, Label, Orientation, Scale, ScaleButton, ToggleButton};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

fn format_time(duration: Duration) -> String {
  let total_secs = duration.as_secs();
  let mins = total_secs / 60;
  let secs = total_secs % 60;
  format!("{mins}:{secs:02}")
}

fn repeat_mode_icon(mode: RepeatMode) -> &'static str {
  match mode {
    RepeatMode::Off => "media-playlist-repeat-symbolic",
    RepeatMode::All => "media-playlist-repeat-symbolic",
    RepeatMode::One => "media-playlist-repeat-song-symbolic",
  }
}

fn repeat_mode_tooltip(mode: RepeatMode) -> &'static str {
  match mode {
    RepeatMode::Off => "Repeat: Off",
    RepeatMode::All => "Repeat: All",
    RepeatMode::One => "Repeat: One",
  }
}

pub fn create_header_bar(
  settings: Rc<RefCell<FmlSettings>>,
  playback_controller: Rc<PlaybackController>,
  tracks: Rc<Vec<Rc<Track>>>,
  playlist_store: gtk::gio::ListStore,
  facet_store: gtk::gio::ListStore,
  playlist_mgr_store: gtk::gio::ListStore,
) -> gtk::Box {
  let pc_for_volume = Rc::clone(&playback_controller);
  let pc_for_pause = Rc::clone(&playback_controller);
  let pc_for_play = Rc::clone(&playback_controller);
  let pc_for_stop = Rc::clone(&playback_controller);
  let pc_for_prev = Rc::clone(&playback_controller);
  let pc_for_next = Rc::clone(&playback_controller);
  let pc_for_seek = Rc::clone(&playback_controller);
  let pc_for_timer = Rc::clone(&playback_controller);
  let pc_for_shuffle = Rc::clone(&playback_controller);
  let pc_for_repeat = Rc::clone(&playback_controller);

  let prev_btn = Button::builder()
    .icon_name("media-skip-backward-symbolic")
    .css_classes(["flat"])
    .build();
  let stop_btn = Button::builder()
    .icon_name("media-playback-stop-symbolic")
    .css_classes(["flat"])
    .build();
  let next_btn = Button::builder()
    .icon_name("media-skip-forward-symbolic")
    .css_classes(["flat"])
    .build();
  let pause_btn = Button::builder()
    .icon_name("media-playback-pause-symbolic")
    .css_classes(["flat"])
    .build();
  let play_btn = Button::builder()
    .icon_name("media-playback-start-symbolic")
    .css_classes(["flat"])
    .build();
  let settings_btn = Button::builder()
    .icon_name("emblem-system-symbolic")
    .css_classes(["flat"])
    .build();
  let shuffle_btn = ToggleButton::builder()
    .icon_name("media-playlist-shuffle-symbolic")
    .css_classes(["flat"])
    .active(playback_controller.shuffle_enabled())
    .tooltip_text("Shuffle")
    .build();

  let initial_repeat_mode = playback_controller.repeat_mode();
  let repeat_btn = Button::builder()
    .icon_name(repeat_mode_icon(initial_repeat_mode))
    .css_classes(if initial_repeat_mode == RepeatMode::Off { vec!["flat"] } else { vec!["flat", "repeat-active"] })
    .tooltip_text(repeat_mode_tooltip(initial_repeat_mode))
    .build();

  let button_box = gtk::Box::new(Orientation::Horizontal, 0);
  let seek_adjustment = Adjustment::new(0.0, 0.0, 1.0, 0.01, 0.0, 0.0);
  let seek_slider = Scale::builder()
    .hexpand(true)
    .orientation(Orientation::Horizontal)
    .adjustment(&seek_adjustment)
    .build();

  let time_current = Label::builder()
    .label("0:00")
    .width_chars(5)
    .css_classes(["monospace"])
    .build();

  let time_total = Label::builder()
    .label("0:00")
    .width_chars(5)
    .css_classes(["monospace"])
    .build();

  let time_separator = Label::builder().label("/").build();

  let is_seeking = Rc::new(Cell::new(false));
  let was_playing = Rc::new(Cell::new(false));

  let is_seeking_for_change = Rc::clone(&is_seeking);
  seek_adjustment.connect_value_changed(move |adj| {
    if is_seeking_for_change.get() {
      match pc_for_seek.playback_source() {
        PlaybackSource::Local => {
          if let Some(duration) = pc_for_seek.audio().get_duration() {
            let pos_secs = adj.value() * duration.as_secs_f64();
            pc_for_seek.audio().try_seek(Duration::from_secs_f64(pos_secs));
          }
        }
        PlaybackSource::YouTube => {
          if let Some(duration) = pc_for_seek.video_widget().get_duration() {
            let pos_secs = adj.value() * duration.as_secs_f64();
            pc_for_seek.video_widget().seek(Duration::from_secs_f64(pos_secs));
          }
        }
        PlaybackSource::None => {}
      }
    }
  });

  let is_seeking_for_press = Rc::clone(&is_seeking);
  let is_seeking_for_release = Rc::clone(&is_seeking);
  let gesture = gtk::GestureClick::new();
  gesture.connect_pressed(move |_, _, _, _| {
    is_seeking_for_press.set(true);
  });
  gesture.connect_end(move |_, _| {
    is_seeking_for_release.set(false);
  });
  seek_slider.add_controller(gesture);

  let seek_adjustment_for_timer = seek_adjustment.clone();
  let is_seeking_for_timer = Rc::clone(&is_seeking);
  let was_playing_for_timer = Rc::clone(&was_playing);
  let time_current_for_timer = time_current.clone();
  let time_total_for_timer = time_total.clone();
  let play_btn_for_timer = play_btn.clone();
  let pause_btn_for_timer = pause_btn.clone();
  let last_playback_source: Rc<Cell<PlaybackSource>> = Rc::new(Cell::new(PlaybackSource::None));
  glib::timeout_add_local(Duration::from_millis(250), move || {
    let current_source = pc_for_timer.playback_source();

    // Reset seekbar when playback source changes
    if current_source != last_playback_source.get() {
      last_playback_source.set(current_source);
      seek_adjustment_for_timer.set_value(0.0);
      time_current_for_timer.set_label("0:00");
      time_total_for_timer.set_label("0:00");
    }

    // Update play/pause button backgrounds based on state
    let is_playing = match current_source {
      PlaybackSource::Local => pc_for_timer.audio().is_playing(),
      PlaybackSource::YouTube => pc_for_timer.video_widget().is_playing(),
      PlaybackSource::None => false,
    };
    if is_playing {
      play_btn_for_timer.add_css_class("suggested-action");
      pause_btn_for_timer.remove_css_class("suggested-action");
    } else if current_source != PlaybackSource::None {
      play_btn_for_timer.remove_css_class("suggested-action");
      pause_btn_for_timer.add_css_class("suggested-action");
    } else {
      play_btn_for_timer.remove_css_class("suggested-action");
      pause_btn_for_timer.remove_css_class("suggested-action");
    }

    match current_source {
      PlaybackSource::Local => {
        if let Some(duration) = pc_for_timer.audio().get_duration() {
          let duration_secs = duration.as_secs_f64();
          if duration_secs > 0.0 {
            let pos = pc_for_timer.audio().get_pos();
            let pos_secs = pos.as_secs_f64();

            pc_for_timer.check_play_threshold(pos_secs);

            if !is_seeking_for_timer.get() {
              let fraction = (pos_secs / duration_secs).clamp(0.0, 1.0);
              seek_adjustment_for_timer.set_value(fraction);
            }

            time_current_for_timer.set_label(&format_time(pos));
            time_total_for_timer.set_label(&format_time(duration));

            if was_playing_for_timer.get() && pc_for_timer.audio().is_empty() {
              was_playing_for_timer.set(false);
              pc_for_timer.play_next();
            } else if !pc_for_timer.audio().is_empty() {
              was_playing_for_timer.set(true);
            }
          }
        }
      }
      PlaybackSource::YouTube => {
        if let Some(duration) = pc_for_timer.video_widget().get_duration() {
          let duration_secs = duration.as_secs_f64();
          if duration_secs > 1.0 {
            if let Some(pos) = pc_for_timer.video_widget().get_time_pos() {
              let pos_secs = pos.as_secs_f64();

              pc_for_timer.check_play_threshold(pos_secs);

              // Only update if position is valid (not beyond duration)
              if pos_secs >= 0.0 && pos_secs <= duration_secs {
                if !is_seeking_for_timer.get() {
                  let fraction = pos_secs / duration_secs;
                  seek_adjustment_for_timer.set_value(fraction);
                }

                time_current_for_timer.set_label(&format_time(pos));
                time_total_for_timer.set_label(&format_time(duration));
              }
            }
          }
        }

        if was_playing_for_timer.get() && pc_for_timer.video_widget().is_ended() {
          was_playing_for_timer.set(false);
          pc_for_timer.play_next();
        } else if pc_for_timer.video_widget().is_playing() {
          was_playing_for_timer.set(true);
        }
      }
      PlaybackSource::None => {}
    }
    ControlFlow::Continue
  });

  let volume_button = ScaleButton::builder()
    .icons([
      "audio-volume-muted-symbolic",
      "audio-volume-low-symbolic",
      "audio-volume-medium-symbolic",
      "audio-volume-high-symbolic",
    ])
    .value({
      let s = settings.borrow();
      s.volume
    })
    .build();
  let settings_for_volume = Rc::clone(&settings);
  volume_button.connect_value_changed(move |_, volume| {
    pc_for_volume.audio().set_volume(volume as f32);
    if pc_for_volume.playback_source() == PlaybackSource::YouTube {
      pc_for_volume.video_widget().set_volume(volume * 100.0);
    }
    let mut s = settings_for_volume.borrow_mut();
    s.volume = volume;
    if let Err(e) = crate::settings::write_settings(&s) {
      eprintln!("Warning: {e}");
    }
  });

  button_box.append(&settings_btn);
  button_box.append(&time_current);
  button_box.append(&time_separator);
  button_box.append(&time_total);
  button_box.append(&seek_slider);
  button_box.append(&play_btn);
  button_box.append(&pause_btn);
  button_box.append(&prev_btn);
  button_box.append(&next_btn);
  button_box.append(&stop_btn);
  button_box.append(&shuffle_btn);
  button_box.append(&repeat_btn);
  button_box.append(&volume_button);

  pause_btn.connect_clicked(move |_| {
    match pc_for_pause.playback_source() {
      PlaybackSource::Local => pc_for_pause.audio().pause(),
      PlaybackSource::YouTube => pc_for_pause.video_widget().pause(),
      PlaybackSource::None => {}
    }
  });

  play_btn.connect_clicked(move |_| {
    match pc_for_play.playback_source() {
      PlaybackSource::Local => pc_for_play.audio().play(),
      PlaybackSource::YouTube => pc_for_play.video_widget().unpause(),
      PlaybackSource::None => {}
    }
  });

  let seek_adjustment_for_stop = seek_adjustment.clone();
  let time_current_for_stop = time_current.clone();
  let time_total_for_stop = time_total.clone();
  let was_playing_for_stop = Rc::clone(&was_playing);
  stop_btn.connect_clicked(move |_| {
    pc_for_stop.stop();
    pc_for_stop.audio().clear_duration();
    seek_adjustment_for_stop.set_value(0.0);
    time_current_for_stop.set_label("0:00");
    time_total_for_stop.set_label("0:00");
    was_playing_for_stop.set(false);
  });

  prev_btn.connect_clicked(move |_| {
    pc_for_prev.play_prev();
  });

  next_btn.connect_clicked(move |_| {
    pc_for_next.play_next();
  });

  let settings_for_shuffle = Rc::clone(&settings);
  shuffle_btn.connect_toggled(move |btn| {
    let enabled = btn.is_active();
    pc_for_shuffle.set_shuffle_enabled(enabled);
    let mut s = settings_for_shuffle.borrow_mut();
    s.shuffle_enabled = enabled;
    if let Err(e) = crate::settings::write_settings(&s) {
      eprintln!("Warning: {e}");
    }
  });

  let settings_for_repeat = Rc::clone(&settings);
  repeat_btn.connect_clicked(move |btn| {
    let new_mode = pc_for_repeat.cycle_repeat_mode();
    btn.set_icon_name(repeat_mode_icon(new_mode));
    btn.set_tooltip_text(Some(repeat_mode_tooltip(new_mode)));
    if new_mode == RepeatMode::Off {
      btn.remove_css_class("repeat-active");
    } else {
      btn.add_css_class("repeat-active");
    }
    let mut s = settings_for_repeat.borrow_mut();
    s.repeat_mode = new_mode;
    if let Err(e) = crate::settings::write_settings(&s) {
      eprintln!("Warning: {e}");
    }
  });

  let ps = playlist_store.clone();
  let fs = facet_store.clone();
  let pms = playlist_mgr_store.clone();
  settings_btn.connect_clicked(move |_| {
    MainContext::default().spawn_local(crate::preferences_dialog::dialog(
      Rc::clone(&playback_controller),
      Rc::clone(&settings),
      Rc::clone(&tracks),
      ps.clone(),
      fs.clone(),
      pms.clone(),
    ));
  });

  button_box
}
