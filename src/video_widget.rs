use gtk::gio;
use gtk::prelude::*;
use gtk::{MediaFile, Video};
use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

pub struct VideoWidget {
    video: Video,
    video_active: Cell<bool>,
    current_url: RefCell<Option<String>>,
}

impl VideoWidget {
    pub fn new() -> Rc<Self> {
        let video = Video::new();
        video.set_hexpand(true);
        video.set_vexpand(true);
        video.set_autoplay(true);

        Rc::new(Self {
            video,
            video_active: Cell::new(false),
            current_url: RefCell::new(None),
        })
    }

    pub fn widget(&self) -> &Video {
        &self.video
    }

    pub fn play(&self, url: &str) {
        eprintln!("VideoWidget::play() with URL: {}", &url[..url.len().min(100)]);

        // Use MediaFile with gio::File for HTTP URLs
        if url.starts_with("http://") || url.starts_with("https://") {
            let file = gio::File::for_uri(url);
            let media_file = MediaFile::for_file(&file);
            media_file.set_playing(true);
            self.video.set_media_stream(Some(&media_file));
        } else {
            self.video.set_filename(Some(url));
        }

        self.video_active.set(true);
        *self.current_url.borrow_mut() = Some(url.to_string());
    }

    pub fn play_youtube(&self, video_id: &str) {
        eprintln!("VideoWidget::play_youtube() with video_id: {video_id}");

        // Use yt-dlp to get the actual stream URL
        let url = format!("https://www.youtube.com/watch?v={video_id}");

        // Try to get a direct video+audio URL (not a manifest)
        // Using format 18 which is usually a direct mp4 with video+audio
        match Command::new("yt-dlp")
            .args([
                "-f", "18/22/best[ext=mp4]/best",  // Prefer direct mp4 formats
                "-g",  // Get URL only
                &url
            ])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let stream_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    eprintln!("Got stream URL: {}", &stream_url[..stream_url.len().min(100)]);
                    self.play(&stream_url);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eprintln!("yt-dlp failed: {stderr}");
                }
            }
            Err(e) => {
                eprintln!("Failed to run yt-dlp: {e}");
            }
        }
    }

    pub fn stop(&self) {
        // Clear the media file to stop playback
        self.video.set_filename(None::<&str>);
        self.video_active.set(false);
        *self.current_url.borrow_mut() = None;
    }

    pub fn pause(&self) {
        if let Some(media_stream) = self.video.media_stream() {
            media_stream.pause();
        }
    }

    pub fn unpause(&self) {
        if let Some(media_stream) = self.video.media_stream() {
            media_stream.play();
        }
    }

    pub fn toggle_pause(&self) {
        if let Some(media_stream) = self.video.media_stream() {
            if media_stream.is_playing() {
                media_stream.pause();
            } else {
                media_stream.play();
            }
        }
    }

    pub fn seek(&self, position: Duration) {
        if let Some(media_stream) = self.video.media_stream() {
            media_stream.seek(position.as_micros() as i64);
        }
    }

    pub fn set_volume(&self, volume: f64) {
        if let Some(media_stream) = self.video.media_stream() {
            media_stream.set_volume(volume);
        }
    }

    pub fn get_time_pos(&self) -> Option<Duration> {
        let media_stream = self.video.media_stream()?;
        let timestamp = media_stream.timestamp();
        if timestamp >= 0 {
            Some(Duration::from_micros(timestamp as u64))
        } else {
            None
        }
    }

    pub fn get_duration(&self) -> Option<Duration> {
        let media_stream = self.video.media_stream()?;
        let duration = media_stream.duration();
        if duration > 0 {
            Some(Duration::from_micros(duration as u64))
        } else {
            None
        }
    }

    pub fn is_video_active(&self) -> bool {
        self.video_active.get()
    }

    pub fn is_playing(&self) -> bool {
        if let Some(media_stream) = self.video.media_stream() {
            media_stream.is_playing()
        } else {
            false
        }
    }
}

impl Default for VideoWidget {
    fn default() -> Self {
        // This shouldn't be used directly, use new() instead
        Self {
            video: Video::new(),
            video_active: Cell::new(false),
            current_url: RefCell::new(None),
        }
    }
}
