use splst_core::Bios;
use splst_cdimg::CdImage;
use crate::Config;

use native_dialog::FileDialog;
use std::path::Path;

pub struct StartMenu {
    bios: Option<(String, Bios)>,
    cd_image: Option<(String, CdImage)>,
    error: Option<String>,
    bios_path: String,
    cd_path: String,
    bios_in_config: bool,
}

impl StartMenu {
    pub fn with_bios(bios: Bios, path: String) -> Self {
        Self {
            bios: Some((path, bios)),
            cd_image: None,
            error: None,
            bios_path: String::new(),
            cd_path: String::new(),
            bios_in_config: true,
        }
    }

    pub fn with_error(error: String) -> Self {
        Self {
            bios: None,
            cd_image: None,
            error: Some(error),
            bios_path: String::new(),
            cd_path: String::new(),
            bios_in_config: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<(Bios, Option<CdImage>)> {
        ui.group(|ui| {
            match self.bios {
                Some((ref path, _)) => {
                    ui.horizontal(|ui| {
                        if !self.bios_in_config {
                            ui.label("BIOS Loaded ✔");
                            if ui.button("Save to Config File").clicked() {
                                let res = Config::store(&Config {
                                    bios: path.clone()
                                });
                                match res {
                                    Err(err) => self.error = Some(err.to_string()),
                                    Ok(()) => self.bios_in_config = true,
                                }
                            }
                        } else {
                            ui.label("BIOS loaded and saved in Config File ✔");
                        }
                    });
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
                                Ok(bios) => self.bios = Some((self.bios_path.clone(), bios)),
                            }
                        }
                    });
                }
            }

            ui.allocate_space(ui.available_size() / 32.0);

            match self.cd_image {
                Some((ref path, _)) => {
                    ui.label(format!("Game Loaded '{path}' ✔"));
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
                                Ok(cd) => self.cd_image = Some((self.cd_path.clone(), cd)),
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
                            self.bios.take().map(|(_, bios)| bios).unwrap(),
                            self.cd_image.take() .map(|(_, cd)| cd)
                        ));
                    }
                }
            }

            if ui.button("Start without Game").clicked() {
                if self.bios.is_some() {
                    return Some((
                        self.bios.take().map(|(_, bios)| bios).unwrap(),
                        self.cd_image.take().map(|(_, cd)| cd)
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

    pub fn show_area(&mut self, ctx: &egui::CtxRef) -> Option<(Bios, Option<CdImage>)> {
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
            self.show(ui)
        })
        .inner
    }
}
