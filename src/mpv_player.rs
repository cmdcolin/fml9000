use std::cell::RefCell;
use std::process::{Child, Command};
use std::rc::Rc;

pub struct MpvPlayer {
  process: RefCell<Option<Child>>,
}

impl MpvPlayer {
  pub fn new() -> Rc<Self> {
    Rc::new(Self {
      process: RefCell::new(None),
    })
  }

  pub fn play(&self, video_id: &str, audio_only: bool) {
    self.stop();

    let url = format!("https://www.youtube.com/watch?v={video_id}");

    let mut cmd = Command::new("mpv");
    if audio_only {
      cmd.arg("--no-video");
    }
    cmd.arg("--force-window=no");
    cmd.arg(&url);

    match cmd.spawn() {
      Ok(child) => {
        *self.process.borrow_mut() = Some(child);
      }
      Err(e) => {
        eprintln!("Failed to start mpv: {e}");
      }
    }
  }

  pub fn play_audio(&self, video_id: &str) {
    self.play(video_id, true);
  }

  pub fn play_video(&self, video_id: &str) {
    self.play(video_id, false);
  }

  pub fn stop(&self) {
    if let Some(mut child) = self.process.borrow_mut().take() {
      let _ = child.kill();
      let _ = child.wait();
    }
  }

  pub fn is_playing(&self) -> bool {
    let mut borrowed = self.process.borrow_mut();
    if let Some(child) = borrowed.as_mut() {
      match child.try_wait() {
        Ok(Some(_)) => {
          *borrowed = None;
          false
        }
        Ok(None) => true,
        Err(_) => {
          *borrowed = None;
          false
        }
      }
    } else {
      false
    }
  }
}

impl Drop for MpvPlayer {
  fn drop(&mut self) {
    if let Some(mut child) = self.process.borrow_mut().take() {
      let _ = child.kill();
      let _ = child.wait();
    }
  }
}

pub fn open_in_browser(video_id: &str) {
  let url = format!("https://www.youtube.com/watch?v={video_id}");

  #[cfg(target_os = "linux")]
  let _ = Command::new("xdg-open").arg(&url).spawn();

  #[cfg(target_os = "macos")]
  let _ = Command::new("open").arg(&url).spawn();

  #[cfg(target_os = "windows")]
  let _ = Command::new("cmd").args(["/C", "start", &url]).spawn();
}
