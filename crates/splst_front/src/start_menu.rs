use splst_core::{Bios, io_port::{memcard, pad}, Disc};
use crate::gui::Popups;
use crate::RunMode;
use super::config::Config;

pub struct StartMenu {
    popups: Popups,
}

impl Default for StartMenu {
    fn default() -> Self {
        Self { popups: Popups::new("start_menu") }
    }
}

impl StartMenu {
    pub fn show(
        &mut self, 
        config: &mut Config,
        gamepads: &mut pad::GamePads,
        memcards: &mut memcard::MemCards,
        disc: &mut Disc,
        ctx: &egui::Context,
    ) -> Option<(Bios, RunMode)> {
        self.popups.show(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            let space = ui.available_size() / 16.0;

            ui.allocate_space(space);
            ui.vertical_centered_justified(|ui| {
                ui.heading("Spillestation")
            });

            ui.allocate_space(space);

            let mut out: Option<(Bios, RunMode)> = None;

            egui::ScrollArea::vertical()
                .max_width(ui.available_width())
                .show(ui, |ui| {
                    ui.group(|ui| {
                        config.show_inside(None, gamepads, memcards, disc, &mut self.popups, ui);
                        ui.horizontal(|ui| {
                            let mut take_bios = || {
                                config.bios.take_bios(&mut self.popups).or_else(|| {
                                    config.show_bios_menu();
                                    self.popups.add("No BIOS", "A BIOS must be loaded to start the emulator");
                                    None
                                })
                            };
                            if ui.button("Start").clicked() {
                                out = take_bios().map(|bios| (bios, RunMode::Emulation));
                            }
                            if ui.button("Start in debug mode").clicked() {
                                out = take_bios().map(|bios| (bios, RunMode::Debug));
                            }
                        });
                    });
                });
            out
        })
        .inner
    }
}
