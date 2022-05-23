use splst_core::{Bios, io_port::pad, Disc};
use crate::gui::GuiContext;
use super::config::Config;

pub fn show(
    config: &mut Config,
    controllers: &mut pad::Controllers,
    disc: &mut Disc,
    ctx: &mut GuiContext,
) -> Option<Bios> {
    egui::CentralPanel::default().show(&ctx.egui_ctx.clone(), |ui| {
        let space = ui.available_size() / 16.0;
        ui.allocate_space(space);

        ui.vertical_centered_justified(|ui| ui.heading("Spillestation"));

        ui.allocate_space(space);

        let mut bios = None;
        
        // This should never really scroll at any time, it's only to limit it's height.
        egui::ScrollArea::vertical()
            .max_width(ui.available_width())
            .show(ui, |ui| {
                config.show_inside(None, controllers, disc, ctx, ui);
                ui.horizontal(|ui| {
                    if ui.button("Start").clicked() {
                        bios = config.bios.take_bios(ctx).or_else(|| {
                            config.show_bios_menu();
                            ctx.error("No BIOS", "A BIOS must be loaded to start the emulator");
                            None
                        });
                    }
                });
            });
        bios
    })
    .inner
}
