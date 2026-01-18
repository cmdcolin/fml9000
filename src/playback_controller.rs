use crate::gtk_helpers::str_or_unknown;
use crate::settings::RepeatMode;
use crate::video_widget::VideoWidget;
use crate::AudioPlayer;
use fml9000::{
  add_track_to_recently_played, add_track_to_queue, add_video_to_queue,
  load_track_by_filename, load_video_by_id, pop_queue_front, queue_len,
  remove_track_from_queue, remove_video_from_queue, update_track_play_stats,
  update_video_play_stats, QueueItem,
};
use fml9000::models::{Track, YouTubeVideo};
use gtk::gdk;
use gtk::gio::ListStore;
use gtk::glib::{Bytes, BoxedAnyObject};
use gtk::prelude::*;
use gtk::{AlertDialog, ApplicationWindow, Picture, Stack};
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use rand::Rng;
use rodio::source::Source;
use rodio::Decoder;
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone)]
enum CurrentPlayStats {
  None,
  Track { filename: String, duration_secs: f64, counted: bool },
  Video { id: i32, duration_secs: f64, counted: bool },
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum PlaybackSource {
  #[default]
  None,
  Local,
  YouTube,
}

#[derive(Clone)]
enum PlayableItem {
  LocalTrack(Rc<Track>),
  YouTubeVideo(Rc<YouTubeVideo>),
}

pub struct PlaybackController {
  audio: AudioPlayer,
  video_widget: Rc<VideoWidget>,
  media_stack: Rc<Stack>,
  playlist_store: ListStore,
  current_index: Cell<Option<u32>>,
  playback_source: Cell<PlaybackSource>,
  shuffle_enabled: Cell<bool>,
  repeat_mode: Cell<RepeatMode>,
  album_art: Rc<Picture>,
  window: Rc<ApplicationWindow>,
  play_stats: RefCell<CurrentPlayStats>,
  on_queue_changed: RefCell<Option<Rc<dyn Fn()>>>,
}

impl PlaybackController {
  pub fn new(
    audio: AudioPlayer,
    playlist_store: ListStore,
    album_art: Rc<Picture>,
    video_widget: Rc<VideoWidget>,
    media_stack: Rc<Stack>,
    window: Rc<ApplicationWindow>,
    shuffle_enabled: bool,
    repeat_mode: RepeatMode,
  ) -> Rc<Self> {
    Rc::new(Self {
      audio,
      video_widget,
      media_stack,
      playlist_store,
      current_index: Cell::new(None),
      playback_source: Cell::new(PlaybackSource::None),
      shuffle_enabled: Cell::new(shuffle_enabled),
      repeat_mode: Cell::new(repeat_mode),
      album_art,
      window,
      play_stats: RefCell::new(CurrentPlayStats::None),
      on_queue_changed: RefCell::new(None),
    })
  }

  pub fn audio(&self) -> &AudioPlayer {
    &self.audio
  }

  pub fn window(&self) -> &Rc<ApplicationWindow> {
    &self.window
  }

  pub fn playlist_len(&self) -> u32 {
    self.playlist_store.n_items()
  }

  fn refresh_playlist_view(&self) {
    let n = self.playlist_store.n_items();
    if n > 0 {
      self.playlist_store.items_changed(0, n, n);
    }
  }

  fn get_item_at(&self, index: u32) -> Option<PlayableItem> {
    let item = self.playlist_store.item(index)?;
    let obj = item.downcast::<BoxedAnyObject>().ok()?;

    // Try Track first
    if let Ok(borrowed) = obj.try_borrow::<Rc<Track>>() {
      return Some(PlayableItem::LocalTrack(borrowed.clone()));
    }

    // Try YouTubeVideo
    if let Ok(borrowed) = obj.try_borrow::<Rc<YouTubeVideo>>() {
      return Some(PlayableItem::YouTubeVideo(borrowed.clone()));
    }

    None
  }

  fn show_error(&self, title: &str, message: &str) {
    let dialog = AlertDialog::builder()
      .modal(true)
      .message(title)
      .detail(message)
      .buttons(["OK"])
      .build();
    dialog.show(Some(&*self.window));
  }

  pub fn play_index(&self, index: u32) -> bool {
    let Some(item) = self.get_item_at(index) else {
      return false;
    };

    match item {
      PlayableItem::LocalTrack(track) => self.play_track(index, &track),
      PlayableItem::YouTubeVideo(video) => {
        self.play_youtube_video(&video, true);
        self.current_index.set(Some(index));
        true
      }
    }
  }

