mod quick_access;

use splst_core::{Bios, io_port::{IoSlot, pad}, Disc};
use splst_util::Exe;
use crate::keys;
use crate::gui::Popups;
use quick_access::QuickAccess;

use winit::event::{VirtualKeyCode, ElementState};

use std::io::Write;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
enum Connection {
    Unconnected,
    Virtual,
    // DualShock,
}

impl Default for Connection {
    fn default() -> Self {
        Connection::Unconnected
    }
}

#[repr(C)]
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct ButtonBindings {
    select: Option<VirtualKeyCode>,  
    l3: Option<VirtualKeyCode>,  
    r3: Option<VirtualKeyCode>,  
    start: Option<VirtualKeyCode>,  
    up: Option<VirtualKeyCode>,  
    right: Option<VirtualKeyCode>,  
    down: Option<VirtualKeyCode>,  
    left: Option<VirtualKeyCode>,
    l2: Option<VirtualKeyCode>,  
    r2: Option<VirtualKeyCode>,  
    l1: Option<VirtualKeyCode>,  
    r1: Option<VirtualKeyCode>,  
    triangle: Option<VirtualKeyCode>,  
    circle: Option<VirtualKeyCode>,  
    cross: Option<VirtualKeyCode>,  
    square: Option<VirtualKeyCode>,  
}

impl ButtonBindings {
    fn as_slice_mut(&mut self) -> &mut [Option<VirtualKeyCode>] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self as *mut Self as *mut Option<VirtualKeyCode>, 16
            )
        }
    }

    fn as_slice(&self) -> &[Option<VirtualKeyCode>] {
        unsafe {
            std::slice::from_raw_parts(self as *const Self as *const Option<VirtualKeyCode>, 16)
        }
    }
}

fn gen_key_map(
    slot1: &ButtonBindings,
    slot2: &ButtonBindings,
) -> Result<HashMap<VirtualKeyCode, (IoSlot, pad::Button)>, String> {
    let s1 = slot1
        .as_slice()
        .iter()
        .zip(pad::Button::ALL.iter())
        .filter_map(|(binding, button)| {
            binding.map(|key| (*button, IoSlot::Slot1, key))
        });
    let s2 = slot2
        .as_slice()
        .iter()
        .zip(pad::Button::ALL.iter())
        .filter_map(|(binding, button)| {
            binding.map(|key| (*button, IoSlot::Slot2, key))
        });

    let mut map = HashMap::new();

    for (button, slot, key) in s1.chain(s2) {
        if let Some((slot2, button2)) = map.insert(key, (slot, button)) {
            return Err(format!(
                "Duplicate key bindings: {button} for {slot} and {button2} for {slot2} are both bound to {}",
                keys::keycode_name(key),
            ));
        }
    }
    Ok(map)
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct GamePadConfig {
    #[serde(rename = "connection-1")]
    conn1: Connection,

    #[serde(rename = "connection-2")]
    conn2: Connection,

    #[serde(rename = "slot-1")]
    slot1: ButtonBindings,

    #[serde(rename = "slot-2")]
    slot2: ButtonBindings,

    #[serde(skip)]
    show_slot: IoSlot,

    #[serde(skip)]
    recording: Option<(IoSlot, pad::Button)>,

    #[serde(skip)]
    pub modified: bool,
}

impl GamePadConfig {
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn mark_as_saved(&mut self) {
        self.modified = false;
    }

    /// Get the key map corresponding to the controller settings. It will always generate a new key
    /// map. If generating the key map fails, an empty map will be returned and the menu will show
    /// and error.
    pub fn get_key_map(
        &mut self,
        popups: &mut Popups,
    ) -> HashMap<VirtualKeyCode, (IoSlot, pad::Button)> {
        gen_key_map(&self.slot1, &self.slot2)
            .map_err(|err| popups.add("Key Error", err))
            .unwrap_or_default()
    }

    /// Handle a key press if the menu is open, either by the menu being open while running or in
    /// the start menu. It's used when recording key bindings.
    ///
    /// Returns true if the input is captured.
    pub fn handle_key_event(
        &mut self,
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, pad::Button)>,
        key: VirtualKeyCode,
        state: ElementState,
        popups: &mut Popups,
    ) -> bool {
        if state != ElementState::Pressed {
            return false;
        }

        if let Some((slot, button)) = self.recording {
            self.recording = None;
            self.modified = true;

            let bindings = match slot {
                IoSlot::Slot1 => &mut self.slot1,
                IoSlot::Slot2 => &mut self.slot2,
            };

            bindings.as_slice_mut()[button as usize] = Some(key);

            // Create a new key map or report the error. Also clears the error if generating the
            // key map succeeded.
            match gen_key_map(&self.slot1, &self.slot2) {
                Ok(map) => *key_map = map,
                Err(err) => popups.add("Key Error", err),
            }

            true
        } else {
            false
        }
    }

    fn show_buttons(&mut self, slot: IoSlot, ui: &mut egui::Ui) {
        let bindings = match slot {
            IoSlot::Slot1 => &mut self.slot1,
            IoSlot::Slot2 => &mut self.slot2,
        };

        egui::Grid::new("button_grid").show(ui, |ui| {
            bindings
                .as_slice_mut()
                .iter_mut()
                .zip(pad::Button::ALL.iter())
                .for_each(|(binding, button)| {
                    ui.label(format!("{button}"));
                    let key_name = binding
                        .map(|key| keys::keycode_name(key))
                        .unwrap_or("Unbound");
                    let rebind = egui::Button::new(key_name);
                    if ui.add_sized([60.0, 8.0], rebind).clicked() {
                        self.recording = Some((slot, *button));
                    }
                    ui.end_row();
                });
        });
    }

    /// Update [`pad::GamePads`] from config. It should only be called when the configs could
    /// have changed since it will reset the internal state of the controllers.
    pub fn update(&self, gamepads: &mut pad::GamePads) {
        for (pad, conn) in gamepads.iter_mut().zip([self.conn1, self.conn2].iter()) {
            *pad = match conn {
                Connection::Unconnected => None,
                Connection::Virtual => Some(pad::PadKind::new_digital())
            };
        }
    }

    pub fn show(&mut self, gamepads: &mut pad::GamePads, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.recording.is_none(), |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.show_slot, IoSlot::Slot1, "Slot 1");
                ui.selectable_value(&mut self.show_slot, IoSlot::Slot2, "Slot 2");
            });
            
            ui.add_space(10.0);

            let connection = match self.show_slot {
                IoSlot::Slot1 => &mut self.conn1,
                IoSlot::Slot2 => &mut self.conn2,
            };

            let before = *connection;

            egui::ComboBox::from_label("Connection")
                .selected_text(format!("{:?}", connection))
                .show_ui(ui, |ui| {
                    ui.selectable_value(connection, Connection::Unconnected, "Unconnected");
                    ui.selectable_value(connection, Connection::Virtual, "Virtual");
                });

            if before != *connection {
                self.modified = true;
                *gamepads.get_mut(self.show_slot) = match *connection {
                    Connection::Unconnected => None,
                    Connection::Virtual => Some(pad::PadKind::new_digital()),
                };
            }

            ui.add_space(10.0);

            egui::ScrollArea::new([false, true]).show(ui, |ui| {
                self.show_buttons(self.show_slot, ui);
            });
        });
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct BiosConfig {
    #[serde(skip)]
    loaded: Option<Bios>,
    default: Option<PathBuf>,
    bioses: QuickAccess,
}

