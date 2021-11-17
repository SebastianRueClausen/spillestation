use egui::CtxRef;
use std::time::{Instant, Duration};

/// How many times the counter updates a second.
const FRAME_COUNTER_UPDATE: u64 = 10;

pub struct FrameCounter {
    last: Instant,
    frames: u64,
    display: u64,
}

impl FrameCounter {
    fn new() -> Self {
        Self {
            last: Instant::now(),
            frames: 0,
            display: 0,
        }
    }

    fn tick(&mut self) {
        self.frames += 1; 
    }

    fn fps(&mut self) -> u64 {
        let now = Instant::now();
        if now.duration_since(self.last) > Duration::from_millis(FRAME_COUNTER_UPDATE * 10) {
            self.last = now;
            self.display = self.frames * FRAME_COUNTER_UPDATE;
            self.frames = 0;
        }
        self.display
    }
}

pub struct Ui {
    show: bool,
    frame_counter: FrameCounter,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            show: true,
            frame_counter: FrameCounter::new(),
        }
    }

    pub fn update(&mut self, ctx: &CtxRef) {
        self.frame_counter.tick();
        let fps = self.frame_counter.fps();
        egui::Window::new("Frame Rate")
            .open(&mut self.show)
            .show(ctx, |ui| {
                ui.label("FPS");
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x /= 2.0;
                    ui.label(format!("{}", fps));
                });
            });
    }
}
