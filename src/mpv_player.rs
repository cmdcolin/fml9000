use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command};
use std::rc::Rc;
use std::time::Duration;

pub struct MpvPlayer {
  process: RefCell<Option<Child>>,
  socket_path: RefCell<Option<String>>,
  stream: RefCell<Option<UnixStream>>,
}

#[derive(Serialize)]
struct MpvCommand {
  command: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct MpvResponse {
  data: Option<serde_json::Value>,
  error: String,
}

impl MpvPlayer {
  pub fn new() -> Rc<Self> {
    Rc::new(Self {
      process: RefCell::new(None),
      socket_path: RefCell::new(None),
      stream: RefCell::new(None),
    })
  }

  pub fn play(&self, video_id: &str, audio_only: bool) {
    self.stop();

    let socket_path = format!("/tmp/fml9000-mpv-{}.sock", std::process::id());
    let url = format!("https://www.youtube.com/watch?v={video_id}");

    let mut cmd = Command::new("mpv");
    cmd.arg(format!("--input-ipc-server={}", socket_path));
    if audio_only {
      cmd.arg("--no-video");
    }
    cmd.arg("--force-window=no");
    cmd.arg(&url);

    match cmd.spawn() {
      Ok(child) => {
        *self.process.borrow_mut() = Some(child);
        *self.socket_path.borrow_mut() = Some(socket_path.clone());

        // Try to connect to the socket (with retries since mpv takes time to create it)
        std::thread::spawn(move || {
          // Give mpv time to create the socket
          std::thread::sleep(Duration::from_millis(500));
        });
      }
      Err(e) => {
        eprintln!("Failed to start mpv: {e}");
      }
    }
  }

  fn ensure_connected(&self) -> bool {
    if self.stream.borrow().is_some() {
      return true;
    }

    let socket_path = self.socket_path.borrow();
    if let Some(path) = socket_path.as_ref() {
      for _ in 0..10 {
        if let Ok(stream) = UnixStream::connect(path) {
          stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .ok();
          stream
            .set_write_timeout(Some(Duration::from_millis(100)))
            .ok();
          drop(socket_path);
          *self.stream.borrow_mut() = Some(stream);
          return true;
        }
        std::thread::sleep(Duration::from_millis(100));
      }
    }
    false
  }

  fn send_command(&self, command: Vec<serde_json::Value>) -> Option<serde_json::Value> {
    if !self.ensure_connected() {
      return None;
    }

    let cmd = MpvCommand { command };
    let mut json = serde_json::to_string(&cmd).ok()?;
    json.push('\n');

    {
      let mut stream_ref = self.stream.borrow_mut();
      let stream = stream_ref.as_mut()?;

      if stream.write_all(json.as_bytes()).is_err() {
        drop(stream_ref);
        *self.stream.borrow_mut() = None;
        return None;
      }
    }

    let response = {
      let mut stream_ref = self.stream.borrow_mut();
      let stream = stream_ref.as_mut()?;
      let mut reader = BufReader::new(stream);
      let mut response = String::new();
      if reader.read_line(&mut response).is_err() {
        drop(reader);
        drop(stream_ref);
        *self.stream.borrow_mut() = None;
        return None;
      }
      response
    };

    let parsed: MpvResponse = serde_json::from_str(&response).ok()?;
    if parsed.error == "success" {
      parsed.data
    } else {
      None
    }
  }

  pub fn get_time_pos(&self) -> Option<Duration> {
    let data = self.send_command(vec!["get_property".into(), "time-pos".into()])?;
    let secs = data.as_f64()?;
    if secs >= 0.0 {
      Some(Duration::from_secs_f64(secs))
    } else {
      None
    }
  }

  pub fn get_duration(&self) -> Option<Duration> {
    let data = self.send_command(vec!["get_property".into(), "duration".into()])?;
    let secs = data.as_f64()?;
    if secs > 0.0 {
      Some(Duration::from_secs_f64(secs))
    } else {
      None
    }
  }

  pub fn seek(&self, position: Duration) {
    let secs = position.as_secs_f64();
    self.send_command(vec!["seek".into(), secs.into(), "absolute".into()]);
  }

  pub fn pause(&self) {
    self.send_command(vec!["set_property".into(), "pause".into(), true.into()]);
  }

  pub fn unpause(&self) {
    self.send_command(vec!["set_property".into(), "pause".into(), false.into()]);
  }

  pub fn toggle_pause(&self) {
    self.send_command(vec!["cycle".into(), "pause".into()]);
  }

  pub fn play_audio(&self, video_id: &str) {
    self.play(video_id, true);
  }

  pub fn play_video(&self, video_id: &str) {
    self.play(video_id, false);
  }

  pub fn stop(&self) {
    *self.stream.borrow_mut() = None;

    if let Some(path) = self.socket_path.borrow_mut().take() {
      let _ = std::fs::remove_file(&path);
    }

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
          drop(borrowed);
          *self.stream.borrow_mut() = None;
          if let Some(path) = self.socket_path.borrow_mut().take() {
            let _ = std::fs::remove_file(&path);
          }
          false
        }
        Ok(None) => true,
        Err(_) => {
          *borrowed = None;
          drop(borrowed);
          *self.stream.borrow_mut() = None;
          if let Some(path) = self.socket_path.borrow_mut().take() {
            let _ = std::fs::remove_file(&path);
          }
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
    if let Some(path) = self.socket_path.borrow_mut().take() {
      let _ = std::fs::remove_file(&path);
    }

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
