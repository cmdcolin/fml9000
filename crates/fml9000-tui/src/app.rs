use fml9000_core::{
    AudioPlayer, Track, YouTubeVideo, YouTubeChannel, Playlist, MediaItem,
    load_tracks, get_all_media, get_all_videos, get_user_playlists, get_queue_items,
    get_playlist_items, get_youtube_channels, get_videos_for_channel,
    load_recently_played_items, load_recently_added_items, queue_len, pop_queue_front,
    add_to_queue, mark_as_played, update_play_stats,
    create_playlist, add_to_playlist, delete_playlist,
    rename_playlist, load_video_by_video_id,
    add_youtube_channel, add_youtube_videos,
};
use fml9000_core::youtube::{fetch_channel_info, ChannelInfo, VideoInfo};
use fml9000_core::settings::{AppState, CoreSettings, RepeatMode, read_state, write_state};
use ratatui::widgets::TableState;
use rodio::source::Source;
use rodio::Decoder;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Navigation,
    TrackList,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NavSection {
    AllMedia,
    AllTracks,
    AllVideos,
    Playlists,
    Queue,
    RecentlyPlayed,
    RecentlyAdded,
    YouTubeChannel,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SectionId {
    AutoPlaylists,
    UserPlaylists,
    YouTubeChannels,
}

#[derive(Clone)]
pub enum NavItem {
    SectionHeader(SectionId, String),
    AutoPlaylist(usize, String),   // (fixed_index, name)
    UserPlaylist(i32, String),     // (playlist_id, name)
    YouTubeChannel(i32, String),   // (channel_id, name)
}

#[derive(Clone, PartialEq, Eq)]
pub enum UiMode {
    Normal,
    ContextMenu,
    PlaylistSelect,
    NewPlaylistInput,
    PlaylistContextMenu,
    RenamePlaylistInput,
    AddYouTubeChannel,
    Help,
}

pub enum YouTubeFetchResult {
    Success(ChannelInfo, Vec<VideoInfo>),
    Error(String),
}

pub struct ContextMenu {
    pub track_idx: usize,
    pub selected: usize,
    pub items: Vec<&'static str>,
}

pub struct PlaylistMenu {
    pub playlist_id: i32,
    pub playlist_name: String,
    pub selected: usize,
    pub items: Vec<&'static str>,
}

pub struct App {
    pub audio: AudioPlayer,
    pub audio_error: Option<String>,
    pub tracks: Vec<Arc<Track>>,
    pub playlists: Vec<Arc<Playlist>>,
    pub channels: Vec<Arc<YouTubeChannel>>,
    pub displayed_items: Vec<MediaItem>,
    pub nav_state: TableState,
    pub track_state: TableState,
    pub active_panel: Panel,
    pub current_nav: NavSection,
    pub current_playlist_id: Option<i32>,
    pub now_playing: Option<NowPlaying>,
    pub now_playing_video: Option<NowPlayingVideo>,
    pub mpv_process: Option<Child>,
    pub mpv_socket_path: Option<PathBuf>,
    pub mpv_paused: bool,
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
    // Section expand/collapse state
    pub section_expanded: [bool; 3], // AutoPlaylists, UserPlaylists, YouTubeChannels
    // Context menu state
    pub ui_mode: UiMode,
    pub context_menu: Option<ContextMenu>,
    pub playlist_select_idx: usize,
    pub new_playlist_name: String,
    pub playlist_menu: Option<PlaylistMenu>,
    pub rename_playlist_name: String,
    // Double-click tracking
    pub last_click: Option<(Instant, usize)>, // (time, track_idx)
    // YouTube channel add state
    pub youtube_channel_url: String,
    pub youtube_fetch_rx: Option<mpsc::Receiver<YouTubeFetchResult>>,
    pub youtube_fetch_tick: u8,
}

#[derive(Clone)]
pub struct NowPlaying {
    pub track: Arc<Track>,
    pub duration: Option<Duration>,
    pub started_at: std::time::Instant,
    pub play_counted: bool,
}

pub struct NowPlayingVideo {
    pub video: Arc<YouTubeVideo>,
    pub started_at: std::time::Instant,
    pub accumulated_secs: u64,  // Time played before last pause
    pub play_counted: bool,
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

        let channels = get_youtube_channels().unwrap_or_default();
        info!("Loaded {} YouTube channels", channels.len());

        let displayed_items = get_all_media();

        let mut nav_state = TableState::default();
        nav_state.select(Some(1));

        let mut track_state = TableState::default();
        if !displayed_items.is_empty() {
            track_state.select(Some(0));
        }

        info!("App::new() completed in {:?}", start.elapsed());

        let mut app = App {
            audio,
            audio_error,
            tracks,
            playlists,
            channels,
            displayed_items,
            nav_state,
            track_state,
            active_panel: Panel::TrackList,
            current_nav: NavSection::AllMedia,
            current_playlist_id: None,
            now_playing: None,
            now_playing_video: None,
            mpv_process: None,
            mpv_socket_path: None,
            mpv_paused: false,
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
            section_expanded: [true, true, true],
            ui_mode: UiMode::Normal,
            context_menu: None,
            playlist_select_idx: 0,
            new_playlist_name: String::new(),
            playlist_menu: None,
            rename_playlist_name: String::new(),
            last_click: None,
            youtube_channel_url: String::new(),
            youtube_fetch_rx: None,
            youtube_fetch_tick: 0,
        };

        app.restore_state();
        info!("State restored");

        app
    }

    pub fn on_tick(&mut self) {
        // Check if current track finished
        if let Some(ref np) = self.now_playing {
            let elapsed = np.started_at.elapsed();

            // Update play count at 50% threshold
            if !np.play_counted {
                if let Some(duration) = np.duration {
                    if elapsed >= duration / 2 {
                        update_play_stats(&MediaItem::Track(np.track.clone()));
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

        // Update YouTube play count at 50% threshold
        let youtube_elapsed = self.get_youtube_elapsed_secs();
        if let Some(ref mut npv) = self.now_playing_video {
            if !npv.play_counted && !self.mpv_paused {
                if let Some(dur) = npv.video.duration_seconds {
                    let half_duration = (dur as u64) / 2;
                    if youtube_elapsed >= half_duration {
                        update_play_stats(&MediaItem::Video(npv.video.clone()));
                        npv.play_counted = true;
                    }
                }
            }
        }

        // Check if mpv process finished (YouTube playback)
        let mpv_finished = if let Some(ref mut process) = self.mpv_process {
            match process.try_wait() {
                Ok(Some(_status)) => true,
                Ok(None) => false,
                Err(e) => {
                    self.status_message = Some(format!("mpv error: {}", e));
                    true
                }
            }
        } else {
            false
        };

        if mpv_finished {
            self.cleanup_mpv();
            self.play_next();
        }

        self.check_youtube_fetch();
    }

    const AUTO_PLAYLISTS: [&'static str; 6] = ["All Media", "All Tracks", "All Videos", "Queue", "Recently Played", "Recently Added"];

    pub fn build_nav_items(&self) -> Vec<NavItem> {
        let mut items = Vec::new();

        items.push(NavItem::SectionHeader(SectionId::AutoPlaylists, "Auto Playlists".to_string()));
        if self.section_expanded[0] {
            for (i, name) in Self::AUTO_PLAYLISTS.iter().enumerate() {
                items.push(NavItem::AutoPlaylist(i, name.to_string()));
            }
        }

        items.push(NavItem::SectionHeader(SectionId::UserPlaylists, "Playlists".to_string()));
        if self.section_expanded[1] {
            for playlist in &self.playlists {
                items.push(NavItem::UserPlaylist(playlist.id, playlist.name.clone()));
            }
        }

        items.push(NavItem::SectionHeader(SectionId::YouTubeChannels, "YouTube Channels".to_string()));
        if self.section_expanded[2] {
            for channel in &self.channels {
                items.push(NavItem::YouTubeChannel(channel.id, channel.name.clone()));
            }
        }

        items
    }

    pub fn nav_item_count(&self) -> usize {
        self.build_nav_items().len()
    }

    pub fn toggle_section(&mut self, section: SectionId) {
        let idx = match section {
            SectionId::AutoPlaylists => 0,
            SectionId::UserPlaylists => 1,
            SectionId::YouTubeChannels => 2,
        };
        self.section_expanded[idx] = !self.section_expanded[idx];
    }

    pub fn expand_or_collapse_nav(&mut self, expand: bool) {
        let selected = self.nav_state.selected().unwrap_or(0);
        let items = self.build_nav_items();
        if let Some(NavItem::SectionHeader(section, _)) = items.get(selected) {
            let idx = match section {
                SectionId::AutoPlaylists => 0,
                SectionId::UserPlaylists => 1,
                SectionId::YouTubeChannels => 2,
            };
            self.section_expanded[idx] = expand;
        }
    }

    pub fn select_nav_leaf(&mut self, leaf_index: usize) {
        let items = self.build_nav_items();
        let mut count = 0;
        for (i, item) in items.iter().enumerate() {
            if !matches!(item, NavItem::SectionHeader(_, _)) {
                if count == leaf_index {
                    self.nav_state.select(Some(i));
                    self.select_nav();
                    return;
                }
                count += 1;
            }
        }
    }

    pub fn nav_down(&mut self) {
        let len = self.nav_item_count();
        if len == 0 {
            return;
        }
        let i = self.nav_state.selected().unwrap_or(0);
        if i + 1 < len {
            self.nav_state.select(Some(i + 1));
        }
    }

    pub fn nav_up(&mut self) {
        let len = self.nav_item_count();
        if len == 0 {
            return;
        }
        let i = self.nav_state.selected().unwrap_or(0);
        if i > 0 {
            self.nav_state.select(Some(i - 1));
        }
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
        if i + 1 < len {
            self.track_state.select(Some(i + 1));
        }
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
        if i > 0 {
            self.track_state.select(Some(i - 1));
        }
    }

    pub fn select_nav(&mut self) {
        let selected = self.nav_state.selected().unwrap_or(0);
        let items = self.build_nav_items();
        let Some(item) = items.get(selected) else {
            return;
        };

        match item {
            NavItem::SectionHeader(section, _) => {
                self.toggle_section(*section);
                return;
            }
            NavItem::AutoPlaylist(idx, _) => {
                match idx {
                    0 => self.load_all_media(),
                    1 => self.load_all_tracks(),
                    2 => self.load_all_videos(),
                    3 => self.load_queue(),
                    4 => self.load_recently_played(),
                    5 => self.load_recently_added(),
                    _ => {}
                }
            }
            NavItem::UserPlaylist(id, _) => {
                self.load_playlist(*id);
            }
            NavItem::YouTubeChannel(id, _) => {
                self.load_youtube_channel(*id);
            }
        }
        self.track_state.select(if self.displayed_items.is_empty() { None } else { Some(0) });
        self.scroll_offset = 0;
    }

    fn load_all_media(&mut self) {
        self.current_nav = NavSection::AllMedia;
        self.current_playlist_id = None;
        self.displayed_items = get_all_media();
    }

    fn load_all_tracks(&mut self) {
        self.current_nav = NavSection::AllTracks;
        self.current_playlist_id = None;
        self.displayed_items = self.tracks.iter()
            .map(|t| MediaItem::Track(t.clone()))
            .collect();
    }

    fn load_all_videos(&mut self) {
        self.current_nav = NavSection::AllVideos;
        self.current_playlist_id = None;
        self.displayed_items = get_all_videos()
            .unwrap_or_default()
            .into_iter()
            .map(MediaItem::Video)
            .collect();
    }

    fn load_playlist(&mut self, playlist_id: i32) {
        self.current_nav = NavSection::Playlists;
        self.current_playlist_id = Some(playlist_id);
        match get_playlist_items(playlist_id) {
            Ok(items) => {
                self.displayed_items = items;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to load playlist: {}", e));
                self.displayed_items.clear();
            }
        }
    }

    fn load_queue(&mut self) {
        self.current_nav = NavSection::Queue;
        self.current_playlist_id = None;
        self.displayed_items = get_queue_items();
    }

    fn load_recently_played(&mut self) {
        self.current_nav = NavSection::RecentlyPlayed;
        self.current_playlist_id = None;
        self.displayed_items = load_recently_played_items(100);
    }

    fn load_recently_added(&mut self) {
        self.current_nav = NavSection::RecentlyAdded;
        self.current_playlist_id = None;
        self.displayed_items = load_recently_added_items(100);
    }

    fn load_youtube_channel(&mut self, channel_id: i32) {
        self.current_nav = NavSection::YouTubeChannel;
        self.current_playlist_id = None;
        match get_videos_for_channel(channel_id) {
            Ok(videos) => {
                self.displayed_items = videos.into_iter().map(|v| MediaItem::Video(v)).collect();
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to load channel: {}", e));
                self.displayed_items.clear();
            }
        }
    }

    pub fn play_selected(&mut self) {
        let selected_idx = if self.is_searching && !self.filtered_indices.is_empty() {
            self.track_state.selected()
                .and_then(|i| self.filtered_indices.get(i).copied())
        } else {
            self.track_state.selected()
        };

        if let Some(idx) = selected_idx {
            if let Some(item) = self.displayed_items.get(idx).cloned() {
                self.play_item(&item);
            }
        }
    }

    fn play_track(&mut self, track: Arc<Track>) {
        self.cleanup_mpv();

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
        mark_as_played(&MediaItem::Track(track.clone()));

        self.now_playing = Some(NowPlaying {
            track,
            duration,
            started_at: std::time::Instant::now(),
            play_counted: false,
        });

        self.status_message = None;
    }

    fn play_youtube(&mut self, video: Arc<YouTubeVideo>) {
        // Stop any currently playing content
        self.audio.stop();
        self.now_playing = None;
        self.cleanup_mpv();

        let url = format!("https://www.youtube.com/watch?v={}", video.video_id);

        // Create a unique socket path for IPC
        let socket_path = PathBuf::from(format!("/tmp/fml9000-mpv-{}.sock", std::process::id()));

        // Spawn mpv in audio-only mode with IPC socket for control
        match Command::new("mpv")
            .arg("--no-video")
            .arg("--really-quiet")
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                self.mpv_process = Some(child);
                self.mpv_socket_path = Some(socket_path);
                self.mpv_paused = false;
                self.now_playing_video = Some(NowPlayingVideo {
                    video,
                    started_at: std::time::Instant::now(),
                    accumulated_secs: 0,
                    play_counted: false,
                });
                self.status_message = None;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to start mpv: {}. Is mpv installed?", e));
            }
        }
    }

    fn play_in_external_mpv(&mut self, item: MediaItem) {
        let url = match &item {
            MediaItem::Track(track) => track.filename.clone(),
            MediaItem::Video(video) => format!("https://www.youtube.com/watch?v={}", video.video_id),
        };

        match Command::new("mpv")
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => {
                self.status_message = Some(format!("Opened in mpv: {}", item.title()));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to start mpv: {}. Is mpv installed?", e));
            }
        }
    }

    fn cleanup_mpv(&mut self) {
        if let Some(mut process) = self.mpv_process.take() {
            let _ = process.kill();
        }
        if let Some(ref path) = self.mpv_socket_path.take() {
            let _ = std::fs::remove_file(path);
        }
        self.mpv_paused = false;
        self.now_playing_video = None;
    }

    fn send_mpv_command(&self, command: &str) -> bool {
        if let Some(ref socket_path) = self.mpv_socket_path {
            if let Ok(mut stream) = UnixStream::connect(socket_path) {
                let cmd = format!("{{ \"command\": {} }}\n", command);
                return stream.write_all(cmd.as_bytes()).is_ok();
            }
        }
        false
    }

    pub fn toggle_pause(&mut self) {
        // Handle mpv (YouTube) playback
        if self.mpv_process.is_some() {
            if self.send_mpv_command("[\"cycle\", \"pause\"]") {
                self.mpv_paused = !self.mpv_paused;
                // Track time for progress bar
                if let Some(ref mut npv) = self.now_playing_video {
                    if self.mpv_paused {
                        // Pausing: save accumulated time
                        npv.accumulated_secs += npv.started_at.elapsed().as_secs();
                    } else {
                        // Resuming: reset timer
                        npv.started_at = std::time::Instant::now();
                    }
                }
            }
            return;
        }

        // Handle rodio (local file) playback
        if self.audio.is_playing() {
            self.audio.pause();
        } else {
            self.audio.play();
        }
    }

    pub fn get_youtube_elapsed_secs(&self) -> u64 {
        if let Some(ref npv) = self.now_playing_video {
            if self.mpv_paused {
                npv.accumulated_secs
            } else {
                npv.accumulated_secs + npv.started_at.elapsed().as_secs()
            }
        } else {
            0
        }
    }

    pub fn stop(&mut self) {
        self.audio.stop();
        self.now_playing = None;
        self.cleanup_mpv();
    }

    fn play_item(&mut self, item: &MediaItem) {
        match item {
            MediaItem::Track(track) => self.play_track(track.clone()),
            MediaItem::Video(video) => self.play_youtube(video.clone()),
        }
    }

    fn current_playing_index(&self) -> Option<usize> {
        if let Some(ref npv) = self.now_playing_video {
            self.displayed_items.iter().position(|item| {
                item.youtube_video_id() == Some(&npv.video.video_id)
            })
        } else if let Some(ref np) = self.now_playing {
            self.displayed_items.iter().position(|item| {
                item.track_filename() == Some(&np.track.filename)
            })
        } else {
            None
        }
    }

    pub fn play_next(&mut self) {
        // Check queue first
        if queue_len() > 0 {
            if let Some(item) = pop_queue_front() {
                self.play_item(&item);
                return;
            }
        }

        if self.repeat_mode == RepeatMode::One {
            if let Some(ref np) = self.now_playing {
                self.play_track(np.track.clone());
                return;
            }
            if let Some(ref npv) = self.now_playing_video {
                self.play_youtube(npv.video.clone());
                return;
            }
        }

        if let Some(current_idx) = self.current_playing_index() {
            let len = self.displayed_items.len();
            if len == 0 {
                return;
            }

            let next_idx = if self.shuffle_enabled {
                use rand::RngExt;
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

            if let Some(item) = self.displayed_items.get(next_idx).cloned() {
                self.play_item(&item);
                self.track_state.select(Some(next_idx));
            }
        }
    }

    pub fn play_prev(&mut self) {
        if let Some(current_idx) = self.current_playing_index() {
            let len = self.displayed_items.len();
            if len == 0 {
                return;
            }

            let prev_idx = if current_idx > 0 {
                current_idx - 1
            } else {
                len - 1
            };

            if let Some(item) = self.displayed_items.get(prev_idx).cloned() {
                self.play_item(&item);
                self.track_state.select(Some(prev_idx));
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
            if let Some(item) = self.displayed_items.get(idx) {
                add_to_queue(item);
                self.status_message = Some(format!("Added to queue: {}", item.title()));
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

    pub fn seek_ratio(&mut self, ratio: f64) {
        let ratio = ratio.clamp(0.0, 1.0);

        if self.mpv_process.is_some() {
            if let Some(ref npv) = self.now_playing_video {
                if let Some(dur) = npv.video.duration_seconds {
                    let target_secs = (dur as f64 * ratio) as u64;
                    let cmd = format!("[\"seek\", {}, \"absolute\"]", target_secs);
                    if self.send_mpv_command(&cmd) {
                        if let Some(ref mut npv) = self.now_playing_video {
                            npv.accumulated_secs = target_secs;
                            npv.started_at = std::time::Instant::now();
                        }
                    }
                }
            }
        } else if let Some(ref np) = self.now_playing {
            if let Some(duration) = np.duration {
                let target = Duration::from_secs_f64(duration.as_secs_f64() * ratio);
                self.audio.try_seek(target);
            }
        }
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

    pub fn get_visible_items(&self) -> (Vec<&MediaItem>, usize) {
        let _total = self.get_total_items();
        let selected = self.track_state.selected().unwrap_or(0);

        // Adjust scroll offset to keep selected item visible
        let scroll = if selected < self.scroll_offset {
            selected
        } else if selected >= self.scroll_offset + self.visible_height {
            selected.saturating_sub(self.visible_height - 1)
        } else {
            self.scroll_offset
        };

        let items: Vec<&MediaItem> = if self.is_searching && !self.filtered_indices.is_empty() {
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

    // Context menu methods
    pub fn open_context_menu(&mut self, track_idx: usize) {
        self.context_menu = Some(ContextMenu {
            track_idx,
            selected: 0,
            items: {
                let mut items = vec!["Add to queue", "Add to playlist", "Create new playlist", "Play in mpv"];
                if let Some(MediaItem::Video(_)) = self.displayed_items.get(track_idx) {
                    items.push("Open in browser");
                }
                items
            },
        });
        self.ui_mode = UiMode::ContextMenu;
    }

    pub fn close_context_menu(&mut self) {
        self.context_menu = None;
        self.ui_mode = UiMode::Normal;
        self.playlist_select_idx = 0;
        self.new_playlist_name.clear();
    }

    pub fn context_menu_up(&mut self) {
        if let Some(ref mut menu) = self.context_menu {
            if menu.selected > 0 {
                menu.selected -= 1;
            }
        }
    }

    pub fn context_menu_down(&mut self) {
        if let Some(ref mut menu) = self.context_menu {
            if menu.selected + 1 < menu.items.len() {
                menu.selected += 1;
            }
        }
    }

    pub fn context_menu_select(&mut self) {
        let (track_idx, selected) = if let Some(ref menu) = self.context_menu {
            (menu.track_idx, menu.selected)
        } else {
            return;
        };

        match selected {
            0 => {
                // Add to queue
                if let Some(item) = self.displayed_items.get(track_idx) {
                    add_to_queue(item);
                    self.status_message = Some(format!("Added to queue: {}", item.title()));
                }
                self.close_context_menu();
            }
            1 => {
                // Add to playlist - show playlist selector
                self.playlists = get_user_playlists().unwrap_or_default();
                self.playlist_select_idx = 0;
                self.ui_mode = UiMode::PlaylistSelect;
            }
            2 => {
                // Create new playlist
                self.new_playlist_name.clear();
                self.ui_mode = UiMode::NewPlaylistInput;
            }
            3 => {
                // Play in external mpv
                if let Some(item) = self.displayed_items.get(track_idx) {
                    self.play_in_external_mpv(item.clone());
                }
                self.close_context_menu();
            }
            4 => {
                // Open in browser (YouTube only)
                if let Some(MediaItem::Video(video)) = self.displayed_items.get(track_idx) {
                    let url = format!("https://www.youtube.com/watch?v={}", video.video_id);
                    match open::that(&url) {
                        Ok(()) => {
                            self.status_message = Some(format!("Opened in browser: {}", video.title));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Failed to open browser: {}", e));
                        }
                    }
                }
                self.close_context_menu();
            }
            _ => {}
        }
    }

    pub fn playlist_select_up(&mut self) {
        if self.playlist_select_idx > 0 {
            self.playlist_select_idx -= 1;
        }
    }

    pub fn playlist_select_down(&mut self) {
        if self.playlist_select_idx + 1 < self.playlists.len() {
            self.playlist_select_idx += 1;
        }
    }

    pub fn playlist_select_confirm(&mut self) {
        let track_idx = if let Some(ref menu) = self.context_menu {
            menu.track_idx
        } else {
            self.close_context_menu();
            return;
        };

        if let Some(playlist) = self.playlists.get(self.playlist_select_idx) {
            if let Some(item) = self.displayed_items.get(track_idx) {
                match add_to_playlist(playlist.id, item) {
                    Ok(()) => {
                        self.status_message = Some(format!(
                            "Added '{}' to '{}'",
                            item.title(),
                            playlist.name
                        ));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Error: {}", e));
                    }
                }
            }
        }
        self.close_context_menu();
    }

    pub fn new_playlist_input(&mut self, c: char) {
        self.new_playlist_name.push(c);
    }

    pub fn new_playlist_backspace(&mut self) {
        self.new_playlist_name.pop();
    }

    pub fn new_playlist_confirm(&mut self) {
        if self.new_playlist_name.is_empty() {
            self.status_message = Some("Playlist name cannot be empty".to_string());
            return;
        }

        let track_idx = if let Some(ref menu) = self.context_menu {
            menu.track_idx
        } else {
            self.close_context_menu();
            return;
        };

        match create_playlist(&self.new_playlist_name) {
            Ok(playlist_id) => {
                if let Some(item) = self.displayed_items.get(track_idx) {
                    match add_to_playlist(playlist_id, item) {
                        Ok(()) => {
                            self.status_message = Some(format!(
                                "Created '{}' and added track",
                                self.new_playlist_name
                            ));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Created playlist but failed to add track: {}", e));
                        }
                    }
                }
                // Refresh playlists
                self.playlists = get_user_playlists().unwrap_or_default();
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to create playlist: {}", e));
            }
        }
        self.close_context_menu();
    }

    pub fn get_selected_track_idx(&self) -> Option<usize> {
        if self.is_searching && !self.filtered_indices.is_empty() {
            self.track_state.selected()
                .and_then(|i| self.filtered_indices.get(i).copied())
        } else {
            self.track_state.selected()
        }
    }

    // Playlist context menu methods
    pub fn open_playlist_menu(&mut self, playlist_id: i32, playlist_name: String) {
        self.playlist_menu = Some(PlaylistMenu {
            playlist_id,
            playlist_name: playlist_name.clone(),
            selected: 0,
            items: vec!["Rename playlist", "Delete playlist"],
        });
        self.rename_playlist_name = playlist_name;
        self.ui_mode = UiMode::PlaylistContextMenu;
    }

    pub fn close_playlist_menu(&mut self) {
        self.playlist_menu = None;
        self.ui_mode = UiMode::Normal;
        self.rename_playlist_name.clear();
    }

    pub fn playlist_menu_up(&mut self) {
        if let Some(ref mut menu) = self.playlist_menu {
            if menu.selected > 0 {
                menu.selected -= 1;
            }
        }
    }

    pub fn playlist_menu_down(&mut self) {
        if let Some(ref mut menu) = self.playlist_menu {
            if menu.selected + 1 < menu.items.len() {
                menu.selected += 1;
            }
        }
    }

    pub fn playlist_menu_select(&mut self) {
        let (playlist_id, selected) = if let Some(ref menu) = self.playlist_menu {
            (menu.playlist_id, menu.selected)
        } else {
            return;
        };

        match selected {
            0 => {
                // Rename playlist - show input dialog
                self.ui_mode = UiMode::RenamePlaylistInput;
            }
            1 => {
                // Delete playlist
                match delete_playlist(playlist_id) {
                    Ok(()) => {
                        self.status_message = Some("Playlist deleted".to_string());
                        self.playlists = get_user_playlists().unwrap_or_default();
                        // Reset nav selection if needed
                        let nav_count = self.nav_item_count();
                        if let Some(sel) = self.nav_state.selected() {
                            if sel >= nav_count {
                                self.nav_state.select(Some(0));
                                self.load_all_media();
                            }
                        }
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to delete: {}", e));
                    }
                }
                self.close_playlist_menu();
            }
            _ => {}
        }
    }

    pub fn rename_playlist_input(&mut self, c: char) {
        self.rename_playlist_name.push(c);
    }

    pub fn rename_playlist_backspace(&mut self) {
        self.rename_playlist_name.pop();
    }

    pub fn rename_playlist_confirm(&mut self) {
        if self.rename_playlist_name.is_empty() {
            self.status_message = Some("Name cannot be empty".to_string());
            return;
        }

        let playlist_id = if let Some(ref menu) = self.playlist_menu {
            menu.playlist_id
        } else {
            self.close_playlist_menu();
            return;
        };

        match rename_playlist(playlist_id, &self.rename_playlist_name) {
            Ok(()) => {
                self.status_message = Some(format!("Renamed to '{}'", self.rename_playlist_name));
                self.playlists = get_user_playlists().unwrap_or_default();
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to rename: {}", e));
            }
        }
        self.close_playlist_menu();
    }

    pub fn save_state(&self) {
        let nav_section = match self.current_nav {
            NavSection::AllMedia => "all_media",
            NavSection::AllTracks => "all_tracks",
            NavSection::AllVideos => "all_videos",
            NavSection::Playlists => "playlists",
            NavSection::Queue => "queue",
            NavSection::RecentlyPlayed => "recently_played",
            NavSection::RecentlyAdded => "recently_added",
            NavSection::YouTubeChannel => "youtube_channel",
        };

        let playing_track = self.now_playing.as_ref().map(|np| np.track.filename.clone());
        let playing_video_id = self.now_playing_video.as_ref().map(|npv| npv.video.video_id.clone());

        let playback_position_secs = if self.now_playing.is_some() {
            self.audio.get_pos().as_secs_f64()
        } else if self.now_playing_video.is_some() {
            self.get_youtube_elapsed_secs() as f64
        } else {
            0.0
        };

        let channel_id = if self.current_nav == NavSection::YouTubeChannel {
            // Find the channel id from the nav selection
            let items = self.build_nav_items();
            if let Some(sel) = self.nav_state.selected() {
                if let Some(NavItem::YouTubeChannel(id, _)) = items.get(sel) {
                    Some(*id)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let active_panel = match self.active_panel {
            Panel::Navigation => "navigation",
            Panel::TrackList => "track_list",
        };

        let state = AppState {
            playing_track,
            playing_video_id,
            playback_position_secs,
            nav_section: nav_section.to_string(),
            playlist_id: self.current_playlist_id,
            channel_id,
            nav_index: self.nav_state.selected().unwrap_or(0),
            track_index: self.track_state.selected().unwrap_or(0),
            scroll_offset: self.scroll_offset,
            section_expanded: self.section_expanded,
            active_panel: active_panel.to_string(),
        };

        if let Err(e) = write_state(&state) {
            eprintln!("Failed to save state: {}", e);
        }
    }

    pub fn restore_state(&mut self) {
        let state = read_state();

        self.section_expanded = state.section_expanded;

        self.active_panel = match state.active_panel.as_str() {
            "navigation" => Panel::Navigation,
            _ => Panel::TrackList,
        };

        match state.nav_section.as_str() {
            "all_media" => self.load_all_media(),
            "all_tracks" => self.load_all_tracks(),
            "all_videos" => self.load_all_videos(),
            "playlists" => {
                if let Some(id) = state.playlist_id {
                    self.load_playlist(id);
                }
            }
            "queue" => self.load_queue(),
            "recently_played" => self.load_recently_played(),
            "recently_added" => self.load_recently_added(),
            "youtube_channel" => {
                if let Some(id) = state.channel_id {
                    self.load_youtube_channel(id);
                }
            }
            _ => {}
        }

        self.nav_state.select(Some(state.nav_index));
        if !self.displayed_items.is_empty() {
            self.track_state.select(Some(state.track_index.min(self.displayed_items.len().saturating_sub(1))));
        }
        self.scroll_offset = state.scroll_offset;

        if let Some(ref filename) = state.playing_track {
            if let Some(track) = self.tracks.iter().find(|t| t.filename == *filename).cloned() {
                self.play_track(track);
                if state.playback_position_secs > 0.0 {
                    let target = Duration::from_secs_f64(state.playback_position_secs);
                    self.audio.try_seek(target);
                }
            }
        } else if let Some(ref video_id) = state.playing_video_id {
            if let Some(video) = load_video_by_video_id(video_id) {
                self.play_youtube(video);
            }
        }
    }

    // Help screen
    pub fn show_help(&mut self) {
        self.ui_mode = UiMode::Help;
    }

    pub fn close_help(&mut self) {
        self.ui_mode = UiMode::Normal;
    }

    pub fn is_youtube_fetching(&self) -> bool {
        self.youtube_fetch_rx.is_some()
    }

    // YouTube channel add methods
    pub fn open_add_youtube(&mut self) {
        if self.is_youtube_fetching() {
            self.status_message = Some("Already fetching a channel...".to_string());
            return;
        }
        self.youtube_channel_url.clear();
        self.ui_mode = UiMode::AddYouTubeChannel;
    }

    pub fn close_add_youtube(&mut self) {
        self.youtube_channel_url.clear();
        self.youtube_fetch_rx = None;
        self.ui_mode = UiMode::Normal;
    }

    pub fn youtube_input(&mut self, c: char) {
        self.youtube_channel_url.push(c);
    }

    pub fn youtube_backspace(&mut self) {
        self.youtube_channel_url.pop();
    }

    pub fn youtube_submit(&mut self) {
        let url = self.youtube_channel_url.trim().to_string();
        if url.is_empty() {
            self.status_message = Some("URL cannot be empty".to_string());
            return;
        }

        self.status_message = Some("Fetching channel info".to_string());
        self.ui_mode = UiMode::Normal;
        self.youtube_fetch_tick = 0;

        let (tx, rx) = mpsc::channel();
        self.youtube_fetch_rx = Some(rx);

        std::thread::spawn(move || {
            let result = fetch_channel_info(&url, |_| {});
            let msg = match result {
                Ok((channel, videos)) => YouTubeFetchResult::Success(channel, videos),
                Err(e) => YouTubeFetchResult::Error(e),
            };
            let _ = tx.send(msg);
        });
    }

    pub fn check_youtube_fetch(&mut self) {
        let result = if let Some(ref rx) = self.youtube_fetch_rx {
            match rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Empty) => {
                    let dots = ".".repeat((self.youtube_fetch_tick % 4) as usize);
                    self.status_message = Some(format!("Fetching channel info{}", dots));
                    self.youtube_fetch_tick = self.youtube_fetch_tick.wrapping_add(1);
                    None
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.youtube_fetch_rx = None;
                    self.status_message = Some("Fetch failed: connection lost".to_string());
                    None
                }
            }
        } else {
            None
        };

        if let Some(result) = result {
            self.youtube_fetch_rx = None;
            match result {
                YouTubeFetchResult::Success(channel, videos) => {
                    match add_youtube_channel(
                        &channel.channel_id,
                        &channel.name,
                        channel.handle.as_deref(),
                        &channel.url,
                        channel.thumbnail_url.as_deref(),
                    ) {
                        Ok(db_channel_id) => {
                            let video_tuples: Vec<_> = videos
                                .iter()
                                .map(|v| {
                                    (
                                        v.video_id.clone(),
                                        v.title.clone(),
                                        v.duration_seconds,
                                        v.thumbnail_url.clone(),
                                        v.published_at,
                                    )
                                })
                                .collect();
                            let _ = add_youtube_videos(db_channel_id, &video_tuples);
                            self.channels = get_youtube_channels().unwrap_or_default();
                            self.status_message = Some(format!(
                                "Added channel '{}' with {} videos",
                                channel.name, videos.len()
                            ));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Failed to add channel: {}", e));
                        }
                    }
                }
                YouTubeFetchResult::Error(e) => {
                    self.status_message = Some(format!("Failed to fetch channel: {}", e));
                }
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Clean up mpv process and socket when app exits
        self.cleanup_mpv();
    }
}
