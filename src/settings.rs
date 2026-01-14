use directories::ProjectDirs;
use serde_derive::{Deserialize, Serialize};
use std::io::Write;

fn default_volume() -> f64 {
  1.0
}

#[derive(Serialize, Deserialize)]
pub struct FmlSettings {
  pub folder: Option<String>,
  #[serde(default = "default_volume")]
  pub volume: f64,
}

impl Default for FmlSettings {
  fn default() -> Self {
    FmlSettings {
      folder: None,
      volume: 1.0,
    }
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
