use directories_next::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{io, fmt, fs};
use std::path::PathBuf;
use std::io::Write;

pub enum ConfigError {
    DirError,
    IoError(io::Error),
    SerializeError(toml::ser::Error),
    DeserializeError(toml::de::Error),
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        ConfigError::IoError(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError::DeserializeError(err)
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(err: toml::ser::Error) -> Self {
        ConfigError::SerializeError(err)
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConfigError::DirError => {
                write!(f, "Failed to find config directory")
            },
            ConfigError::IoError(ref err) => {
                write!(f, "Failed to load config file: {}", err)
            },
            ConfigError::SerializeError(ref err) => {
                write!(f, "Failed to serialize config file: {}", err)
            },
            ConfigError::DeserializeError(ref err) => {
                write!(f, "Failed to deserialize config file: {}", err)
            },
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    /// Path to BIOS file.
    pub bios: String,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let project = ProjectDirs::from("spillestation", "", "")
            .ok_or(ConfigError::DirError)?;
        let directory = project.config_dir()
            .to_str()
            .ok_or(ConfigError::DirError)?;
        let path: PathBuf = [directory, "config.toml"].iter().collect();
        Ok(toml::from_str(&std::fs::read_to_string(&path)?)?)
    }

    pub fn store(&self) -> Result<(), ConfigError> {
        let project = ProjectDirs::from("spillestation", "", "")
            .ok_or(ConfigError::DirError)?;
        let directory = project.config_dir()
            .to_str()
            .ok_or(ConfigError::DirError)?;
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
