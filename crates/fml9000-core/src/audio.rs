use rodio::{OutputStream, OutputStreamBuilder, Sink};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

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
    let stream = OutputStreamBuilder::open_default_stream()
      .map_err(|e| format!("Failed to initialize audio output: {e}"))?;
    let sink = Sink::connect_new(&stream.mixer());
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
    S::Item: rodio::cpal::Sample + Send,
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

  pub fn is_paused(&self) -> bool {
    if let Some(audio) = self.inner.borrow().as_ref() {
      audio.sink.is_paused()
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

impl Default for AudioPlayer {
  fn default() -> Self {
    Self::new().0
  }
}
