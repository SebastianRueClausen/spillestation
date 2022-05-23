use splst_core::Disc;
use super::quick_access::QuickAccess;
use crate::gui::GuiContext;

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

    pub fn show(&mut self, disc: &mut Disc, ctx: &mut GuiContext, ui: &mut egui::Ui) {
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
                Err(err) =>  ctx.error("Disc Error", err.to_string()),
                Ok(cd) => disc.load(cd),
            } 
        }
    }
}