  fn play_track(&self, index: u32, track: &Track) -> bool {
    self.video_widget.stop();
    self.media_stack.set_visible_child_name("album_art");

    let filename = track.filename.clone();
    let artist = track.artist.clone();
    let album = track.album.clone();
    let title = track.title.clone();

    if !self.audio.is_available() {
      self.show_error("No Audio", "Audio playback is not available.");
      return false;
    }

    let file = match File::open(&filename) {
      Ok(f) => BufReader::new(f),
      Err(e) => {
        self.show_error(
          "Cannot open file",
          &format!("Failed to open '{filename}':\n{e}"),
        );
        return false;
      }
    };

    let source = match Decoder::new(file) {
      Ok(s) => s,
      Err(e) => {
        self.show_error(
          "Cannot decode file",
          &format!("Failed to decode '{filename}':\n{e}"),
        );
        return false;
      }
    };

    let duration = source.total_duration();
    let duration_secs = duration.map(|d| d.as_secs_f64()).unwrap_or(0.0);
    self.audio.play_source(source, duration);
    self.current_index.set(Some(index));
    self.playback_source.set(PlaybackSource::Local);
    add_track_to_recently_played(&filename);
    *self.play_stats.borrow_mut() = CurrentPlayStats::Track {
      filename: filename.clone(),
      duration_secs,
      counted: false,
    };
    self.refresh_playlist_view();

    if !self.try_set_embedded_cover_art(&filename) {
      let mut cover_path = PathBuf::from(&filename);
      cover_path.pop();
      cover_path.push("cover.jpg");
      self.album_art.set_filename(Some(cover_path));
    }

    self.window.set_title(Some(&format!(
      "fml9000 // {} - {} - {}",
      str_or_unknown(&artist),
      str_or_unknown(&album),
      str_or_unknown(&title),
    )));

    true
  }

  fn try_set_embedded_cover_art(&self, filename: &str) -> bool {
    let Ok(probe) = Probe::open(filename) else {
      return false;
    };
    let Ok(tagged_file) = probe.read() else {
      return false;
    };

    let tag = tagged_file
      .primary_tag()
      .or_else(|| tagged_file.first_tag());

    if let Some(t) = tag {
      let pictures = t.pictures();
      if let Some(picture) = pictures.first() {
        let data = picture.data().to_vec();
        let bytes = Bytes::from_owned(data);
        if let Ok(texture) = gdk::Texture::from_bytes(&bytes) {
          self.album_art.set_paintable(Some(&texture));
          return true;
        }
      }
    }
    false
  }

  pub fn play_youtube_video(&self, video: &YouTubeVideo, _audio_only: bool) {
    self.audio.stop();
    self.playback_source.set(PlaybackSource::YouTube);
    self.media_stack.set_visible_child_name("video");
    self.video_widget.play_youtube(&video.video_id);

    *self.play_stats.borrow_mut() = CurrentPlayStats::Video {
      id: video.id,
      duration_secs: video.duration_seconds.map(|s| s as f64).unwrap_or(0.0),
      counted: false,
    };
    self.refresh_playlist_view();

    self.window.set_title(Some(&format!(
      "fml9000 // YouTube - {}",
      video.title,
    )));
  }

  pub fn playback_source(&self) -> PlaybackSource {
    self.playback_source.get()
  }

  pub fn video_widget(&self) -> &Rc<VideoWidget> {
    &self.video_widget
  }

  pub fn check_play_threshold(&self, current_pos_secs: f64) {
    let action = {
      let mut stats = self.play_stats.borrow_mut();
      match &mut *stats {
        CurrentPlayStats::Track { filename, duration_secs, counted } => {
          if !*counted && *duration_secs > 0.0 && current_pos_secs >= *duration_secs * 0.5 {
            *counted = true;
            let fname = filename.clone();
            update_track_play_stats(&fname);
            Some((Some(fname), None))
          } else {
            None
          }
        }
        CurrentPlayStats::Video { id, duration_secs, counted } => {
          if !*counted && *duration_secs > 0.0 && current_pos_secs >= *duration_secs * 0.5 {
            *counted = true;
            let vid_id = *id;
            update_video_play_stats(vid_id);
            Some((None, Some(vid_id)))
          } else {
            None
          }
        }
        CurrentPlayStats::None => None,
      }
    };

    if let Some((track_filename, video_id)) = action {
      if let Some(fname) = track_filename {
        self.refresh_track_in_store(&fname);
      }
      if let Some(vid_id) = video_id {
        self.refresh_video_in_store(vid_id);
      }
    }
  }

  fn refresh_track_in_store(&self, filename: &str) {
    if let Some(updated_track) = load_track_by_filename(filename) {
      let n_items = self.playlist_store.n_items();
      for i in 0..n_items {
        if let Some(item) = self.playlist_store.item(i) {
          if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
            if let Ok(track) = obj.try_borrow::<Rc<Track>>() {
              if track.filename == filename {
                self.playlist_store.remove(i);
                self.playlist_store.insert(i, &BoxedAnyObject::new(updated_track));
                return;
              }
            }
          }
        }
      }
    }
  }

  fn refresh_video_in_store(&self, vid_id: i32) {
    if let Some(updated_video) = load_video_by_id(vid_id) {
      let n_items = self.playlist_store.n_items();
      for i in 0..n_items {
        if let Some(item) = self.playlist_store.item(i) {
          if let Ok(obj) = item.downcast::<BoxedAnyObject>() {
            if let Ok(video) = obj.try_borrow::<Rc<YouTubeVideo>>() {
              if video.id == vid_id {
                self.playlist_store.remove(i);
                self.playlist_store.insert(i, &BoxedAnyObject::new(updated_video));
                return;
              }
            }
          }
        }
      }
    }
  }

