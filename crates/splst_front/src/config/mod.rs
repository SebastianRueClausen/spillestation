mod controller;
mod bios;
mod disc;
mod exe;
mod quick_access;

use splst_core::{Bios, io_port::{IoSlot, pad}, Disc};

use crate::gui::GuiContext;
use controller::ControllerConfig;
use bios::BiosConfig;
use disc::DiscConfig;
use exe::ExeConfig;

use winit::event::{VirtualKeyCode, ElementState};

use std::io::Write;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use std::fs;

/// Configuration for the emulator. This holds all the settings for the emulator like controller
/// key bindings, the disc loaded and the BIOS used. It's can be serialized and deserialized to
/// allow for saving the settings the a config file. It can also be rendered as GUI.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// Path to config file.
    ///
    /// For now it's just the caching the default path, but the user should be able to specify a
    /// path in the future.
    #[serde(skip)]
    config_path: Option<PathBuf>,

    /// This is set if the user has tried starting the emulator without specifying a BIOS.
    #[serde(skip)]
    show_bios: bool,

    /// If the config is saved to a config file. It dosn't represent if the config has been
    /// modified compared to the version the config file.
    #[serde(skip)]
    saved_to_file: bool,

    #[serde(default)]
    pub controller: ControllerConfig,

    #[serde(default)]
    pub bios: BiosConfig,

    #[serde(default)]
    pub disc: DiscConfig,

    #[serde(default)]
    pub exe: ExeConfig,
}

impl Config {
    /// If the config is modified compared to either default, loaded from a config file or saved to
    /// a config file.
    fn is_modified(&self) -> bool {
        self.controller.is_modified()
            || self.bios.is_modified()
            || self.disc.is_modified()
            || self.exe.is_modified()
    }

    /// Show the BIOS menu. Used when trying to start the emulator without a loaded BIOS.
    pub fn show_bios_menu(&mut self) {
        self.show_bios = true;
    }

    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|mut dir| {
           dir.push("spillestation");   
           dir.push("config.toml");
           dir
        })
    }

    /// Try load config from a config file at the default location. If loading or deserializing the
    /// config file fails, the default configs will be returned.
    pub fn from_file_or_default(ctx: &mut GuiContext) -> Self {
        let Some(path) = Self::config_path() else {
            ctx.error("Config Error", "Failed to decide config directory.");
            return Self::default();
        };
        match fs::read_to_string(&path) {
            Err(err) => {
                ctx.error("Config Error", format!("Failed to open config file: {err}."));
                return Self { config_path: Some(path), ..Default::default() };
            }
            Ok(toml) => match toml::from_str::<Config>(&toml) {
                Ok(mut config) => {
                    config.saved_to_file = true;
                    config.config_path = Some(path);

                    config
                }
                Err(err) => {
                    ctx.error("Config Error", format!("Failed load config file: {err}."));
                    return Self { config_path: Some(path), ..Default::default() }
                },
            }
        }
    }

    /// Try to save the configs to a config file.
    fn try_save_to_file(&mut self, ctx: &mut GuiContext) {
        let Some(path) = &self.config_path else {
            ctx.error("Config Error", "Unable to decide config directory.");
            return;
        };
        let Some(Ok(_)) = path.parent().map(fs::create_dir_all) else {
            ctx.error("Config Error", format!("Unable to create config folder for {path:?}"));
            return; 
        };
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path);
        match file {
            Err(err) => ctx.error("Config Error", format!("Failed to open config file: {err}.")),
            Ok(mut file) => {
                let source = match toml::to_string_pretty(self) {
                    Ok(source) => source,
                    Err(err) => {
                        ctx.error("Config Error", format!("Failed to deserialize config: {err}."));
                        return;
                    }
                };
                if let Err(err) = file.write_all(source.as_bytes()) {
                    ctx.error("Config Error", format!("Failed to write to config file: {err}."));
                }

                self.saved_to_file = true;

                self.controller.mark_as_saved();
                self.disc.mark_as_saved();
                self.bios.mark_as_saved();
                self.exe.mark_as_saved();
            }
        }
    }

    /// Handle a window key event. It should only be called when the config window is open.
    pub fn handle_key_event(
        &mut self,
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, pad::Button)>,
        key: VirtualKeyCode,
        state: ElementState,
        ctx: &mut GuiContext,
    ) -> bool {
        self.controller.handle_key_event(key_map, key, state, ctx)
    }

    pub fn handle_dropped_file(&mut self, _: &Path) {
        // TODO.
    }

    pub fn show_inside(
        &mut self,
        used_bios: Option<&Bios>,
        controllers: &mut pad::Controllers,
        disc: &mut Disc,
        ctx: &mut GuiContext,
        ui: &mut egui::Ui,
    ) {
        ui.horizontal(|ui| {
            let name = self.config_path
                .as_ref()
                .map(|path| {
                    path.as_path()
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or("config file".to_string());

            if !self.saved_to_file || self.is_modified() {
                if ui.button(format!("Save to {name}")).clicked() {
                    self.try_save_to_file(ctx);
                }
                if ui.button(format!("Reload from {name}")).clicked() {
                    *self = Self::from_file_or_default(ctx);
                }
            } else {
                ui.label(format!("Saved to {name}"));
            }
        });
        
        ui.collapsing("Controller", |ui| self.controller.show(controllers, ui));

        let bios_open = if self.show_bios { Some(true) } else { None };
        let bios = egui::CollapsingHeader::new("Bios")
            .open(bios_open)
            .show(ui, |ui| self.bios.show(used_bios, ctx, ui));

        ui.collapsing("Disc", |ui| self.disc.show(disc, ctx, ui));
        ui.collapsing("Executable", |ui| self.exe.show(ctx, ui));
        
        if self.show_bios {
            self.show_bios = false;
            ui.scroll_to_rect(bios.header_response.rect, Some(egui::Align::Center));
        }
    }

    pub fn show(
        &mut self,
        used_bios: Option<&Bios>,
        controllers: &mut pad::Controllers,
        disc: &mut Disc,
        ctx: &mut GuiContext,
    ) {
        egui::SidePanel::left("settings").show(&ctx.egui_ctx.clone(), |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.show_inside(used_bios, controllers, disc, ctx, ui)
            });
        });
    }
}
