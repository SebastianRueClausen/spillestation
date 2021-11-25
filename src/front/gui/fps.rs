use std::time::Duration;
use std::fmt::Write;
use super::app::App;

pub struct FrameCounter {
    frames: u64,
    last_update: Duration,
    show: String,
}

impl FrameCounter {
    pub fn new() -> Self {
        Self {
            frames: 0,
            last_update: Duration::ZERO,
            show: String::from(""),
        }
    }

    pub fn tick(&mut self, dt: Duration) {
        self.frames += 1;
        self.last_update += dt;
        if self.last_update > Duration::from_secs(1) {
            self.show.clear();
            self.last_update = Duration::ZERO;
            write!(&mut self.show, "{} fps", self.frames).unwrap();
            self.frames = 0;
        }
    }
}

impl App for FrameCounter {
    fn update(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x /= 2.0;
            ui.label(&self.show);
        });
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Frame Rate")
            .open(open)
            .show(ctx, |ui| self.update(ui));
    }
    
}
