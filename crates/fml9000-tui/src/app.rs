use fml9000_core::{
    AudioPlayer, Track, YouTubeVideo, Playlist, QueueItem,
    load_tracks, get_user_playlists, get_queue_items,
    load_recently_played_items, load_recently_added_items, queue_len, pop_queue_front,
    add_track_to_queue, add_track_to_recently_played, update_track_play_stats,
};
use fml9000_core::settings::{CoreSettings, RepeatMode};
use ratatui::widgets::TableState;
use rodio::source::Source;
use rodio::Decoder;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Navigation,
    TrackList,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NavSection {
    AllTracks,
    Playlists,
    Queue,
    RecentlyPlayed,
    RecentlyAdded,
}

pub struct App {
    pub audio: AudioPlayer,
    pub audio_error: Option<String>,
    pub tracks: Vec<Arc<Track>>,
    pub playlists: Vec<Arc<Playlist>>,
    pub displayed_items: Vec<DisplayItem>,
    pub nav_state: TableState,
    pub track_state: TableState,
    pub active_panel: Panel,
    pub current_nav: NavSection,
    pub current_playlist_id: Option<i32>,
    pub now_playing: Option<NowPlaying>,
    pub shuffle_enabled: bool,
    pub repeat_mode: RepeatMode,
    pub settings: CoreSettings,
    pub search_query: String,
    pub is_searching: bool,
    pub filtered_indices: Vec<usize>,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub scroll_offset: usize,
    pub visible_height: usize,
}

#[derive(Clone)]
pub struct NowPlaying {
    pub track: Arc<Track>,
    pub duration: Option<Duration>,
    pub started_at: std::time::Instant,
    pub play_counted: bool,
}

#[derive(Clone)]
pub enum DisplayItem {
    Track(Arc<Track>),
    Video(Arc<YouTubeVideo>),
}

impl DisplayItem {
    pub fn title(&self) -> &str {
        match self {
            DisplayItem::Track(t) => t.title.as_deref().unwrap_or("Unknown"),
            DisplayItem::Video(v) => &v.title,
        }
    }

    pub fn artist(&self) -> &str {
        match self {
            DisplayItem::Track(t) => t.artist.as_deref().unwrap_or("Unknown"),
            DisplayItem::Video(_) => "YouTube",
        }
    }

    pub fn album(&self) -> &str {
        match self {
            DisplayItem::Track(t) => t.album.as_deref().unwrap_or("Unknown"),
            DisplayItem::Video(_) => "",
        }
    }

    pub fn duration_str(&self) -> String {
        let secs = match self {
            DisplayItem::Track(t) => t.duration_seconds,
            DisplayItem::Video(v) => v.duration_seconds,
        };
        match secs {
            Some(s) => format!("{}:{:02}", s / 60, s % 60),
            None => "?:??".to_string(),
        }
    }
}

impl App {
    pub fn new() -> Self {
        info!("App::new() starting");
        let start = Instant::now();

        let (audio, audio_error) = AudioPlayer::new();
        info!("Audio initialized in {:?}", start.elapsed());

        let settings = fml9000_core::settings::read_settings::<CoreSettings>();
        info!("Settings loaded in {:?}", start.elapsed());

        let tracks_start = Instant::now();
        let tracks = load_tracks().unwrap_or_default();
        info!("Loaded {} tracks in {:?}", tracks.len(), tracks_start.elapsed());

        let playlists_start = Instant::now();
        let playlists = get_user_playlists().unwrap_or_default();
        info!("Loaded {} playlists in {:?}", playlists.len(), playlists_start.elapsed());

        let displayed_items: Vec<DisplayItem> = tracks.iter()
            .map(|t| DisplayItem::Track(t.clone()))
            .collect();

        let mut nav_state = TableState::default();
        nav_state.select(Some(0));

        let mut track_state = TableState::default();
        if !displayed_items.is_empty() {
            track_state.select(Some(0));
        }

        info!("App::new() completed in {:?}", start.elapsed());

        App {
            audio,
            audio_error,
            tracks,
            playlists,
            displayed_items,
            nav_state,
            track_state,
            active_panel: Panel::TrackList,
            current_nav: NavSection::AllTracks,
            current_playlist_id: None,
            now_playing: None,
            shuffle_enabled: settings.shuffle_enabled,
            repeat_mode: settings.repeat_mode,
            settings,
            search_query: String::new(),
            is_searching: false,
            filtered_indices: Vec::new(),
            should_quit: false,
            status_message: None,
            scroll_offset: 0,
            visible_height: 20,
        }
    }

