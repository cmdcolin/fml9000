use crate::playback_controller::PlaybackController;
use crate::youtube::{fetch_channel_info, parse_youtube_url, ChannelInfo, VideoInfo};
use gtk::prelude::*;
use fml9000_core::{add_youtube_channel, add_youtube_videos};
use gtk::glib;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

enum FetchResult {
  Success(String, ChannelInfo, Vec<VideoInfo>),
  Error(String, String),
  AllDone,
}

pub fn show_dialog(
  playback_controller: Rc<PlaybackController>,
  on_channel_added: impl Fn() + 'static,
) {
  let wnd = playback_controller.window();

  let dialog = gtk::Window::builder()
    .title("Add YouTube Channels")
    .default_width(500)
    .default_height(350)
    .modal(true)
    .transient_for(&**wnd)
    .build();

  let content = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
    .spacing(12)
    .margin_top(24)
    .margin_bottom(24)
    .margin_start(24)
    .margin_end(24)
    .build();

  let hint_label = gtk::Label::builder()
    .label("Enter YouTube channel URLs or handles (one per line)")
    .xalign(0.0)
    .build();

  let url_textview = gtk::TextView::builder()
    .hexpand(true)
    .vexpand(true)
    .wrap_mode(gtk::WrapMode::Word)
    .build();

  let scrolled = gtk::ScrolledWindow::builder()
    .hscrollbar_policy(gtk::PolicyType::Never)
    .vscrollbar_policy(gtk::PolicyType::Automatic)
    .min_content_height(150)
    .child(&url_textview)
    .build();

  let status_label = gtk::Label::builder()
    .label("")
    .css_classes(["dim-label"])
    .wrap(true)
    .xalign(0.0)
    .build();

  let spinner = gtk::Spinner::builder()
    .spinning(false)
    .visible(false)
    .build();

  let button_box = gtk::Box::builder()
    .orientation(gtk::Orientation::Horizontal)
    .spacing(12)
    .halign(gtk::Align::End)
    .build();

  let cancel_btn = gtk::Button::builder().label("Cancel").build();

  let add_btn = gtk::Button::builder()
    .label("Add Channels")
    .css_classes(["suggested-action"])
    .sensitive(false)
    .build();

  button_box.append(&cancel_btn);
  button_box.append(&add_btn);

  content.append(&hint_label);
  content.append(&scrolled);
  content.append(&spinner);
  content.append(&status_label);
  content.append(&button_box);

  dialog.set_child(Some(&content));

  let buffer = url_textview.buffer();
  let add_btn_clone = add_btn.clone();
  buffer.connect_changed(move |buf| {
    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
    let has_valid = text
      .lines()
      .any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && parse_youtube_url(trimmed).is_some()
      });
    add_btn_clone.set_sensitive(has_valid);
  });

  let dialog_weak = dialog.downgrade();
  cancel_btn.connect_clicked(move |_| {
    if let Some(d) = dialog_weak.upgrade() {
      d.close();
    }
  });

  let on_channel_added = Rc::new(on_channel_added);
  let dialog_weak = dialog.downgrade();
  let buffer_clone = buffer.clone();
  let status_label_clone = status_label.clone();
  let spinner_clone = spinner.clone();
  let add_btn_clone = add_btn.clone();
  add_btn.connect_clicked(move |_| {
    let text = buffer_clone.text(&buffer_clone.start_iter(), &buffer_clone.end_iter(), false);
    let urls: Vec<String> = text
      .lines()
      .filter_map(|line| {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
          parse_youtube_url(trimmed)
        } else {
          None
        }
      })
      .collect();

    if urls.is_empty() {
      return;
    }

    let status2 = status_label_clone.clone();
    let spin2 = spinner_clone.clone();
    let btn2 = add_btn_clone.clone();
    let dialog_weak2 = dialog_weak.clone();
    let callback = on_channel_added.clone();

    btn2.set_sensitive(false);
    spin2.set_visible(true);
    spin2.set_spinning(true);

    let total = urls.len();
    status2.set_label(&format!("Fetching channel 1 of {}...", total));

    let (tx, rx) = mpsc::channel::<FetchResult>();

    std::thread::spawn(move || {
      for (idx, url) in urls.iter().enumerate() {
        let result = fetch_channel_info(url, |_| {});
        let msg = match result {
          Ok((channel, videos)) => FetchResult::Success(url.clone(), channel, videos),
          Err(e) => FetchResult::Error(url.clone(), e),
        };
        let _ = tx.send(msg);
        if idx < urls.len() - 1 {
          std::thread::sleep(std::time::Duration::from_secs(3));
        }
      }
      let _ = tx.send(FetchResult::AllDone);
    });

    let added_count = Rc::new(RefCell::new(0usize));
    let error_count = Rc::new(RefCell::new(0usize));
    let processed_count = Rc::new(RefCell::new(0usize));

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
      match rx.try_recv() {
        Ok(FetchResult::Success(_url, channel, videos)) => {
          *processed_count.borrow_mut() += 1;
          let processed = *processed_count.borrow();

          match add_youtube_channel(
            &channel.channel_id,
            &channel.name,
            channel.handle.as_deref(),
            &channel.url,
            channel.thumbnail_url.as_deref(),
          ) {
            Ok(db_channel_id) => {
              let video_tuples: Vec<_> = videos
                .iter()
                .map(|v| {
                  (
                    v.video_id.clone(),
                    v.title.clone(),
                    v.duration_seconds,
                    v.thumbnail_url.clone(),
                    v.published_at,
                  )
                })
                .collect();
              let _ = add_youtube_videos(db_channel_id, &video_tuples);
              *added_count.borrow_mut() += 1;
              callback();
            }
            Err(_e) => {
              *error_count.borrow_mut() += 1;
            }
          }

          if processed < total {
            status2.set_label(&format!("Fetching channel {} of {}...", processed + 1, total));
          }
          glib::ControlFlow::Continue
        }
        Ok(FetchResult::Error(_url, _e)) => {
          *processed_count.borrow_mut() += 1;
          *error_count.borrow_mut() += 1;
          let processed = *processed_count.borrow();

          if processed < total {
            status2.set_label(&format!("Fetching channel {} of {}...", processed + 1, total));
          }
          glib::ControlFlow::Continue
        }
        Ok(FetchResult::AllDone) => {
          spin2.set_spinning(false);
          spin2.set_visible(false);

          let added = *added_count.borrow();
          let errors = *error_count.borrow();

          if errors == 0 {
            status2.set_label(&format!("Added {} channels", added));
          } else {
            status2.set_label(&format!("Added {} channels, {} failed", added, errors));
          }

          let dialog_weak3 = dialog_weak2.clone();
          glib::timeout_add_local_once(std::time::Duration::from_secs(1), move || {
            if let Some(d) = dialog_weak3.upgrade() {
              d.close();
            }
          });
          glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(mpsc::TryRecvError::Disconnected) => {
          spin2.set_spinning(false);
          spin2.set_visible(false);
          status2.set_label("Error: Connection lost");
          btn2.set_sensitive(true);
          glib::ControlFlow::Break
        }
      }
    });
  });

  dialog.present();
}
