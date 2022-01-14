use directories_next::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{{io, io::Write, fs}, path::PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to find config directory")]
    ConfigDir,

    #[error("Failed to load config file: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to serialize config file: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("Failed to deserialize config file: {0}")]
    Deserialize(#[from] toml::de::Error),
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    /// Path to BIOS file.
    pub bios: String,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let project = ProjectDirs::from("spillestation", "", "")
            .ok_or(ConfigError::ConfigDir)?;
        let directory = project.config_dir()
            .to_str()
            .ok_or(ConfigError::ConfigDir)?;
        let path: PathBuf = [directory, "config.toml"].iter().collect();
        Ok(toml::from_str(&std::fs::read_to_string(&path)?)?)
    }

    pub fn store(&self) -> Result<(), ConfigError> {
        let project = ProjectDirs::from("spillestation", "", "")
            .ok_or(ConfigError::ConfigDir)?;
        let directory = project.config_dir()
            .to_str()
            .ok_or(ConfigError::ConfigDir)?;
        fs::create_dir_all(project.config_dir())?;
        let path: PathBuf = [directory, "config.toml"].iter().collect();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        let string = toml::to_string_pretty(self)?;
        Ok(file.write_all(string.as_bytes())?)
    }
}