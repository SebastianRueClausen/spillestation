use crate::system::System;
use std::time::Duration;

/// Egui App/Window.
pub trait App {
    /// Show the app as a window.
    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool);

    /// Show the app on any given ['egui::Ui']. This can be used to display the app inline in
    /// another GUI. This is most likely called by 'show' onto an empty window.
    fn show(&mut self, ui: &mut egui::Ui);

    /// Called every frame.
    fn frame_tick(&mut self, _: Duration) { }

    /// Called every update.
    fn update_tick(&mut self,_dt: Duration, _: &mut System) { }

    fn name(&self) -> &'static str;
} 
