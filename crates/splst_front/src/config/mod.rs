mod controller;
mod bios;
mod disc;

use splst_core::Bios;

use controller::ControllerConfig;
use bios::BiosConfig;
use disc::DiscConfig;

use directories_next::ProjectDirs;
use serde::{Deserialize, Serialize};
use winit::event::{VirtualKeyCode, ElementState};
use thiserror::Error;

use std::io::{self, Write};
use std::path::PathBuf;
use std::fs;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to find config directory")]
    ConfigDir,

    #[error("failed to load config file: {0}")]
    Io(#[from] io::Error),

    #[error("failed to serialize config file: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("failed to deserialize config file: {0}")]
    Deserialize(#[from] toml::de::Error),
}

#[derive(PartialEq)]
enum Menu {
    Controller,
    Bios,
    Disc,
}

impl Default for Menu {
    fn default() -> Self {
        Menu::Controller
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    /// Which menu to show.
    #[serde(skip)]
    menu: Menu, 

    #[serde(skip)]
    in_config: bool,

    #[serde(skip)]
    error: Option<String>,

    pub controller: ControllerConfig,
    pub bios: BiosConfig,
    pub disc: DiscConfig,
}

impl Config {
    fn is_modified(&self) -> bool {
        self.controller.is_modified
            || self.bios.is_modified
            || self.disc.is_modified
    }

    pub fn load_from_file() -> Result<Self, ConfigError> {
        let project = ProjectDirs::from("spillestation", "", "")
            .ok_or(ConfigError::ConfigDir)?;
        let directory = project
            .config_dir()
            .to_str()
            .ok_or(ConfigError::ConfigDir)?;
        let path: PathBuf = [directory, "config.toml"]
            .iter()
            .collect();
        let mut config: Config = toml::from_str(&fs::read_to_string(&path)?)?;
        config.in_config = true;
        Ok(config)
    }

    fn save_to_file(&mut self) -> Result<(), ConfigError> {
        let project = ProjectDirs::from("spillestation", "", "")
            .ok_or(ConfigError::ConfigDir)?;
        let directory = project
            .config_dir()
            .to_str()
            .ok_or(ConfigError::ConfigDir)?;
        fs::create_dir_all(project.config_dir())?;
        let path: PathBuf = [directory, "config.toml"]
            .iter()
            .collect();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.write_all(toml::to_string_pretty(self)?.as_bytes())?;
        self.in_config = true;
        Ok(())
    }

    pub fn handle_key_closed(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        self.controller.handle_key_closed(key, state)
    }

    pub fn handle_key_open(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        match self.menu {
            Menu::Controller => self.controller.handle_key_open(key, state),
            _ => false,
        }
    }

    pub fn show_inside(&mut self, used_bios: Option<&Bios>, ui: &mut egui::Ui) {
        egui::SidePanel::left("menu_panel")
            .max_width(32.0)
            .show_inside(ui, |ui| {
                ui.selectable_value(&mut self.menu, Menu::Controller, "Controller");
                ui.selectable_value(&mut self.menu, Menu::Bios, "Bios");
                ui.selectable_value(&mut self.menu, Menu::Disc, "Disc");

                ui.separator();

                if let Some(error) = &self.error {
                    ui.label(error);
                }

                if !self.in_config || self.is_modified() {
                    if ui.button("Save").clicked() {
                        if let Err(err) = self.save_to_file() {
                            self.error = Some(err.to_string()); 
                        } else {
                            self.controller.is_modified = false;
                            self.bios.is_modified = false;
                            self.disc.is_modified = false;
                        }
                    }
                } else {
                    ui.label("Saved to Config");
                }
            });

        match self.menu {
            Menu::Controller => self.controller.show(ui),
            Menu::Bios => self.bios.show(used_bios, ui),
            Menu::Disc => self.disc.show(ui),
        }
    }

    pub fn show(&mut self, used_bios: Option<&Bios>, ctx: &egui::CtxRef) {
        egui::SidePanel::left("settings").show(ctx, |ui| {
            self.show_inside(used_bios, ui)
        });
    }
}