impl BiosConfig {
    pub fn is_modified(&self) -> bool {
        self.bioses.modified
    }

    pub fn mark_as_saved(&mut self) {
        self.bioses.modified = false;
    }

    pub fn take_bios(&mut self, popups: &mut Popups) -> Option<Bios> {
        self.loaded.take().or_else(|| {
            if let Some(default) = &self.default {
                match Bios::from_file(&default) {
                    Err(err) => popups.add("BIOS Error", err.to_string()),
                    Ok(bios) => return Some(bios),
                }
            }
            None
        })
    }

    pub fn show(&mut self, used: Option<&Bios>, popups: &mut Popups, ui: &mut egui::Ui) {
        match used {
            Some(bios) => {
                ui.add_enabled_ui(false, |ui| ui.label(bios.name()));
            }
            None => match &self.loaded {
                Some(bios) => {
                    let unload = ui.horizontal(|ui| {
                        ui.label(bios.name());
                        ui.button("Unload").clicked()
                    })
                    .inner;

                    if unload {
                        self.loaded = None;
                    }
                }
                None => {
                    ui.label("A BIOS File must be loaded");
                }
            }
        }

        ui.add_space(10.0);

        if let Some(path) = self.bioses.show("bios", ui) {
            match Bios::from_file(&path) {
                Err(err) =>  popups.add("BIOS Error", err.to_string()),
                Ok(bios) => self.loaded = Some(bios),
            }
        }
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct DiscConfig {
    discs: QuickAccess,
}

impl DiscConfig {
    pub fn is_modified(&self) -> bool {
        self.discs.modified
    }

    pub fn mark_as_saved(&mut self) {
        self.discs.modified = false;
    }

    pub fn show(&mut self, disc: &mut Disc, popups: &mut Popups, ui: &mut egui::Ui) {
        match disc.cd() {
            None => {
                ui.label("No Disc Loaded");
            }
            Some(cd) => {
                let unload = ui.horizontal(|ui| {
                    ui.label(cd.name());
                    ui.button("Unload").clicked()
                })
                .inner;

                if unload {
                    disc.unload();
                }
            }
        }

        ui.add_space(10.0);

        if let Some(path) = self.discs.show("cue", ui) {
            match splst_cdimg::open_cd(&path) {
                Err(err) =>  popups.add("Disc Error", err.to_string()),
                Ok(cd) => disc.load(cd),
            } 
        }
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct ExeConfig {
    /// If an executable file has been loaded, but not taken yet.
    #[serde(skip)]
    loaded: Option<Exe>,
   
    /// The name of either the executable in `loaded` or the one that is currently loaded in the
    /// system.
    #[serde(skip)]
    name: Option<String>,

    #[serde(rename = "executables")]
    exes: QuickAccess,
}

impl ExeConfig {
    pub fn is_modified(&self) -> bool {
        self.exes.modified
    }

    pub fn mark_as_saved(&mut self) {
        self.exes.modified = false;
    }

    pub fn take_exe(&mut self) -> Option<Exe> {
        self.loaded.take()
    }

    pub fn show(&mut self, popups: &mut Popups, ui: &mut egui::Ui) {
        match &self.name {
            Some(name) if self.loaded.is_some() => {
                let unload = ui.horizontal(|ui| {
                    ui.label(name);
                    ui.button("Unload").clicked()
                })
                .inner;

                if unload {
                    self.loaded = None;
                }
            }
            Some(name) => {
                ui.add_enabled_ui(false, |ui| ui.label(name));
            }
            None => {
                ui.label("No executable loaded");
            }
        }

        ui.add_space(10.0);

        if let Some(path) = self.exes.show("exe", ui) {
            match Exe::load(&path) {
                Err(err) => popups.add("Exe Error", err.to_string()),
                Ok(exe) => {
                    let name = path
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                        .to_string();

                    self.loaded = Some(exe);
                    self.name = Some(name);
                }
            }
        }
    }
}

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
    pub gamepads: GamePadConfig,

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
        self.gamepads.is_modified()
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
    pub fn from_file_or_default(popups: &mut Popups) -> Self {
        let Some(path) = Self::config_path() else {
            popups.add("Config Error", "Failed to decide config directory.");
            return Self::default();
        };
        match fs::read_to_string(&path) {
            Err(err) => {
                popups.add("Config Error", format!("Failed to open config file: {err}."));
                return Self { config_path: Some(path), ..Default::default() };
            }
            Ok(toml) => match toml::from_str::<Config>(&toml) {
                Ok(mut config) => {
                    config.saved_to_file = true;
                    config.config_path = Some(path);

                    config
                }
                Err(err) => {
                    popups.add("Config Error", format!("Failed load config file: {err}."));
                    return Self { config_path: Some(path), ..Default::default() }
                },
            }
        }
    }

    /// Try to save the configs to a config file.
    fn try_save_to_file(&mut self, popups: &mut Popups) {
        let Some(path) = &self.config_path else {
            popups.add("Config Error", "Unable to decide config directory.");
            return;
        };
        let Some(Ok(_)) = path.parent().map(fs::create_dir_all) else {
            popups.add("Config Error", format!("Unable to create config folder for {path:?}"));
            return; 
        };
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path);
        match file {
            Err(err) => popups.add("Config Error", format!("Failed to open config file: {err}.")),
            Ok(mut file) => {
                let source = match toml::to_string_pretty(self) {
                    Ok(source) => source,
                    Err(err) => {
                        popups.add("Config Error", format!("Failed to deserialize config: {err}."));
                        return;
                    }
                };
                if let Err(err) = file.write_all(source.as_bytes()) {
                    popups.add("Config Error", format!("Failed to write to config file: {err}."));
                }

                self.saved_to_file = true;

                self.gamepads.mark_as_saved();
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
        popups: &mut Popups,
    ) -> bool {
        self.gamepads.handle_key_event(key_map, key, state, popups)
    }

    pub fn handle_dropped_file(&mut self, _: &Path) {
        // TODO.
    }

    pub fn show_inside(
        &mut self,
        used_bios: Option<&Bios>,
        controllers: &mut pad::GamePads,
        disc: &mut Disc,
        popups: &mut Popups,
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
                    self.try_save_to_file(popups);
                }
                if ui.button(format!("Reload from {name}")).clicked() {
                    *self = Self::from_file_or_default(popups);
                }
            } else {
                ui.label(format!("Saved to {name}"));
            }
        });
        
        ui.collapsing("Controller", |ui| self.gamepads.show(controllers, ui));

        let bios_open = if self.show_bios { Some(true) } else { None };
        let bios = egui::CollapsingHeader::new("Bios")
            .open(bios_open)
            .show(ui, |ui| self.bios.show(used_bios, popups, ui));

        ui.collapsing("Disc", |ui| self.disc.show(disc, popups, ui));
        ui.collapsing("Executable", |ui| self.exe.show(popups, ui));
        
        if self.show_bios {
            self.show_bios = false;
            ui.scroll_to_rect(bios.header_response.rect, Some(egui::Align::Center));
        }
    }

    pub fn show(
        &mut self,
        used_bios: Option<&Bios>,
        gamepads: &mut pad::GamePads,
        disc: &mut Disc,
        popups: &mut Popups,
        ctx: &egui::Context,
    ) {
        egui::SidePanel::left("settings").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.show_inside(used_bios, gamepads, disc, popups, ui)
            });
        });
    }
}
