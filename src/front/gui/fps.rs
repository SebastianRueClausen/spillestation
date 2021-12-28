//! Simple FPS counter GUI app.

use super::app::App;
use std::fmt::Write;
use std::time::Duration;

/// ['App'] for displaying the current frames per second.
pub struct FrameCounter {
    /// Frames since the last update.
    frames: u64,
    /// Duration since the last update.
    last_update: Duration,
    /// The current FPS being displayed.
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

impl Default for FrameCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl App for FrameCounter {
    fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x /= 2.0;
            ui.label(&self.show);
        });
    }

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Frame Rate")
            .open(open)
            .show(ctx, |ui| self.show(ui));
    }
}
