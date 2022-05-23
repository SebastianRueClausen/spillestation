
use native_dialog::FileDialog;
use std::path::PathBuf;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct QuickAccess {
    #[serde(skip)] 
    input: String, 

    #[serde(skip)]
    pub modified: bool,

    paths: Vec<PathBuf>,
}

impl QuickAccess {
    pub fn show(&mut self, file_hint: &str, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut load = None;

        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.input).hint_text(file_hint));

            if ui.button("Select").clicked() {
                let loaded = FileDialog::new()
                    .set_location(".")
                    .show_open_single_file()
                    .unwrap_or(None);

                if let Some(loaded) = loaded {
                    self.input = loaded
                        .to_str()
                        .unwrap_or("Invalid path")
                        .to_string(); 
                }
            }

            if ui.button("Save").clicked() {
                self.modified = true;
                self.paths.push(PathBuf::from(&self.input));
            }

            if ui.button("Load").clicked() {
                load = Some(PathBuf::from(&self.input)); 
            }
        });

        ui.add_space(10.0);

        if self.paths.is_empty() {
            ui.label(format!("No {file_hint} saved"));
            return load;
        }

        egui::Grid::new("grid").show(ui, |ui| {
            let len_before = self.paths.len(); 
            self.paths.retain(|path| {
                let short = path
                    .as_path()
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy();
                let long = path.as_path().to_string_lossy();

                ui.label(&*short).on_hover_text(&*long);

                let retain = !ui.button("Remove").clicked();
                if ui.button("Load").clicked() {
                    load = Some(path.to_path_buf());
                }

                ui.end_row();

                retain
            });

            if len_before != self.paths.len() {
                self.modified = true; 
            }
        });

        load
    }
}
