
pub mod menu;
pub mod cpu;
pub mod fps;
pub mod gpu;
pub mod irq;
pub mod mem;
pub mod timer;
pub mod vram;
pub mod schedule;
mod io_port;

use splst_core::System;

use std::time::Duration;

pub use menu::DebugMenu;

/// Egui Debug App/Window.
pub trait DebugApp {
    /// Show the app as a window.
    fn show_window(&mut self, system: &mut System, ctx: &egui::Context, open: &mut bool);

    // Show the app inside a UI.
    fn show(&mut self, system: &mut System, ui: &mut egui::Ui);

    /// Called every frame.
    fn frame_tick(&mut self, _: Duration) {}

    /// Called every update.
    fn update_tick(&mut self, _dt: Duration, _: &mut System) {}

    fn name(&self) -> &'static str;
} 
