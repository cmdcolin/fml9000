use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct VaporwaveDecoder {
  _process: Child,
}

impl VaporwaveDecoder {
  /// Spawn ffmpeg process for vaporwave effect and return the process
  /// The caller should read from process.stdout
  pub fn spawn(file_path: &str) -> Result<Self, String> {
    let child = Command::new("ffmpeg")
      .args([
        "-i", file_path,
        "-af", "asetrate=44100*0.66,aresample=44100",
        "-f", "wav",
        "-"
      ])
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

    Ok(Self {
      _process: child,
    })
  }

  /// Get mutable reference to process for reading stdout
  pub fn process_mut(&mut self) -> &mut Child {
    &mut self._process
  }
}

impl Drop for VaporwaveDecoder {
  fn drop(&mut self) {
    let _ = self._process.kill();
    let _ = self._process.wait();
  }
}

pub fn check_ffmpeg_available() -> Result<(), String> {
  Command::new("ffmpeg")
    .arg("-version")
    .output()
    .map_err(|e| format!(
      "FFmpeg not found: {}.\n\nPlease install ffmpeg:\n\
       • Ubuntu/Debian: sudo apt install ffmpeg\n\
       • macOS: brew install ffmpeg\n\
       • Arch: sudo pacman -S ffmpeg",
      e
    ))?;
  Ok(())
}

pub fn calculate_vaporwave_duration(original: Option<Duration>) -> Option<Duration> {
  original.map(|d| Duration::from_secs_f64(d.as_secs_f64() * 1.515))
}

pub fn check_yt_dlp_available() -> Result<(), String> {
  Command::new("yt-dlp")
    .arg("--version")
    .output()
    .map_err(|e| format!(
      "yt-dlp not found: {}.\n\nPlease install yt-dlp:\n\
       • Ubuntu/Debian: pip install yt-dlp\n\
       • macOS: brew install yt-dlp\n\
       • Arch: sudo pacman -S yt-dlp\n\
       • Or: pipx install yt-dlp",
      e
    ))?;
  Ok(())
}

pub struct YouTubeVaporwaveDecoder {
  _ffmpeg_process: Child,
}

impl YouTubeVaporwaveDecoder {
  /// Spawn yt-dlp → ffmpeg pipeline for YouTube vaporwave effect
  pub fn spawn(video_id: &str) -> Result<Self, String> {
    let video_url = format!("https://www.youtube.com/watch?v={}", video_id);

    let mut yt_dlp = Command::new("yt-dlp")
      .args([
        "-f", "bestaudio[ext=m4a]/bestaudio",
        "-o", "-",
        &video_url,
      ])
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|e| format!("Failed to spawn yt-dlp: {}", e))?;

    let yt_dlp_stdout = yt_dlp.stdout.take()
      .ok_or_else(|| "Failed to capture yt-dlp stdout".to_string())?;

    let ffmpeg_child = Command::new("ffmpeg")
      .args([
        "-i", "pipe:0",
        "-af", "asetrate=44100*0.66,aresample=44100",
        "-f", "wav",
        "-",
      ])
      .stdin(yt_dlp_stdout)
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

    Ok(Self {
      _ffmpeg_process: ffmpeg_child,
    })
  }

  /// Get mutable reference to process for reading stdout
  pub fn process_mut(&mut self) -> &mut Child {
    &mut self._ffmpeg_process
  }
}

impl Drop for YouTubeVaporwaveDecoder {
  fn drop(&mut self) {
    let _ = self._ffmpeg_process.kill();
    let _ = self._ffmpeg_process.wait();
  }
}
