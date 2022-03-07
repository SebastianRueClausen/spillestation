use splst_core::Bios;
use super::config::Config;

pub struct StartMenu {
    error: Option<String>,
}

impl StartMenu {
    pub fn new() -> Self {
        Self { error: None }
    }

    fn show_settings(&mut self, config: &mut Config, ui: &mut egui::Ui) -> Option<Bios> {
        ui.group(|ui| {
            config.show_inside(None, ui);
        });

        ui.horizontal(|ui| {
            if ui.button("Start").clicked() {
                config.bios
                    .take_bios()
                    .or_else(|| {
                        self.error = Some(
                            "A BIOS must be loaded to start the emulator".to_string()
                        );
                        None
                    })
            } else {
                None
            }
        })
        .inner
    }

    pub fn show_area(
        &mut self,
        config: &mut Config,
        ctx: &egui::CtxRef
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
            self.show_settings(config, ui)
        })
        .inner
    }
}