    pub fn on_tick(&mut self) {
        // Check if current track finished
        if let Some(ref np) = self.now_playing {
            let elapsed = np.started_at.elapsed();

            // Update play count at 50% threshold
            if !np.play_counted {
                if let Some(duration) = np.duration {
                    if elapsed >= duration / 2 {
                        update_track_play_stats(&np.track.filename);
                        if let Some(ref mut np) = self.now_playing {
                            np.play_counted = true;
                        }
                    }
                }
            }

            // Check if track ended
            if self.audio.is_empty() {
                self.play_next();
            }
        }
    }

    pub fn nav_items(&self) -> Vec<&str> {
        vec!["All Tracks", "Playlists", "Queue", "Recently Played", "Recently Added"]
    }

    pub fn nav_down(&mut self) {
        let len = self.nav_items().len();
        if len == 0 {
            return;
        }
        let i = self.nav_state.selected().unwrap_or(0);
        self.nav_state.select(Some((i + 1) % len));
    }

    pub fn nav_up(&mut self) {
        let len = self.nav_items().len();
        if len == 0 {
            return;
        }
        let i = self.nav_state.selected().unwrap_or(0);
        self.nav_state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn track_down(&mut self) {
        let len = if self.is_searching && !self.filtered_indices.is_empty() {
            self.filtered_indices.len()
        } else {
            self.displayed_items.len()
        };
        if len == 0 {
            return;
        }
        let i = self.track_state.selected().unwrap_or(0);
        self.track_state.select(Some((i + 1) % len));
    }

    pub fn track_up(&mut self) {
        let len = if self.is_searching && !self.filtered_indices.is_empty() {
            self.filtered_indices.len()
        } else {
            self.displayed_items.len()
        };
        if len == 0 {
            return;
        }
        let i = self.track_state.selected().unwrap_or(0);
        self.track_state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }

    pub fn select_nav(&mut self) {
        let selected = self.nav_state.selected().unwrap_or(0);
        match selected {
            0 => self.load_all_tracks(),
            1 => self.load_playlists_view(),
            2 => self.load_queue(),
            3 => self.load_recently_played(),
            4 => self.load_recently_added(),
            _ => {}
        }
        self.track_state.select(if self.displayed_items.is_empty() { None } else { Some(0) });
    }

    fn load_all_tracks(&mut self) {
        self.current_nav = NavSection::AllTracks;
        self.current_playlist_id = None;
        self.displayed_items = self.tracks.iter()
            .map(|t| DisplayItem::Track(t.clone()))
            .collect();
    }

    fn load_playlists_view(&mut self) {
        self.current_nav = NavSection::Playlists;
        // For now, show all tracks. Could expand to show playlist list
        self.displayed_items = self.tracks.iter()
            .map(|t| DisplayItem::Track(t.clone()))
            .collect();
    }

    fn load_queue(&mut self) {
        self.current_nav = NavSection::Queue;
        self.current_playlist_id = None;
        let items = get_queue_items();
        self.displayed_items = items.into_iter().map(|item| {
            match item {
                QueueItem::Track(t) => DisplayItem::Track(t),
                QueueItem::Video(v) => DisplayItem::Video(v),
            }
        }).collect();
    }

    fn load_recently_played(&mut self) {
        self.current_nav = NavSection::RecentlyPlayed;
        self.current_playlist_id = None;
        let items = load_recently_played_items(100);
        self.displayed_items = items.into_iter().map(|item| {
            match item {
                QueueItem::Track(t) => DisplayItem::Track(t),
                QueueItem::Video(v) => DisplayItem::Video(v),
            }
        }).collect();
    }

    fn load_recently_added(&mut self) {
        self.current_nav = NavSection::RecentlyAdded;
        self.current_playlist_id = None;
        let items = load_recently_added_items(100);
        self.displayed_items = items.into_iter().map(|item| {
            match item {
                QueueItem::Track(t) => DisplayItem::Track(t),
                QueueItem::Video(v) => DisplayItem::Video(v),
            }
        }).collect();
    }

    pub fn play_selected(&mut self) {
        let selected_idx = if self.is_searching && !self.filtered_indices.is_empty() {
            self.track_state.selected()
                .and_then(|i| self.filtered_indices.get(i).copied())
        } else {
            self.track_state.selected()
        };

        if let Some(idx) = selected_idx {
            if let Some(item) = self.displayed_items.get(idx) {
                match item {
                    DisplayItem::Track(track) => {
                        self.play_track(track.clone());
                    }
                    DisplayItem::Video(_) => {
                        self.status_message = Some("YouTube playback not supported in TUI".to_string());
                    }
                }
            }
        }
    }

    fn play_track(&mut self, track: Arc<Track>) {
        let filename = &track.filename;

        let file = match File::open(filename) {
            Ok(f) => BufReader::new(f),
            Err(e) => {
                self.status_message = Some(format!("Cannot open file: {}", e));
                return;
            }
        };

        let source: Decoder<BufReader<File>> = match Decoder::new(file) {
            Ok(s) => s,
            Err(e) => {
                self.status_message = Some(format!("Cannot decode file: {}", e));
                return;
            }
        };

        let duration = source.total_duration();
        self.audio.play_source(source, duration);
        add_track_to_recently_played(filename);

        self.now_playing = Some(NowPlaying {
            track,
            duration,
            started_at: std::time::Instant::now(),
            play_counted: false,
        });

        self.status_message = None;
    }

    pub fn toggle_pause(&mut self) {
        if self.audio.is_playing() {
            self.audio.pause();
        } else {
            self.audio.play();
        }
    }

    pub fn stop(&mut self) {
        self.audio.stop();
        self.now_playing = None;
    }

    pub fn play_next(&mut self) {
        // Check queue first
        if queue_len() > 0 {
            if let Some(item) = pop_queue_front() {
                match item {
                    QueueItem::Track(track) => {
                        self.play_track(track);
                        return;
                    }
                    QueueItem::Video(_) => {
                        self.status_message = Some("YouTube not supported in TUI".to_string());
                    }
                }
            }
        }

        if self.repeat_mode == RepeatMode::One {
            if let Some(ref np) = self.now_playing {
                self.play_track(np.track.clone());
                return;
            }
        }

        // Find current track index and play next
        if let Some(ref np) = self.now_playing.clone() {
            if let Some(current_idx) = self.displayed_items.iter().position(|item| {
                if let DisplayItem::Track(t) = item {
                    t.filename == np.track.filename
                } else {
                    false
                }
            }) {
                let len = self.displayed_items.len();
                if len == 0 {
                    return;
                }

                let next_idx = if self.shuffle_enabled {
                    use rand::Rng;
                    let mut rng = rand::rng();
                    rng.random_range(0..len)
                } else if current_idx + 1 < len {
                    current_idx + 1
                } else if self.repeat_mode == RepeatMode::All {
                    0
                } else {
                    self.now_playing = None;
                    return;
                };

                if let Some(DisplayItem::Track(track)) = self.displayed_items.get(next_idx) {
                    self.play_track(track.clone());
                    self.track_state.select(Some(next_idx));
                }
            }
        }
    }

    pub fn play_prev(&mut self) {
        if let Some(ref np) = self.now_playing.clone() {
            if let Some(current_idx) = self.displayed_items.iter().position(|item| {
                if let DisplayItem::Track(t) = item {
                    t.filename == np.track.filename
                } else {
                    false
                }
            }) {
                let len = self.displayed_items.len();
                if len == 0 {
                    return;
                }

                let prev_idx = if current_idx > 0 {
                    current_idx - 1
                } else {
                    len - 1
                };

                if let Some(DisplayItem::Track(track)) = self.displayed_items.get(prev_idx) {
                    self.play_track(track.clone());
                    self.track_state.select(Some(prev_idx));
                }
            }
        }
    }

    pub fn queue_selected(&mut self) {
        let selected_idx = if self.is_searching && !self.filtered_indices.is_empty() {
            self.track_state.selected()
                .and_then(|i| self.filtered_indices.get(i).copied())
        } else {
            self.track_state.selected()
        };

        if let Some(idx) = selected_idx {
            if let Some(DisplayItem::Track(track)) = self.displayed_items.get(idx) {
                add_track_to_queue(&track.filename);
                self.status_message = Some(format!("Added to queue: {}", track.title.as_deref().unwrap_or("Unknown")));
            }
        }
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle_enabled = !self.shuffle_enabled;
        self.status_message = Some(format!("Shuffle: {}", if self.shuffle_enabled { "On" } else { "Off" }));
    }

    pub fn cycle_repeat(&mut self) {
        self.repeat_mode = match self.repeat_mode {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
        let mode_str = match self.repeat_mode {
            RepeatMode::Off => "Off",
            RepeatMode::All => "All",
            RepeatMode::One => "One",
        };
        self.status_message = Some(format!("Repeat: {}", mode_str));
    }

    pub fn toggle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Navigation => Panel::TrackList,
            Panel::TrackList => Panel::Navigation,
        };
    }