  pub fn stop(&self) {
    self.audio.stop();
    self.video_widget.stop();
    self.media_stack.set_visible_child_name("album_art");
    self.playback_source.set(PlaybackSource::None);
    *self.play_stats.borrow_mut() = CurrentPlayStats::None;
    self.refresh_playlist_view();
  }

  pub fn shuffle_enabled(&self) -> bool {
    self.shuffle_enabled.get()
  }

  pub fn set_shuffle_enabled(&self, enabled: bool) {
    self.shuffle_enabled.set(enabled);
  }

  pub fn repeat_mode(&self) -> RepeatMode {
    self.repeat_mode.get()
  }

  pub fn cycle_repeat_mode(&self) -> RepeatMode {
    let next = match self.repeat_mode.get() {
      RepeatMode::Off => RepeatMode::All,
      RepeatMode::All => RepeatMode::One,
      RepeatMode::One => RepeatMode::Off,
    };
    self.repeat_mode.set(next);
    next
  }

  pub fn queue_track(&self, filename: String) {
    add_track_to_queue(&filename);
    self.notify_queue_changed();
  }

  pub fn queue_video(&self, video_id: i32) {
    add_video_to_queue(video_id);
    self.notify_queue_changed();
  }

  pub fn set_on_queue_changed(&self, callback: Option<Rc<dyn Fn()>>) {
    *self.on_queue_changed.borrow_mut() = callback;
  }

  fn notify_queue_changed(&self) {
    if let Some(cb) = self.on_queue_changed.borrow().as_ref() {
      cb();
    }
  }

  pub fn remove_from_queue_by_filename(&self, filename: &str) {
    remove_track_from_queue(filename);
    self.notify_queue_changed();
  }

  pub fn remove_from_queue_by_video_id(&self, video_id: i32) {
    remove_video_from_queue(video_id);
    self.notify_queue_changed();
  }

  fn play_from_queue(&self) -> bool {
    if let Some(item) = pop_queue_front() {
      self.notify_queue_changed();
      match item {
        QueueItem::Track(track) => {
          let n_items = self.playlist_store.n_items();
          for i in 0..n_items {
            if let Some(playlist_item) = self.playlist_store.item(i) {
              if let Ok(obj) = playlist_item.downcast::<BoxedAnyObject>() {
                if let Ok(t) = obj.try_borrow::<Rc<Track>>() {
                  if t.filename == track.filename {
                    return self.play_index(i);
                  }
                }
              }
            }
          }
        }
        QueueItem::Video(video) => {
          let n_items = self.playlist_store.n_items();
          for i in 0..n_items {
            if let Some(playlist_item) = self.playlist_store.item(i) {
              if let Ok(obj) = playlist_item.downcast::<BoxedAnyObject>() {
                if let Ok(v) = obj.try_borrow::<Rc<YouTubeVideo>>() {
                  if v.id == video.id {
                    return self.play_index(i);
                  }
                }
              }
            }
          }
        }
      }
    }
    false
  }

  pub fn play_next(&self) -> bool {
    // Check queue first
    if queue_len() > 0 {
      return self.play_from_queue();
    }

    let len = self.playlist_len();
    if len == 0 {
      return false;
    }

    // Repeat One: replay the same track
    if self.repeat_mode.get() == RepeatMode::One {
      if let Some(idx) = self.current_index.get() {
        return self.play_index(idx);
      }
    }

    let next_index = if self.shuffle_enabled.get() {
      // Shuffle: pick random, but avoid same track if possible
      let mut rng = rand::thread_rng();
      if len == 1 {
        0
      } else {
        loop {
          let idx = rng.gen_range(0..len);
          if Some(idx) != self.current_index.get() {
            break idx;
          }
        }
      }
    } else {
      match self.current_index.get() {
        Some(idx) => {
          if idx + 1 < len {
            idx + 1
          } else if self.repeat_mode.get() == RepeatMode::All {
            0 // Wrap around
          } else {
            return false; // Stop at end when repeat is off
          }
        }
        None => 0,
      }
    };

    self.play_index(next_index)
  }

  pub fn play_prev(&self) -> bool {
    let len = self.playlist_len();
    if len == 0 {
      return false;
    }

    let prev_index = match self.current_index.get() {
      Some(idx) => {
        if idx > 0 {
          idx - 1
        } else {
          len - 1
        }
      }
      None => 0,
    };

    self.play_index(prev_index)
  }

  pub fn is_track_playing(&self, filename: &str) -> bool {
    let stats = self.play_stats.borrow();
    matches!(&*stats, CurrentPlayStats::Track { filename: f, .. } if f == filename)
  }

  pub fn is_video_playing(&self, video_id: i32) -> bool {
    let stats = self.play_stats.borrow();
    matches!(&*stats, CurrentPlayStats::Video { id, .. } if *id == video_id)
  }
}
