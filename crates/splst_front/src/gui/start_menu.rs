use splst_core::{Bios, Button};
use splst_cdimg::CdImage;
use crate::Config;
use crate::WithPath;
use crate::gui::keys;

use winit::event::VirtualKeyCode;
use native_dialog::FileDialog;

use std::path::{Path, PathBuf};
use std::collections::HashMap;

struct KeyBindings {
    bindings: [Option<VirtualKeyCode>; BUTTON_COUNT],
    recording: Option<Button>,
}

impl KeyBindings {
    fn new(bindings: [Option<VirtualKeyCode>; BUTTON_COUNT]) -> Self {
        Self { bindings, recording: None }
    }
}

pub struct StartMenu {
    bios: Option<WithPath<Bios>>,
    cd_image: Option<WithPath<CdImage>>,
    error: Option<String>,
    bios_path: String,
    cd_path: String,
    bios_in_config: bool,
    key_bindings: KeyBindings,
}

impl StartMenu {
    pub fn new(
        bios: Option<WithPath<Bios>>,
        key_bindings: Option<HashMap<VirtualKeyCode, Button>>,
        error: Option<String>,
    ) -> Self {
        let mut bindings = [None; BUTTON_COUNT];
        if let Some(keys) = key_bindings {
            for (k, v) in &keys {
                bindings[*v as usize] = Some(*k);
            }
        }
        Self {
            bios,
            error,
            cd_image: None,
            bios_path: String::new(),
            cd_path: String::new(),
            bios_in_config: bios.is_some(),
            key_bindings: KeyBindings::new(bindings),
        }
    }

    fn key_bindings(&mut self, ui: &mut egui::Ui) {
        if let Some(button) = self.key_bindings.recording {
            for event in ui.input().events {
                if let egui::Event::Key { key, pressed: true, ..  } = event {
                    self.key_bindings.recording = None;
                    self.key_bindings.bindings[button as usize] = Some(
                        keys::egui_to_winit_key(key)
                    ); 
                    break;
                }
            }
        }
        ui.group(|ui| {
            egui::ScrollArea::new([false, true]).show(ui, |ui| {
                egui::Grid::new("key_bindings").show(ui, |ui| {
                    self.key_bindings.bindings
                        .iter()
                        .zip(BUTTONS_NAMES.iter())
                        .zip(BUTTONS.iter())
                        .for_each(|((key, name), button)| {
                            ui.label(*name);
                            let key = key
                                .map(|key| keys::keycode_name(key))
                                .unwrap_or("None");
                            ui.label(key);
                            if ui.button("change").clicked() {
                                self.key_bindings.recording = Some(*button);     
                            }
                        });
                });
            });
        });
    }

