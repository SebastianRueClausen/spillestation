use splst_core::Disc;

use native_dialog::FileDialog;
use serde::{Serialize, Deserialize};

use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize)]
pub struct DiscConfig {
    #[serde(skip)]
    pub is_modified: bool,

    #[serde(skip)]
    add_path: String,

    #[serde(skip)]
    disc: Disc,

    #[serde(skip)]
    error: Option<String>,
   
    game_paths: Vec<PathBuf>,
}

impl DiscConfig {
    pub fn disc(&self) -> Disc {
        self.disc.clone()
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        match *self.disc.cd() {
            None => {
                ui.label("No Disc Loaded");
            }
            Some(ref cd) => {
                ui.label(cd.name());
                if ui.button("Eject").clicked() {
                    self.disc.eject();
                }
            }
        }

        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.add_path).hint_text("Path"));

            if ui.button("Select").clicked() {
                let loaded = FileDialog::new()
                    .set_location(".")
                    .show_open_single_file()
                    .unwrap_or(None);

                if let Some(loaded) = loaded {
                    self.add_path = loaded
                        .to_str()
                        .unwrap_or("Invalid path")
                        .to_string(); 
                }
            }

            if ui.button("Add").clicked() {
                self.is_modified = true;
                self.game_paths.push(PathBuf::from(&self.add_path));
            }
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("game_grid").show(ui, |ui| {
                let len_before = self.game_paths.len(); 
                self.game_paths.retain(|path| {
                    let name = path
                        .as_path()
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                        .to_string();

                    ui.label(name);

                    let retain = !ui.button("Remove").clicked();

                    if ui.button("Load").clicked() {
                        match splst_cdimg::open_cd(path) {
                            Err(err) => self.error = Some(err.to_string()),
                            Ok(cd) => {
                                self.disc.load(cd);
                            }
                        }
                    }
    
                    ui.end_row();

                    retain
                });

                if len_before != self.game_paths.len() {
                    self.is_modified = true; 
                }
            });
        });
    }
}
