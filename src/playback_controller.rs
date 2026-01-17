use crate::gtk_helpers::str_or_unknown;
use crate::mpv_player::MpvPlayer;
use crate::AudioPlayer;
use fml9000::add_track_to_recently_played;
use fml9000::models::{Track, YouTubeVideo};
use gtk::gio::ListStore;
use gtk::glib::BoxedAnyObject;
use gtk::prelude::*;
use gtk::{AlertDialog, ApplicationWindow, Image};
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
pub enum PlayableItem {
  LocalTrack(Rc<Track>),
  YouTubeVideo(Rc<YouTubeVideo>),
}

pub struct PlaybackController {
  audio: AudioPlayer,
  mpv: Rc<MpvPlayer>,
  playlist_store: ListStore,
  current_index: Cell<Option<u32>>,
  playback_source: Cell<PlaybackSource>,
  album_art: Rc<Image>,
  window: Rc<ApplicationWindow>,
}

impl PlaybackController {
  pub fn new(
    audio: AudioPlayer,
    playlist_store: ListStore,
    album_art: Rc<Image>,
    window: Rc<ApplicationWindow>,
  ) -> Rc<Self> {
    Rc::new(Self {
      audio,
      mpv: MpvPlayer::new(),
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

  pub fn current_index(&self) -> Option<u32> {
    self.current_index.get()
  }

  pub fn playlist_len(&self) -> u32 {
    self.playlist_store.n_items()
  }

  fn get_track_at(&self, index: u32) -> Option<Rc<Track>> {
    let item = self.playlist_store.item(index)?;
    let obj = item.downcast::<BoxedAnyObject>().ok()?;
    let borrowed: std::cell::Ref<'_, Rc<Track>> = obj.borrow();
    Some(borrowed.clone())
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
    self.mpv.stop();

    let Some(track) = self.get_track_at(index) else {
      return false;
    };

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

    let mut cover_path = PathBuf::from(&filename);
    cover_path.pop();
    cover_path.push("cover.jpg");
    self.album_art.set_from_file(Some(cover_path));

    self.window.set_title(Some(&format!(
      "fml9000 // {} - {} - {}",
      str_or_unknown(&artist),
      str_or_unknown(&album),
      str_or_unknown(&title),
    )));

    true
  }

  pub fn play_youtube_video(&self, video: &YouTubeVideo, audio_only: bool) {
    self.audio.stop();
    self.playback_source.set(PlaybackSource::YouTube);

    if audio_only {
      self.mpv.play_audio(&video.video_id);
    } else {
      self.mpv.play_video(&video.video_id);
    }

    self.window.set_title(Some(&format!(
      "fml9000 // YouTube - {}",
      video.title,
    )));
  }

  pub fn playback_source(&self) -> PlaybackSource {
    self.playback_source.get()
  }

  pub fn mpv(&self) -> &Rc<MpvPlayer> {
    &self.mpv
  }

  pub fn stop(&self) {
    self.audio.stop();
    self.mpv.stop();
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
