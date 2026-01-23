use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, GraphicsOffloadEnabled, Label, MediaFile, Orientation, Spinner, Stack, Video};
use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

pub fn open_in_browser(video_id: &str) {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let _ = Command::new("xdg-open").arg(&url).spawn();
}

pub struct VideoWidget {
    stack: Stack,
    video: Video,
    spinner: Spinner,
    video_active: Cell<bool>,
    current_url: RefCell<Option<String>>,
}

impl VideoWidget {
    pub fn new() -> Rc<Self> {
        let video = Video::new();
        video.set_hexpand(true);
        video.set_vexpand(true);
        video.set_autoplay(true);
        video.set_graphics_offload(GraphicsOffloadEnabled::Enabled);

        // Create loading view with spinner
        let loading_box = GtkBox::new(Orientation::Vertical, 12);
        loading_box.set_halign(Align::Center);
        loading_box.set_valign(Align::Center);

        let spinner = Spinner::new();
        spinner.set_size_request(48, 48);

        let loading_label = Label::new(Some("Loading video..."));
        loading_label.add_css_class("dim-label");

        loading_box.append(&spinner);
        loading_box.append(&loading_label);

        // Create stack to switch between loading and video
        let stack = Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.add_named(&loading_box, Some("loading"));
        stack.add_named(&video, Some("video"));
        stack.set_visible_child_name("video");

        Rc::new(Self {
            stack,
            video,
            spinner,
            video_active: Cell::new(false),
            current_url: RefCell::new(None),
        })
    }

    pub fn widget(&self) -> &Stack {
        &self.stack
    }

    fn show_loading(&self) {
        self.spinner.start();
        self.stack.set_visible_child_name("loading");
    }

    fn show_video(&self) {
        self.spinner.stop();
        self.stack.set_visible_child_name("video");
    }

    pub fn play(&self, url: &str) {

        // Use MediaFile with gio::File for HTTP URLs
        if url.starts_with("http://") || url.starts_with("https://") {
            let file = gio::File::for_uri(url);
            let media_file = MediaFile::for_file(&file);
            media_file.set_playing(true);
            self.video.set_media_stream(Some(&media_file));
        } else {
            self.video.set_filename(Some(url));
        }

        self.show_video();
        self.video_active.set(true);
        *self.current_url.borrow_mut() = Some(url.to_string());
    }

    pub fn play_youtube(self: &Rc<Self>, video_id: &str) {
        eprintln!("VideoWidget::play_youtube() with video_id: {video_id}");

        // Show loading state
        self.show_loading();

        let url = format!("https://www.youtube.com/watch?v={video_id}");
        let widget = Rc::clone(self);

        // Use std channel + glib timeout to poll for result
        let (sender, receiver) = std::sync::mpsc::channel::<Result<String, String>>();

        std::thread::spawn(move || {
            let result = Command::new("yt-dlp")
                .args([
                    "-f", "18/22/best[ext=mp4]/best",
                    "-g",
                    &url
                ])
                .output();

            let message = match result {
                Ok(output) => {
                    if output.status.success() {
                        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
                    } else {
                        Err(String::from_utf8_lossy(&output.stderr).to_string())
                    }
                }
                Err(e) => Err(format!("Failed to run yt-dlp: {e}")),
            };
            let _ = sender.send(message);
        });

        // Poll for result from main thread
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            match receiver.try_recv() {
                Ok(Ok(stream_url)) => {
                    eprintln!("Got stream URL: {}", &stream_url[..stream_url.len().min(100)]);
                    widget.play(&stream_url);
                    glib::ControlFlow::Break
                }
                Ok(Err(e)) => {
                    eprintln!("yt-dlp error: {e}");
                    widget.show_video();
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    glib::ControlFlow::Continue
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    eprintln!("yt-dlp channel disconnected");
                    widget.show_video();
                    glib::ControlFlow::Break
                }
            }
        });
    }

    pub fn stop(&self) {
        self.video.set_filename(None::<&str>);
        self.video_active.set(false);
        *self.current_url.borrow_mut() = None;
        self.show_video();
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

    pub fn is_playing(&self) -> bool {
        if let Some(media_stream) = self.video.media_stream() {
            media_stream.is_playing()
        } else {
            false
        }
    }

    pub fn is_ended(&self) -> bool {
        if let Some(media_stream) = self.video.media_stream() {
            media_stream.is_ended()
        } else {
            false
        }
    }

    pub fn bind_to_other(&self, other: &Rc<VideoWidget>) {
        self.video.bind_property("media-stream", &other.video, "media-stream")
            .sync_create()
            .build();
        self.stack.bind_property("visible-child-name", &other.stack, "visible-child-name")
            .sync_create()
            .build();
    }
}

impl Default for VideoWidget {
    fn default() -> Self {
        // This shouldn't be used directly, use new() instead
        Self {
            stack: Stack::new(),
            video: Video::new(),
            spinner: Spinner::new(),
            video_active: Cell::new(false),
            current_url: RefCell::new(None),
        }
    }
}
