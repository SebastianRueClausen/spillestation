use splst_core::Bios;

use serde::{Serialize, Deserialize};
use native_dialog::FileDialog;

use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
pub struct BiosConfig {
    #[serde(skip)]
    pub is_modified: bool,

    #[serde(skip)]
    add_path: String,

    #[serde(skip)]
    loaded: Option<Bios>,

    #[serde(skip)]
    error: Option<String>,
   
    paths: Vec<PathBuf>,
    default: Option<PathBuf>,
}

impl BiosConfig {
    pub fn take_bios(&mut self) -> Option<Bios> {
        self.loaded.take().or_else(|| {
            if let Some(default) = &self.default {
                match Bios::from_file(&Path::new(default)) {
                    Err(err) => self.error = Some(err.to_string()),
                    Ok(bios) => return Some(bios),
                }
            }
            None
        })
    }

    pub fn handle_dropped_file(&mut self, path: &Path) {
        self.is_modified = true;
        self.paths.push(path.to_path_buf()); 
    }

    pub fn show(&mut self, used: Option<&Bios>, ui: &mut egui::Ui) {
        match used {
            Some(bios) => {
                ui.add_enabled_ui(false, |ui| {
                    ui.label(bios.name());
                });
            }
            None => match self.loaded {
                Some(ref bios) => {
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

        ui.separator();

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
                self.paths.push(PathBuf::from(&self.add_path));
            }
        });

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("bios_grid").show(ui, |ui| {
                let len_before = self.paths.len();
                self.paths.retain(|path| {
                    let short = path
                        .as_path()
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy();

                    let long = path
                        .as_path()
                        .to_string_lossy();

                    ui.label(&*short).on_hover_text(&*long);

                    let retain = !ui.button("Remove").clicked();
                    if used.is_none() && ui.button("Load").clicked() {
                        match Bios::from_file(path) {
                            Err(err) => self.error = Some(err.to_string()),
                            Ok(bios) => self.loaded = Some(bios),
                        }
                    }
    
                    ui.end_row();

                    retain
                });

                if len_before != self.paths.len() {
                    self.is_modified = true;
                }
            });
        });
    }
}
