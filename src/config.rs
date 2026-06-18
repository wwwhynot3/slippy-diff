use std::{
    fs,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_WIDTH: i32 = 1120;
pub const DEFAULT_HEIGHT: i32 = 760;
pub const MIN_WIDTH: i32 = 720;
pub const MIN_HEIGHT: i32 = 520;
pub const DEFAULT_VERTICAL_SPLIT: f32 = 0.45;
pub const MIN_VERTICAL_SPLIT: f32 = 0.30;
pub const MAX_VERTICAL_SPLIT: f32 = 0.70;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: u32,
    pub width: i32,
    pub height: i32,
    pub vertical_split: f32,
    pub theme: Theme,
    pub ui_font: String,
    pub mono_font: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLoadStatus {
    Loaded,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigLoadResult {
    pub config: AppConfig,
    pub status: ConfigLoadStatus,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config directory unavailable")]
    ConfigDirectoryUnavailable,
    #[error("could not create config directory: {0}")]
    CreateDirectory(#[source] std::io::Error),
    #[error("could not serialize config: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("could not write config: {0}")]
    Write(#[source] std::io::Error),
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            vertical_split: DEFAULT_VERTICAL_SPLIT,
            theme: Theme::System,
            ui_font: String::new(),
            mono_font: String::new(),
        }
    }
}

impl AppConfig {
    pub fn normalized(mut self) -> Self {
        self.width = self.width.max(MIN_WIDTH);
        self.height = self.height.max(MIN_HEIGHT);
        self.vertical_split = self
            .vertical_split
            .clamp(MIN_VERTICAL_SPLIT, MAX_VERTICAL_SPLIT);
        self
    }
}

pub fn load_config_from_path(path: impl AsRef<Path>) -> ConfigLoadResult {
    let path = path.as_ref();

    let Ok(contents) = fs::read_to_string(path) else {
        return ConfigLoadResult {
            config: AppConfig::default(),
            status: ConfigLoadStatus::Missing,
        };
    };

    match serde_json::from_str::<AppConfig>(&contents) {
        Ok(config) => ConfigLoadResult {
            config: config.normalized(),
            status: ConfigLoadStatus::Loaded,
        },
        Err(_) => ConfigLoadResult {
            config: AppConfig::default(),
            status: ConfigLoadStatus::Invalid,
        },
    }
}

pub fn save_config_to_path(path: impl AsRef<Path>, config: &AppConfig) -> Result<(), ConfigError> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ConfigError::CreateDirectory)?;
    }

    let contents = serde_json::to_string_pretty(&config.clone().normalized())
        .map_err(ConfigError::Serialize)?;
    fs::write(path, contents).map_err(ConfigError::Write)
}

pub fn config_path() -> Result<PathBuf, ConfigError> {
    let dirs = ProjectDirs::from("dev", "wwwhynot3", "Slippy")
        .ok_or(ConfigError::ConfigDirectoryUnavailable)?;

    Ok(dirs.config_dir().join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_v1_contract() {
        let config = AppConfig::default();

        assert_eq!(config.version, 1);
        assert_eq!(config.width, DEFAULT_WIDTH);
        assert_eq!(config.height, DEFAULT_HEIGHT);
        assert_eq!(config.vertical_split, DEFAULT_VERTICAL_SPLIT);
        assert_eq!(config.theme, Theme::System);
        assert!(config.ui_font.is_empty());
        assert!(config.mono_font.is_empty());
    }

    #[test]
    fn normalization_clamps_window_and_split_values() {
        let config = AppConfig {
            width: 100,
            height: 100,
            vertical_split: 0.99,
            ..AppConfig::default()
        }
        .normalized();

        assert_eq!(config.width, MIN_WIDTH);
        assert_eq!(config.height, MIN_HEIGHT);
        assert_eq!(config.vertical_split, MAX_VERTICAL_SPLIT);
    }

    #[test]
    fn missing_config_returns_defaults_with_missing_status() {
        let temp = tempfile::tempdir().expect("tempdir");
        let result = load_config_from_path(temp.path().join("missing.json"));

        assert_eq!(result.config, AppConfig::default());
        assert_eq!(result.status, ConfigLoadStatus::Missing);
    }

    #[test]
    fn invalid_config_returns_defaults_with_invalid_status() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("config.json");
        fs::write(&path, "{invalid json").expect("write invalid config");

        let result = load_config_from_path(path);

        assert_eq!(result.config, AppConfig::default());
        assert_eq!(result.status, ConfigLoadStatus::Invalid);
    }

    #[test]
    fn valid_config_loads_and_normalizes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("config.json");
        fs::write(
            &path,
            r#"{
                "version": 1,
                "width": 640,
                "height": 480,
                "vertical_split": 0.2,
                "theme": "Dark",
                "ui_font": "UI Font",
                "mono_font": "Mono Font"
            }"#,
        )
        .expect("write config");

        let result = load_config_from_path(path);

        assert_eq!(result.status, ConfigLoadStatus::Loaded);
        assert_eq!(result.config.width, MIN_WIDTH);
        assert_eq!(result.config.height, MIN_HEIGHT);
        assert_eq!(result.config.vertical_split, MIN_VERTICAL_SPLIT);
        assert_eq!(result.config.theme, Theme::Dark);
        assert_eq!(result.config.ui_font, "UI Font");
        assert_eq!(result.config.mono_font, "Mono Font");
    }

    #[test]
    fn save_config_writes_normalized_json() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("nested").join("config.json");
        let config = AppConfig {
            width: 1,
            height: 2,
            vertical_split: 1.0,
            theme: Theme::Light,
            ..AppConfig::default()
        };

        save_config_to_path(&path, &config).expect("save config");
        let result = load_config_from_path(path);

        assert_eq!(result.status, ConfigLoadStatus::Loaded);
        assert_eq!(result.config.width, MIN_WIDTH);
        assert_eq!(result.config.height, MIN_HEIGHT);
        assert_eq!(result.config.vertical_split, MAX_VERTICAL_SPLIT);
        assert_eq!(result.config.theme, Theme::Light);
    }
}
