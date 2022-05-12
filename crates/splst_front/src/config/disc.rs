//! # TODO
//!
//! - Maybe check that the paths exists when adding them.
//!
//! - Add support for whole folder which we listen to and add files as they are added to the
//!   folder.

use splst_core::Disc;

use native_dialog::FileDialog;
use serde::{Serialize, Deserialize};

use std::path::{PathBuf, Path};

#[derive(Default, Serialize, Deserialize)]
pub struct DiscConfig {
    #[serde(skip)]
    pub is_modified: bool,

    #[serde(skip)]
    add_path: String,

    #[serde(skip)]
    error: Option<String>,
   
    paths: Vec<PathBuf>,
}

impl DiscConfig {
    pub fn _handle_dropped_file(&mut self, path: &Path) {
        self.is_modified = true;
        self.paths.push(path.to_path_buf());
    }

    pub fn show(&mut self, disc: &mut Disc, ui: &mut egui::Ui) {
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

        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.add_path).hint_text("CUE File"));

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

        ui.add_space(10.0);

        egui::Grid::new("game_grid").show(ui, |ui| {
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
                if ui.button("Load").clicked() {
                    match splst_cdimg::open_cd(path) {
                        Err(err) => self.error = Some(err.to_string()),
                        Ok(cd) => disc.load(cd),
                    }
                }

                ui.end_row();

                retain
            });

            if len_before != self.paths.len() {
                self.is_modified = true; 
            }
        });
    }
}
