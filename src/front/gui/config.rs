use super::app::App;
use crate::front::config::Config;
use native_dialog::FileDialog;

/// ['App'] for setting up configs.
pub struct Configurator {
    pub config: Config,
    pub try_load_bios: bool,
    pub bios_message: Option<String>,
    /// This is displayed If the app is opened because loading the config file failed.
    pub config_error: Option<String>,
}

impl Configurator {
    pub fn new(
        config_error: Option<String>,
        bios_message: Option<String>,
    ) -> Self {
        Self {
            config: Default::default(),
            try_load_bios: false,
            bios_message,
            config_error,
        }
    }
}

impl App for Configurator {
    fn update(&mut self, ui: &mut egui::Ui) {
        // Show if something failed when loading config file.
        if let Some(ref err) = self.config_error {
            ui.label(err);
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("BIOS File");
            ui.add(egui::TextEdit::singleline(&mut self.config.bios)
                .hint_text("Path"));
            if ui.button("Open").clicked() {
                let path = FileDialog::new()
                    .set_location(".")
                    .show_open_single_file()
                    .unwrap_or(None);
                if let Some(path) = path {
                    self.config.bios = String::from(path.to_str().unwrap_or("Invalid path")); 
                }
            }
            if ui.button("Load BIOS").clicked() {
                self.try_load_bios = true;
            }
        });
        if let Some(ref err) = self.bios_message {
            ui.label(err);
        }
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Config")
            .open(open)
            .show(ctx, |ui| self.update(ui));
    }
    
}
