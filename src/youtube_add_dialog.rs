use crate::playback_controller::PlaybackController;
use crate::youtube::{fetch_channel_info, parse_youtube_url, ChannelInfo, VideoInfo};
use gtk::prelude::*;
use fml9000::{add_youtube_channel, add_youtube_videos};
use gtk::glib;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

enum FetchResult {
  Success(ChannelInfo, Vec<VideoInfo>),
  Error(String),
}

pub fn show_dialog(
  playback_controller: Rc<PlaybackController>,
  on_channel_added: impl Fn() + 'static,
) {
  let wnd = playback_controller.window();

  let dialog = gtk::Window::builder()
    .title("Add YouTube Channel")
    .default_width(450)
    .default_height(200)
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

  let url_entry = gtk::Entry::builder()
    .placeholder_text("YouTube channel URL or handle (e.g. @channelname)")
    .hexpand(true)
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
    .label("Add Channel")
    .css_classes(["suggested-action"])
    .sensitive(false)
    .build();

  button_box.append(&cancel_btn);
  button_box.append(&add_btn);

  content.append(&url_entry);
  content.append(&spinner);
  content.append(&status_label);
  content.append(&button_box);

  dialog.set_child(Some(&content));

  let add_btn_clone = add_btn.clone();
  url_entry.connect_changed(move |entry| {
    let text = entry.text();
    add_btn_clone.set_sensitive(!text.is_empty() && parse_youtube_url(&text).is_some());
  });

  let dialog_weak = dialog.downgrade();
  cancel_btn.connect_clicked(move |_| {
    if let Some(d) = dialog_weak.upgrade() {
      d.close();
    }
  });

  let on_channel_added = Rc::new(RefCell::new(Some(on_channel_added)));
  let dialog_weak = dialog.downgrade();
  let url_entry_clone = url_entry.clone();
  let status_label_clone = status_label.clone();
  let spinner_clone = spinner.clone();
  let add_btn_clone = add_btn.clone();
  add_btn.connect_clicked(move |_| {
    let url = url_entry_clone.text().to_string();
    let status2 = status_label_clone.clone();
    let spin2 = spinner_clone.clone();
    let btn2 = add_btn_clone.clone();
    let dialog_weak2 = dialog_weak.clone();
    let callback = on_channel_added.clone();

    btn2.set_sensitive(false);
    spin2.set_visible(true);
    spin2.set_spinning(true);
    status2.set_label("Fetching channel info...");

    let (tx, rx) = mpsc::channel::<FetchResult>();

    std::thread::spawn(move || {
      let result = fetch_channel_info(&url, |_| {});
      let msg = match result {
        Ok((channel, videos)) => FetchResult::Success(channel, videos),
        Err(e) => FetchResult::Error(e),
      };
      let _ = tx.send(msg);
    });

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
      match rx.try_recv() {
        Ok(FetchResult::Success(channel, videos)) => {
          spin2.set_spinning(false);
          spin2.set_visible(false);

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

              status2.set_label(&format!("Added {} with {} videos", channel.name, videos.len()));

              if let Some(cb) = callback.borrow_mut().take() {
                cb();
              }

              let dialog_weak3 = dialog_weak2.clone();
              glib::timeout_add_local_once(std::time::Duration::from_secs(1), move || {
                if let Some(d) = dialog_weak3.upgrade() {
                  d.close();
                }
              });
            }
            Err(e) => {
              status2.set_label(&format!("Error: {e}"));
              btn2.set_sensitive(true);
            }
          }
          glib::ControlFlow::Break
        }
        Ok(FetchResult::Error(e)) => {
          spin2.set_spinning(false);
          spin2.set_visible(false);
          status2.set_label(&format!("Error: {e}"));
          btn2.set_sensitive(true);
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
