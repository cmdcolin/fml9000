use crate::playback_controller::PlaybackController;
use crate::settings::FmlSettings;
use adw::prelude::*;
use fml9000::models::Track;
use gtk::glib::{self, ControlFlow, MainContext};
use gtk::{Adjustment, Button, Orientation, Scale, ScaleButton};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

pub fn create_header_bar(
  settings: Rc<RefCell<FmlSettings>>,
  playback_controller: Rc<PlaybackController>,
  tracks: Rc<Vec<Rc<Track>>>,
) -> gtk::Box {
  let pc_for_volume = Rc::clone(&playback_controller);
  let pc_for_pause = Rc::clone(&playback_controller);
  let pc_for_play = Rc::clone(&playback_controller);
  let pc_for_stop = Rc::clone(&playback_controller);
  let pc_for_prev = Rc::clone(&playback_controller);
  let pc_for_next = Rc::clone(&playback_controller);
  let pc_for_seek = Rc::clone(&playback_controller);
  let pc_for_timer = Rc::clone(&playback_controller);

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

  let button_box = gtk::Box::new(Orientation::Horizontal, 0);
  let seek_adjustment = Adjustment::new(0.0, 0.0, 1.0, 0.01, 0.0, 0.0);
  let seek_slider = Scale::builder()
    .hexpand(true)
    .orientation(Orientation::Horizontal)
    .adjustment(&seek_adjustment)
    .build();

  let is_seeking = Rc::new(Cell::new(false));

  let is_seeking_for_change = Rc::clone(&is_seeking);
  seek_adjustment.connect_value_changed(move |adj| {
    if is_seeking_for_change.get() {
      if let Some(duration) = pc_for_seek.audio().get_duration() {
        let pos_secs = adj.value() * duration.as_secs_f64();
        pc_for_seek.audio().try_seek(Duration::from_secs_f64(pos_secs));
      }
    }
  });

  let is_seeking_for_press = Rc::clone(&is_seeking);
  let gesture = gtk::GestureClick::new();
  gesture.connect_pressed(move |_, _, _, _| {
    is_seeking_for_press.set(true);
  });
  seek_slider.add_controller(gesture);

  let is_seeking_for_release = Rc::clone(&is_seeking);
  let gesture_release = gtk::GestureClick::new();
  gesture_release.connect_released(move |_, _, _, _| {
    is_seeking_for_release.set(false);
  });
  seek_slider.add_controller(gesture_release);

  let seek_adjustment_for_timer = seek_adjustment.clone();
  let is_seeking_for_timer = Rc::clone(&is_seeking);
  glib::timeout_add_local(Duration::from_millis(250), move || {
    if !is_seeking_for_timer.get() {
      if let Some(duration) = pc_for_timer.audio().get_duration() {
        let duration_secs = duration.as_secs_f64();
        if duration_secs > 0.0 {
          let pos = pc_for_timer.audio().get_pos().as_secs_f64();
          let fraction = (pos / duration_secs).clamp(0.0, 1.0);
          seek_adjustment_for_timer.set_value(fraction);
        }
      }
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
    let mut s = settings_for_volume.borrow_mut();
    s.volume = volume;
    if let Err(e) = crate::settings::write_settings(&s) {
      eprintln!("Warning: {e}");
    }
  });

  button_box.append(&settings_btn);
  button_box.append(&seek_slider);
  button_box.append(&play_btn);
  button_box.append(&pause_btn);
  button_box.append(&prev_btn);
  button_box.append(&next_btn);
  button_box.append(&stop_btn);
  button_box.append(&volume_button);

  pause_btn.connect_clicked(move |_| {
    pc_for_pause.audio().pause();
  });

  play_btn.connect_clicked(move |_| {
    pc_for_play.audio().play();
  });

  stop_btn.connect_clicked(move |_| {
    pc_for_stop.audio().stop();
  });

  prev_btn.connect_clicked(move |_| {
    pc_for_prev.play_prev();
  });

  next_btn.connect_clicked(move |_| {
    pc_for_next.play_next();
  });

  settings_btn.connect_clicked(move |_| {
    MainContext::default().spawn_local(crate::preferences_dialog::dialog(
      Rc::clone(&playback_controller),
      Rc::clone(&settings),
      Rc::clone(&tracks),
    ));
  });

  button_box
}
