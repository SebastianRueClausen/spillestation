use splst_core::{Bios, io_port::pad, Disc};
use crate::gui;
use crate::RunMode;
use super::config::Config;

pub fn show(
    config: &mut Config,
    controllers: &mut pad::Controllers,
    disc: &mut Disc,
    ctx: &mut gui::GuiCtx,
) -> Option<(Bios, RunMode)> {
    egui::CentralPanel::default().show(&ctx.egui_ctx.clone(), |ui| {
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
                    config.show_inside(None, controllers, disc, ctx, ui);
                    ui.horizontal(|ui| {
                        let mut take_bios = || {
                            config.bios.take_bios(ctx).or_else(|| {
                                config.show_bios_menu();
                                ctx.errors.add("No BIOS", "A BIOS must be loaded to start the emulator");
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
