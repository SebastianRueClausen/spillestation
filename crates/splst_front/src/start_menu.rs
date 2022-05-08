use splst_core::{Bios, Controllers, IoSlot, Button, Disc};
use super::config::Config;

use winit::event::VirtualKeyCode;
use std::collections::HashMap;

/// Start menu shoved when starting the emulator.
pub struct StartMenu {
    error: Option<String>,
}

impl StartMenu {
    pub fn new() -> Self {
        Self { error: None }
    }

    fn show_settings(
        &mut self,
        config: &mut Config,
        controllers: &mut Controllers,
        disc: &mut Disc,
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, Button)>,
        ui: &mut egui::Ui,
    ) -> Option<Bios> {
        // This should never really scroll at any time, it's only to limit it's height.
        egui::ScrollArea::neither()
            .max_height(ui.available_size().y / 1.1)
            .show(ui, |ui| {
                ui.group(|ui| {
                    config.show_inside(None, controllers, disc, key_map, ui);
                });
            });

        ui.horizontal(|ui| {
            let bios = if ui.button("Start").clicked() {
                config.bios
                    .take_bios()
                    .or_else(|| {
                        config.show_bios_menu();
                        self.error = Some(
                            "A BIOS must be loaded to start the emulator".to_string()
                        );
                        None
                    })
            } else {
                None
            };

            if let Some(err) = &self.error {
                ui.label(err);
            }

            bios
        })
        .inner
    }

    pub fn show_area(
        &mut self,
        config: &mut Config,
        controllers: &mut Controllers,
        disc: &mut Disc,
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, Button)>,
        ctx: &egui::Context,
    ) -> Option<Bios> {
        egui::CentralPanel::default().show(ctx, |ui| {
            let space = ui.available_size() / 32.0;
            ui.allocate_space(space);
            ui.vertical_centered_justified(|ui| {
                ui.label(egui::WidgetText::RichText(
                    egui::RichText::new("Spillestation")
                        .text_style(egui::TextStyle::Heading)
                        .color(egui::Color32::BLACK)
                ));
            });
            ui.allocate_space(space);
            self.show_settings(config, controllers, disc, key_map, ui)
        })
        .inner
    }
}
