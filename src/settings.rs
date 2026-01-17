use directories::ProjectDirs;
use serde::de::{Deserialize, Deserializer};
use serde_derive::{Deserialize, Serialize};
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

fn default_window_width() -> i32 {
  1200
}

fn default_window_height() -> i32 {
  600
}

fn default_pane_position() -> i32 {
  -1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowHeight {
  #[default]
  Normal,
  Compact,
  UltraCompact,
}

impl RowHeight {
  pub fn height_pixels(&self) -> i32 {
    match self {
      RowHeight::Normal => 24,
      RowHeight::Compact => 18,
      RowHeight::UltraCompact => 8,
    }
  }

  pub fn is_compact(&self) -> bool {
    matches!(self, RowHeight::Compact | RowHeight::UltraCompact)
  }
}

#[derive(Serialize, Deserialize)]
pub struct FmlSettings {
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
  pub row_height: RowHeight,
  #[serde(default)]
  pub shuffle_enabled: bool,
  #[serde(default = "default_window_width")]
  pub window_width: i32,
  #[serde(default = "default_window_height")]
  pub window_height: i32,
  #[serde(default = "default_pane_position")]
  pub main_pane_position: i32,
  #[serde(default = "default_pane_position")]
  pub left_pane_position: i32,
  #[serde(default = "default_pane_position")]
  pub right_pane_position: i32,
}

impl Default for FmlSettings {
  fn default() -> Self {
    FmlSettings {
      folders: Vec::new(),
      volume: 1.0,
      rescan_on_startup: false,
      youtube_audio_only: true,
      youtube_fetch_limit: 100,
      row_height: RowHeight::Normal,
      shuffle_enabled: false,
      window_width: 1200,
      window_height: 600,
      main_pane_position: -1,
      left_pane_position: -1,
      right_pane_position: -1,
    }
  }
}

impl FmlSettings {
  pub fn add_folder(&mut self, folder: String) {
    if !self.folders.contains(&folder) {
      self.folders.push(folder);
    }
  }

  pub fn remove_folder(&mut self, folder: &str) {
    self.folders.retain(|f| f != folder);
  }
}

fn get_project_dirs() -> Option<ProjectDirs> {
  ProjectDirs::from("com", "github", "fml9000")
}

pub fn read_settings() -> FmlSettings {
  let Some(proj_dirs) = get_project_dirs() else {
    eprintln!("Warning: Could not determine config directory, using defaults");
    return FmlSettings::default();
  };

  let path = proj_dirs.config_dir().join("config.toml");

  match std::fs::read_to_string(&path) {
    Ok(conf) => toml::from_str(&conf).unwrap_or_else(|e| {
      eprintln!("Warning: Failed to parse config file: {e}, using defaults");
      FmlSettings::default()
    }),
    Err(_) => FmlSettings::default(),
  }
}

pub fn write_settings(settings: &FmlSettings) -> Result<(), String> {
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
