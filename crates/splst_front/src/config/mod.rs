mod controller;
mod bios;
mod disc;

use splst_core::{Bios, io_port::{IoSlot, pad}, Disc};

use crate::gui::GuiContext;
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
    #[serde(skip)]
    show_bios: bool,

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
        self.show_bios = true;
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
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, pad::Button)>,
        key: VirtualKeyCode,
        state: ElementState,
    ) -> bool {
        self.controller.handle_key_event(key_map, key, state)
    }

    pub fn handle_dropped_file(&mut self, _: &Path) {
        // TODO.
    }

    /// Show inside config settings inside UI.
    pub fn show_inside(
        &mut self,
        used_bios: Option<&Bios>,
        controllers: &mut pad::Controllers,
        disc: &mut Disc,
        ui: &mut egui::Ui,
    ) {
        ui.horizontal(|ui| {
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

            if let Some(msg) = &self.message {
                ui.label(msg);
            }
        });
        
        ui.collapsing("Controller", |ui| {
            self.controller.show(controllers, ui);
        });

        let bios_open = if self.show_bios {
            Some(true)
        } else {
            None  
        };

        let bios = egui::CollapsingHeader::new("Bios")
            .open(bios_open)
            .show(ui, |ui| self.bios.show(used_bios, ui));

        ui.collapsing("Disc", |ui| {
            self.disc.show(disc, ui);
        });
        
        if self.show_bios {
            self.show_bios = false;
            ui.scroll_to_rect(bios.header_response.rect, Some(egui::Align::Center));
        }
    }

    /// Show as side panel of the while window.
    pub fn show(
        &mut self,
        used_bios: Option<&Bios>,
        controllers: &mut pad::Controllers,
        disc: &mut Disc,
        ctx: &GuiContext,
    ) {
        egui::SidePanel::left("settings").show(&ctx.egui_ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.show_inside(used_bios, controllers, disc, ui)
            });
        });
    }
}
