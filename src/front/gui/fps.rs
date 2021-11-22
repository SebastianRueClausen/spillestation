use std::time::{Instant, Duration};
use super::app::App;

/// How many times the counter updates a second.
const UPDATE_RATE: u64 = 10;

/// Duration between updates.
const UPDATE_INTERVAL: Duration = Duration::from_millis(UPDATE_RATE * 10);

/// Simple FPS counter.
pub struct FrameCounter {
    /// Last time the it was updated.
    last: Instant,
    /// How many frams since last update.
    frames: u64,
    /// The number being displayed on screen.
    display: u64,
}

impl App for FrameCounter {
    fn update(&mut self, ui: &mut egui::Ui) {
        self.frames += 1; 
        let now = Instant::now();
        if now.duration_since(self.last) > UPDATE_INTERVAL {
            self.last = now;
            self.display = self.frames * UPDATE_RATE;
            self.frames = 0;
        };
        ui.label("Frames Per Second");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x /= 2.0;
            ui.label(format!("{}", self.display));
        });
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Frame Rate")
            .open(open)
            .show(ctx, |ui| self.update(ui));
         
    }
}

impl FrameCounter {
    pub fn new() -> Self {
        Self {
            last: Instant::now(),
            frames: 0,
            display: 0,
        }
    }
}
