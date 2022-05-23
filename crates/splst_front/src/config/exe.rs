use splst_core::exe::Exe;
use super::quick_access::QuickAccess;
use crate::gui::GuiContext;

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

    pub fn show(&mut self, ctx: &mut GuiContext, ui: &mut egui::Ui) {
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
                Err(err) => ctx.error("Exe Error", err.to_string()),
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
