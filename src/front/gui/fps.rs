use std::time::{Instant, Duration};
use std::fmt::Write;
use super::app::App;

const ALPHA: f64 = 0.7;

pub struct FrameCounter {
    average: f64,
    frames: u64,
    last: Instant,
    show: String,
}

impl FrameCounter {
    pub fn new() -> Self {
        Self {
            average: 60.0,
            frames: 0,
            last: Instant::now(),
            show: String::from(""),
        }
    }
}

impl App for FrameCounter {
    fn update(&mut self, ui: &mut egui::Ui) {
        self.frames += 1;
        let now = Instant::now();
        if now.duration_since(self.last) > Duration::from_secs(1) {
            self.last = now;
            self.average = ALPHA * self.average + (1.0 - ALPHA) * self.frames as f64;
            self.show.clear();
            write!(&mut self.show, "{:.2}", self.average).unwrap();
            self.frames = 0;
        };
        ui.label("Frames Per Second");
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
