use directories_next::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    pub bios: String,
}

impl Config {
    pub fn load() -> Option<Self> {
        let project = ProjectDirs::from("rs", "", "spillestation")?;
        let directory = project.config_dir().to_str()?;
        let path: PathBuf = [directory, "config.toml"].iter().collect();
        match std::fs::read_to_string(&path) {
            Ok(ref string) => Some(match toml::from_str(&string) {
                Ok(config) => config,
                Err(ref err) => panic!("Failed to parse config file: {}", err),
            }),
            Err(ref err) if err.kind() == ErrorKind::NotFound => None,
            Err(ref err) => {
                panic!("{}", err)
            }
        }
    }
}
