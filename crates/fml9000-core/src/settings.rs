use directories::ProjectDirs;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use std::io::Write;

fn default_volume() -> f64 {
  1.0
}

fn deserialize_folders<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
  D: Deserializer<'de>,
{
  #[derive(Deserialize)]
  #[serde(untagged)]
  enum FoldersOrFolder {
    Folders(Vec<String>),
    Folder(Option<String>),
  }

  match FoldersOrFolder::deserialize(deserializer)? {
    FoldersOrFolder::Folders(v) => Ok(v),
    FoldersOrFolder::Folder(Some(f)) => Ok(vec![f]),
    FoldersOrFolder::Folder(None) => Ok(Vec::new()),
  }
}

fn default_true() -> bool {
  true
}

fn default_fetch_limit() -> usize {
  100
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
  Off,
  #[default]
  All,
  One,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CoreSettings {
  #[serde(default, deserialize_with = "deserialize_folders", alias = "folder")]
  pub folders: Vec<String>,
  #[serde(default = "default_volume")]
  pub volume: f64,
  #[serde(default)]
  pub rescan_on_startup: bool,
  #[serde(default = "default_true")]
  pub youtube_audio_only: bool,
  #[serde(default = "default_fetch_limit")]
  pub youtube_fetch_limit: usize,
  #[serde(default)]
  pub shuffle_enabled: bool,
  #[serde(default)]
  pub repeat_mode: RepeatMode,
  #[serde(default)]
  pub vaporwave_enabled: bool,
}

impl Default for CoreSettings {
  fn default() -> Self {
    CoreSettings {
      folders: Vec::new(),
      volume: 1.0,
      rescan_on_startup: false,
      youtube_audio_only: true,
      youtube_fetch_limit: 100,
      shuffle_enabled: false,
      repeat_mode: RepeatMode::All,
      vaporwave_enabled: false,
    }
  }
}

impl CoreSettings {
  pub fn add_folder(&mut self, folder: String) {
    if !self.folders.contains(&folder) {
      self.folders.push(folder);
    }
  }

  pub fn remove_folder(&mut self, folder: &str) {
    self.folders.retain(|f| f != folder);
  }
}

pub fn get_project_dirs() -> Option<ProjectDirs> {
  ProjectDirs::from("com", "github", "fml9000")
}

pub fn get_config_path() -> Option<std::path::PathBuf> {
  get_project_dirs().map(|dirs| dirs.config_dir().to_path_buf())
}

pub fn read_settings<T: Default + for<'de> Deserialize<'de>>() -> T {
  let Some(proj_dirs) = get_project_dirs() else {
    eprintln!("Warning: Could not determine config directory, using defaults");
    return T::default();
  };

  let path = proj_dirs.config_dir().join("config.toml");

  match std::fs::read_to_string(&path) {
    Ok(conf) => toml::from_str(&conf).unwrap_or_else(|e| {
      eprintln!("Warning: Failed to parse config file: {e}, using defaults");
      T::default()
    }),
    Err(_) => T::default(),
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
  #[serde(default)]
  pub playing_track: Option<String>,
  #[serde(default)]
  pub playing_video_id: Option<String>,
  #[serde(default)]
  pub playback_position_secs: f64,
  #[serde(default = "default_nav_section")]
  pub nav_section: String,
  #[serde(default)]
  pub playlist_id: Option<i32>,
  #[serde(default)]
  pub channel_id: Option<i32>,
  #[serde(default)]
  pub nav_index: usize,
  #[serde(default)]
  pub track_index: usize,
  #[serde(default)]
  pub scroll_offset: usize,
  #[serde(default = "default_section_expanded")]
  pub section_expanded: [bool; 3],
  #[serde(default)]
  pub active_panel: String,
}

fn default_nav_section() -> String {
  "all_media".to_string()
}

fn default_section_expanded() -> [bool; 3] {
  [true, true, true]
}

pub fn read_state() -> AppState {
  let Some(proj_dirs) = get_project_dirs() else {
    return AppState::default();
  };

  let path = proj_dirs.config_dir().join("state.toml");

  match std::fs::read_to_string(&path) {
    Ok(contents) => toml::from_str(&contents).unwrap_or_else(|e| {
      eprintln!("Warning: Failed to parse state file: {e}, using defaults");
      AppState::default()
    }),
    Err(_) => AppState::default(),
  }
}

pub fn write_state(state: &AppState) -> Result<(), String> {
  let proj_dirs = get_project_dirs().ok_or("Could not determine config directory")?;
  let path = proj_dirs.config_dir();

  std::fs::create_dir_all(path).map_err(|e| format!("Failed to create config directory: {e}"))?;

  let toml = toml::to_string(state).map_err(|e| format!("Failed to serialize state: {e}"))?;

  let mut f = std::fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(path.join("state.toml"))
    .map_err(|e| format!("Failed to open state file: {e}"))?;

  write!(f, "{}", toml).map_err(|e| format!("Failed to write state file: {e}"))
}

pub fn write_settings<T: Serialize>(settings: &T) -> Result<(), String> {
  let proj_dirs = get_project_dirs().ok_or("Could not determine config directory")?;
  let path = proj_dirs.config_dir();

  std::fs::create_dir_all(path).map_err(|e| format!("Failed to create config directory: {e}"))?;

  let toml = toml::to_string(&settings).map_err(|e| format!("Failed to serialize settings: {e}"))?;

  let mut f = std::fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(path.join("config.toml"))
    .map_err(|e| format!("Failed to open config file: {e}"))?;

  write!(f, "{}", toml).map_err(|e| format!("Failed to write config file: {e}"))
}
