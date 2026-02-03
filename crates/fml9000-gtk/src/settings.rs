use fml9000_core::settings::CoreSettings;
pub use fml9000_core::settings::RepeatMode;
use serde_derive::{Deserialize, Serialize};

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
  // Core settings (flattened)
  #[serde(flatten)]
  pub core: CoreSettings,
  // GTK-specific settings
  #[serde(default)]
  pub row_height: RowHeight,
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
      core: CoreSettings::default(),
      row_height: RowHeight::Normal,
      window_width: 1200,
      window_height: 600,
      main_pane_position: -1,
      left_pane_position: -1,
      right_pane_position: -1,
    }
  }
}

// Delegate core settings methods
impl FmlSettings {
  pub fn add_folder(&mut self, folder: String) {
    self.core.add_folder(folder);
  }

  pub fn remove_folder(&mut self, folder: &str) {
    self.core.remove_folder(folder);
  }
}

// Re-export convenience accessors
impl std::ops::Deref for FmlSettings {
  type Target = CoreSettings;
  fn deref(&self) -> &Self::Target {
    &self.core
  }
}

impl std::ops::DerefMut for FmlSettings {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.core
  }
}


pub fn read_settings() -> FmlSettings {
  fml9000_core::settings::read_settings()
}

pub fn write_settings(settings: &FmlSettings) -> Result<(), String> {
  fml9000_core::settings::write_settings(settings)
}