    fn bios_and_game(
        &mut self,
        ui: &mut egui::Ui
    ) -> Option<(Bios, Option<CdImage>, HashMap<VirtualKeyCode, Button>)> {
        ui.group(|ui| {
            match self.bios {
                Some(ref bios) => {
                    let change = ui.horizontal(|ui| {
                        if !self.bios_in_config {
                            ui.label(format!("BIOS Loaded '{}' ✔", bios.name));
                            if ui.button("Save to Config File").clicked() {
                                let res = Config::store(&Config {
                                    bios: bios.path.clone(),
                                    key_bindings: self.key_bindings.bindings
                                        .iter()
                                        .zip(BUTTONS.iter())
                                        .filter_map(|(k, b)| k.map(|k| (k, *b)))
                                        .collect(),
                                });
                                match res {
                                    Err(err) => self.error = Some(err.to_string()),
                                    Ok(()) => self.bios_in_config = true,
                                }
                            }
                        } else {
                            ui.label(format!("BIOS loaded '{}' and saved in Config File ✔", bios.name));
                        }
                        ui.button("Change").clicked()
                    });

                    if change.inner {
                        self.bios_in_config = false;
                        self.bios = None;
                    }
                }
                None => {
                    ui.label("A BIOS must be loaded to use the Emulator");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.bios_path).hint_text("Path"));

                        if ui.button("Select").clicked() {
                            let loaded = FileDialog::new()
                                .set_location(".")
                                .show_open_single_file()
                                .unwrap_or(None);

                            if let Some(loaded) = loaded {
                                self.bios_path = loaded
                                    .to_str()
                                    .unwrap_or("Invalid path")
                                    .to_string(); 
                            }
                        }

                        if ui.button("Load").clicked() {
                            match Bios::from_file(Path::new(&self.bios_path)) {
                                Err(err) => self.error = Some(err.to_string()),
                                Ok(bios) => self.bios = Some(WithPath::new(
                                    bios, PathBuf::from(self.bios_path.clone()),
                                )),
                            }
                        }
                    });
                }
            }

            ui.allocate_space(ui.available_size() / 32.0);

            match self.cd_image {
                Some(cd_image) => {
                    let change = ui.horizontal(|ui| {
                        ui.label(format!("Game Loaded '{}' ✔", cd_image.name));
                        ui.button("Change").clicked()
                    });

                    if change.inner {
                        self.cd_image = None; 
                    }
                }
                None => {
                    ui.label("No Game Loaded");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.cd_path).hint_text("Path"));

                        if ui.button("Select").clicked() {
                            let loaded = FileDialog::new()
                                .set_location(".")
                                .show_open_single_file()
                                .unwrap_or(None);

                            if let Some(loaded) = loaded {
                                self.cd_path = loaded
                                    .to_str()
                                    .unwrap_or("Invalid path")
                                    .to_string(); 
                            }
                        }

                        if ui.button("Load").clicked() {
                            match splst_cdimg::open_cd(Path::new(&self.cd_path)) {
                                Err(err) => self.error = Some(err.to_string()),
                                Ok(cd) => self.cd_image = Some(WithPath::new(
                                    cd, PathBuf::from(self.cd_path.clone()),
                                )),
                            }
                        }
                    });
                }
            }
        });

        if let Some(ref error) = self.error {
            ui.label(error);
        }

        let val = ui.horizontal(|ui| {
            if ui.button("Start").clicked() {
                match (self.bios.is_some(), self.cd_image.is_some()) {
                    (false, _) => {
                        self.error = Some(
                            "A BIOS must be loaded to start the emulator".to_string()
                        );
                    }
                    (true, false) => {
                        self.error = Some(
                            "No Game is loaded. Click 'Start without Game' to start anyway".to_string()
                        ); 
                    }
                    (true, true) => {
                        return Some((
                            self.bios
                                .take()
                                .map(|bios| bios.item)
                                .unwrap(),
                            self.cd_image
                                .take()
                                .map(|cd| cd.item),
                            self.key_bindings.bindings
                                .iter()
                                .zip(BUTTONS.iter())
                                .filter_map(|(k, b)| k.map(|k| (k, *b)))
                                .collect(),
                        ));
                    }
                }
            }

            if ui.button("Start without Game").clicked() {
                if self.bios.is_some() {
                    return Some((
                        self.bios
                            .take()
                            .map(|bios| bios.item)
                            .unwrap(),
                        self.cd_image
                            .take()
                            .map(|cd| cd.item),
                        self.key_bindings.bindings
                            .iter()
                            .zip(BUTTONS.iter())
                            .filter_map(|(k, b)| k.map(|k| (k, *b)))
                            .collect(),
                    ));
                } else {
                    self.error = Some(
                        "A BIOS must be loaded to start the emulator".to_string()
                    );
                }
            }
            None
        });
        val.inner
    }

    pub fn show_area(
        &mut self,
        ctx: &egui::CtxRef,
    ) -> Option<(Bios, Option<CdImage>, HashMap<VirtualKeyCode, Button>)> {
        egui::CentralPanel::default().show(ctx, |ui| {
            let space = ui.available_size() / 16.0;
            ui.allocate_space(space);
            ui.vertical_centered_justified(|ui| {
                ui.label(egui::WidgetText::RichText(
                    egui::RichText::new("Spillestation")
                        .text_style(egui::TextStyle::Heading)
                        .color(egui::Color32::BLACK)
                ));
            });
            ui.allocate_space(space);
            self.bios_and_game(ui)
        })
        .inner
    }
}

const BUTTON_COUNT: usize = 16;

const BUTTONS_NAMES: [&str; BUTTON_COUNT] = [
    "Select",
    "L3",
    "R3",
    "Start",
    "Up",
    "Right",
    "Down",
    "Left",
    "L2",
    "R2",
    "L1",
    "R1",
    "Triangle",
    "Circle",
    "Cross",
    "Square",
];

const BUTTONS: [Button; BUTTON_COUNT] = [
    Button::Select,
    Button::L3,
    Button::R3,
    Button::Start,
    Button::JoyUp,
    Button::JoyRight,
    Button::JoyDown,
    Button::JoyLeft,
    Button::L2,
    Button::R2,
    Button::L1,
    Button::R1,
    Button::Triangle,
    Button::Circle,
    Button::Cross,
    Button::Square,

];
