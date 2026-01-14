use crate::gtk_helpers::{create_button, load_img};
use crate::settings::FmlSettings;
use adw::prelude::*;
use gtk::glib::MainContext;
use gtk::{Adjustment, Orientation, Scale, ScaleButton};
use rodio::Sink;
use std::cell::RefCell;
use std::rc::Rc;

static PREV_SVG: &[u8] = include_bytes!("img/prev.svg");
static STOP_SVG: &[u8] = include_bytes!("img/stop.svg");
static NEXT_SVG: &[u8] = include_bytes!("img/next.svg");
static PAUSE_SVG: &[u8] = include_bytes!("img/pause.svg");
static PLAY_SVG: &[u8] = include_bytes!("img/play.svg");
static SETTINGS_SVG: &[u8] = include_bytes!("img/settings.svg");

pub fn create_header_bar(
  settings: Rc<RefCell<FmlSettings>>,
  sink: Rc<RefCell<Sink>>,
  window: &Rc<gtk::ApplicationWindow>,
) -> gtk::Box {
  let sink_for_pause = Rc::clone(&sink);
  let sink_for_play = Rc::clone(&sink);
  let sink_for_stop = Rc::clone(&sink);
  let window_for_settings = Rc::clone(window);

  let prev_btn = create_button(&load_img(PREV_SVG));
  let stop_btn = create_button(&load_img(STOP_SVG));
  let next_btn = create_button(&load_img(NEXT_SVG));
  let pause_btn = create_button(&load_img(PAUSE_SVG));
  let play_btn = create_button(&load_img(PLAY_SVG));
  let settings_btn = create_button(&load_img(SETTINGS_SVG));

  let button_box = gtk::Box::new(Orientation::Horizontal, 0);
  let seek_slider = Scale::builder()
    .hexpand(true)
    .orientation(Orientation::Horizontal)
    .adjustment(&Adjustment::new(0.0, 0.0, 1.0, 0.01, 0.0, 0.0))
    .build();

  let volume_button = ScaleButton::builder()
    .value({
      let s = settings.borrow();
      s.volume
    })
    .build();
  let settings1 = settings.clone();
  volume_button.connect_value_changed(move |_, volume| {
    let sink = sink.borrow();
    let mut s = settings1.borrow_mut();
    s.volume = volume;
    if let Err(e) = crate::settings::write_settings(&s) {
      eprintln!("Warning: {e}");
    }
    sink.set_volume(volume as f32);
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
    sink_for_pause.borrow().pause();
  });

  play_btn.connect_clicked(move |_| {
    sink_for_play.borrow().play();
  });

  stop_btn.connect_clicked(move |_| {
    sink_for_stop.borrow().stop();
  });

  settings_btn.connect_clicked(move |_| {
    MainContext::default().spawn_local(crate::preferences_dialog::dialog(
      Rc::clone(&window_for_settings),
      Rc::clone(&settings),
    ));
  });

  button_box
}
