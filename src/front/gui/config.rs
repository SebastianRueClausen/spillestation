use super::app::App;
use crate::front::config::Config;

/// ['App'] for setting up configs.
pub struct Configurator {
    pub config: Config,
    pub try_load_bios: bool,
    pub bios_error: Option<String>,
}

impl Configurator {
    pub fn new() -> Self {
        Self {
            config: Default::default(),
            try_load_bios: false,
            bios_error: None,
        }
    }
}

impl App for Configurator {
    fn update(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("BIOS File");
            ui.add(egui::TextEdit::singleline(&mut self.config.bios).hint_text("Path"));
        });
        if ui.button("Load BIOS").clicked() {
            self.try_load_bios = true;
        }
        if let Some(ref err) = self.bios_error {
            ui.label(err);
        }
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Config")
            .open(open)
            .show(ctx, |ui| self.update(ui));
    }
    
}
