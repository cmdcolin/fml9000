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

pub fn read_settings() -> FmlSettings {
  let proj_dirs = ProjectDirs::from("com", "github", "fml9000").unwrap();
  let path = proj_dirs.config_dir().join("config.toml");

  match std::fs::read_to_string(&path) {
    Ok(conf) => {
      let config: FmlSettings = toml::from_str(&conf).unwrap();
      config
    }
    Err(_) => FmlSettings {
      folder: None,
      volume: 1.0,
    },
  }
}

pub fn write_settings(settings: &FmlSettings) -> std::io::Result<()> {
  let proj_dirs = ProjectDirs::from("com", "github", "fml9000").unwrap();
  let path = proj_dirs.config_dir();

  std::fs::create_dir_all(path)?;

  let toml = toml::to_string(&settings).unwrap();
  let mut f = std::fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(path.join("config.toml"))?;
  write!(f, "{}", toml)
}
