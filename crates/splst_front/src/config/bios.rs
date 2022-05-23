use splst_core::Bios;
use super::quick_access::QuickAccess;
use crate::gui::GuiContext;

use std::path::PathBuf;

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

    pub fn take_bios(&mut self, ctx: &mut GuiContext) -> Option<Bios> {
        self.loaded.take().or_else(|| {
            if let Some(default) = &self.default {
                match Bios::from_file(&default) {
                    Err(err) => ctx.error("BIOS Error", err.to_string()),
                    Ok(bios) => return Some(bios),
                }
            }
            None
        })
    }

    pub fn show(&mut self, used: Option<&Bios>, ctx: &mut GuiContext, ui: &mut egui::Ui) {
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
                Err(err) =>  ctx.error("BIOS Error", err.to_string()),
                Ok(bios) => self.loaded = Some(bios),
            }
        }
    }
}
