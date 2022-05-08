mod controller;
mod bios;
mod disc;

use splst_core::{Bios, Button, IoSlot, Controllers, Disc};

use controller::ControllerConfig;
use bios::BiosConfig;
use disc::DiscConfig;

use directories_next::ProjectDirs;
use serde::{Deserialize, Serialize};
use winit::event::{VirtualKeyCode, ElementState};

use std::io::Write;
use std::path::Path;
use std::collections::HashMap;
use std::fs;

/// Configuration for the emulator. This holds all the settings for the emulator like controller
/// key bindings, the disc loaded and the BIOS used. It's can be serialized and deserialized to
/// allow for saving the settings the a config file. It can also be rendered as GUI.
#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    /// Which menu to show.
    #[serde(skip)]
    menu: Menu, 
    /// If the config is saved to a config file. It dosn't represent if the config has been
    /// modified compared to the version the config file.
    #[serde(skip)]
    saved_to_file: bool,

    #[serde(skip)]
    message: Option<String>,

    pub controller: ControllerConfig,
    pub bios: BiosConfig,
    pub disc: DiscConfig,
}

impl Config {
    /// If the config is modified compared to either default, loaded from a config file or saved to
    /// a config file.
    fn is_modified(&self) -> bool {
        self.controller.is_modified
            || self.bios.is_modified
            || self.disc.is_modified
    }

    /// Show the BIOS menu. Used when trying to start the emulator without a loaded BIOS.
    pub fn show_bios_menu(&mut self) {
        self.menu = Menu::Bios;
    }

    /// The default configs, but showing a message.
    fn default_with_message(msg: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.message = Some(msg.into());
        config
    }

    /// Try load config from a config file at the default location. If loading or deserializing the
    /// config file fails, the default configs will be returned.
    pub fn from_file_or_default() -> Self {
        let Some(dir) = ProjectDirs::from("spillestation", "", "") else {
            return Self::default_with_message("failed to find config directory");
        };
        let path = dir
            .config_dir()
            .to_path_buf()
            .join(&Path::new("config.toml"));
        match fs::read_to_string(&path) {
            Err(err) => return Self::default_with_message(
                format!("failed open config file: {err}")
            ),
            Ok(toml) => match toml::from_str::<Config>(&toml) {
                Ok(mut config) => {
                    config.saved_to_file = true;
                    config
                }
                Err(err) => return Self::default_with_message(
                    format!("failed load config file: {err}")
                ),
            }
        }
    }

    /// Try to save the configs to a config file.
    fn try_save_to_file(&mut self) {
        let Some(dir) = ProjectDirs::from("spillestation", "", "") else {
            self.message = Some("failed to find config directory".to_string());
            return;
        };
        let dir = dir.config_dir();
        if let Err(err) = fs::create_dir_all(dir) {
            self.message = Some(format!(
                "failed to create config directory: {err}"
            ));
            return;
        }
        let path = dir.to_path_buf().join(&Path::new("config.toml"));
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path);
        match file {
            Ok(mut file) => {
                let source = match toml::to_string_pretty(self) {
                    Ok(source) => source,
                    Err(err) => {
                        self.message = Some(
                            format!("failed to deserialize config: {err}")
                        );
                        return;
                    }
                };
                if let Err(err) = file.write_all(source.as_bytes()) {
                    self.message = Some(
                        format!("failed to write to config file: {err}")
                    );
                }
                self.saved_to_file = true;

                self.controller.is_modified = false;
                self.disc.is_modified = false;
                self.bios.is_modified = false;

                // Clear any previous error message.
                self.message = None;
            }
            Err(err) => self.message = Some(
                format!("failed to open config file: {err}")
            ),
        }
    }

    /// Handle a window key event. It should only be called when the config window is open.
    pub fn handle_key_event(
        &mut self,
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, Button)>,
        key: VirtualKeyCode,
        state: ElementState,
    ) -> bool {
        match self.menu {
            Menu::Controller => {
                self.controller.handle_key_event(key_map, key, state)
            }
            _ => false,
        }
    }

    pub fn handle_dropped_file(&mut self, path: &Path) {
        match self.menu {
            Menu::Bios => self.bios.handle_dropped_file(path),
            Menu::Disc => self.disc.handle_dropped_file(path),
            _ => (),
        }
    }

    /// Show inside config settings inside UI.
    pub fn show_inside(
        &mut self,
        used_bios: Option<&Bios>,
        controllers: &mut Controllers,
        disc: &mut Disc,
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, Button)>,
        ui: &mut egui::Ui,
    ) {
        egui::SidePanel::left("menu_panel")
            .max_width(32.0)
            .show_inside(ui, |ui| {
                ui.selectable_value(&mut self.menu, Menu::Controller, "Controller");
                ui.selectable_value(&mut self.menu, Menu::Bios, "Bios");
                ui.selectable_value(&mut self.menu, Menu::Disc, "Disc");

                ui.separator();

                if let Some(msg) = &self.message {
                    ui.label(msg);
                }
                if !self.saved_to_file || self.is_modified() {
                    if ui.button("Save to config").clicked() {
                        self.try_save_to_file();
                    }
                    if ui.button("Reload from Config").clicked() {
                        *self = Self::from_file_or_default();
                    }
                } else {
                    ui.label("Saved to Config");
                }
            });

        match self.menu {
            Menu::Controller => self.controller.show(controllers, key_map, ui),
            Menu::Bios => self.bios.show(used_bios, ui),
            Menu::Disc => self.disc.show(disc, ui),
        }
    }

    pub fn show(
        &mut self,
        used_bios: Option<&Bios>,
        controllers: &mut Controllers,
        disc: &mut Disc,
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, Button)>,
        ctx: &egui::Context,
    ) {
        egui::SidePanel::left("settings").show(ctx, |ui| {
            self.show_inside(used_bios, controllers, disc, key_map, ui)
        });
    }
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