    pub fn start_search(&mut self) {
        self.is_searching = true;
        self.search_query.clear();
        self.filtered_indices.clear();
    }

    pub fn cancel_search(&mut self) {
        self.is_searching = false;
        self.search_query.clear();
        self.filtered_indices.clear();
    }

    pub fn update_search(&mut self, c: char) {
        self.search_query.push(c);
        self.filter_tracks();
        self.track_state.select(if self.filtered_indices.is_empty() { None } else { Some(0) });
    }

    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        if self.search_query.is_empty() {
            self.filtered_indices.clear();
            self.track_state.select(if self.displayed_items.is_empty() { None } else { Some(0) });
        } else {
            self.filter_tracks();
            self.track_state.select(if self.filtered_indices.is_empty() { None } else { Some(0) });
        }
    }

    fn filter_tracks(&mut self) {
        let query = self.search_query.to_lowercase();
        self.filtered_indices = self.displayed_items.iter().enumerate()
            .filter(|(_, item)| {
                let title = item.title().to_lowercase();
                let artist = item.artist().to_lowercase();
                let album = item.album().to_lowercase();
                title.contains(&query) || artist.contains(&query) || album.contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
    }

    pub fn get_playback_position(&self) -> Option<(Duration, Duration)> {
        if let Some(ref np) = self.now_playing {
            let pos = self.audio.get_pos();
            let duration = np.duration?;
            Some((pos, duration))
        } else {
            None
        }
    }

    pub fn set_visible_height(&mut self, height: usize) {
        self.visible_height = height;
    }

    pub fn get_total_items(&self) -> usize {
        if self.is_searching && !self.filtered_indices.is_empty() {
            self.filtered_indices.len()
        } else if self.is_searching {
            0
        } else {
            self.displayed_items.len()
        }
    }

    pub fn get_visible_items(&self) -> (Vec<&DisplayItem>, usize) {
        let total = self.get_total_items();
        let selected = self.track_state.selected().unwrap_or(0);

        // Adjust scroll offset to keep selected item visible
        let scroll = if selected < self.scroll_offset {
            selected
        } else if selected >= self.scroll_offset + self.visible_height {
            selected.saturating_sub(self.visible_height - 1)
        } else {
            self.scroll_offset
        };

        let items: Vec<&DisplayItem> = if self.is_searching && !self.filtered_indices.is_empty() {
            self.filtered_indices.iter()
                .skip(scroll)
                .take(self.visible_height)
                .filter_map(|&i| self.displayed_items.get(i))
                .collect()
        } else if self.is_searching {
            Vec::new()
        } else {
            self.displayed_items.iter()
                .skip(scroll)
                .take(self.visible_height)
                .collect()
        };

        (items, scroll)
    }

    pub fn update_scroll_for_selection(&mut self) {
        let selected = self.track_state.selected().unwrap_or(0);
        if selected < self.scroll_offset {
            self.scroll_offset = selected;
        } else if selected >= self.scroll_offset + self.visible_height {
            self.scroll_offset = selected.saturating_sub(self.visible_height - 1);
        }
    }
}
