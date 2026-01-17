use crate::gtk_helpers::str_or_unknown;
use crate::video_widget::VideoWidget;
use crate::AudioPlayer;
use fml9000::{add_track_to_recently_played, update_track_play_stats};
use fml9000::models::{Track, YouTubeVideo};
use gtk::gdk;
use gtk::gio::ListStore;
use gtk::glib::{Bytes, BoxedAnyObject};
use gtk::prelude::*;
use gtk::{AlertDialog, ApplicationWindow, Picture, Stack};
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use rodio::source::Source;
use rodio::Decoder;
use std::cell::Cell;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;

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
  album_art: Rc<Picture>,
  window: Rc<ApplicationWindow>,
}

impl PlaybackController {
  pub fn new(
    audio: AudioPlayer,
    playlist_store: ListStore,
    album_art: Rc<Picture>,
    video_widget: Rc<VideoWidget>,
    media_stack: Rc<Stack>,
    window: Rc<ApplicationWindow>,
  ) -> Rc<Self> {
    Rc::new(Self {
      audio,
      video_widget,
      media_stack,
      playlist_store,
      current_index: Cell::new(None),
      playback_source: Cell::new(PlaybackSource::None),
      album_art,
      window,
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
    self.audio.play_source(source, duration);
    self.current_index.set(Some(index));
    self.playback_source.set(PlaybackSource::Local);
    add_track_to_recently_played(&filename);
    update_track_play_stats(&filename);

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

  pub fn stop(&self) {
    self.audio.stop();
    self.video_widget.stop();
    self.media_stack.set_visible_child_name("album_art");
    self.playback_source.set(PlaybackSource::None);
  }

  pub fn play_next(&self) -> bool {
    let len = self.playlist_len();
    if len == 0 {
      return false;
    }

    let next_index = match self.current_index.get() {
      Some(idx) => {
        if idx + 1 < len {
          idx + 1
        } else {
          0
        }
      }
      None => 0,
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
}
