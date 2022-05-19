use splst_core::{Bios, io_port::pad, Disc};
use crate::gui::GuiContext;
use super::config::Config;

/// Start menu shoved when starting the emulator.
pub struct StartMenu {
    error: Option<String>,
}

impl StartMenu {
    pub fn new() -> Self {
        Self { error: None }
    }

    pub fn show_area(
        &mut self,
        config: &mut Config,
        controllers: &mut pad::Controllers,
        disc: &mut Disc,
        ctx: &GuiContext,
    ) -> Option<Bios> {
        egui::CentralPanel::default().show(&ctx.egui_ctx, |ui| {
            let space = ui.available_size() / 16.0;
            ui.allocate_space(space);

            ui.vertical_centered_justified(|ui| {
                ui.heading("Spillestation");
            });

            ui.allocate_space(space);

            let mut bios = None;
            
            // This should never really scroll at any time, it's only to limit it's height.
            egui::ScrollArea::vertical()
                .max_width(ui.available_width())
                .show(ui, |ui| {
                    config.show_inside(None, controllers, disc, ui);
                    ui.horizontal(|ui| {
                        if ui.button("Start").clicked() {
                            bios = config.bios
                                .take_bios()
                                .or_else(|| {
                                    config.show_bios_menu();
                                    self.error = Some(
                                        "A BIOS must be loaded to start the emulator"
                                            .to_string()
                                    );
                                    None
                                });
                        }

                        if let Some(err) = &self.error {
                            ui.label(err);
                        }
                    });
            });
            
            bios
        })
        .inner
    }
}
