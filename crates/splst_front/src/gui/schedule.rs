use super::App;

use splst_core::System;
use crate::render::Renderer;

use std::fmt::Write;
use std::time::Duration;

#[derive(Default)]
pub struct ScheduleView {
    cycle: String,
    events: Vec<(String, String)>,
}
impl App for ScheduleView {
    fn name(&self) -> &'static str {
        "Schedule View"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System, _: &mut Renderer) {
        let now = system.cpu.bus().schedule.cycle();

        self.cycle.clear();
        write!(&mut self.cycle, "cycle: {}", now).unwrap();

        self.events = system.cpu.bus().schedule
            .iter()
            .map(|entry| {
                (entry.0.saturating_sub(now).to_string(), format!("{}", entry.1))
            })
            .collect();
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        ui.label(&self.cycle);
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("schdule_grid").show(ui, |ui| {
                    ui.strong("execute");
                    ui.strong("event");
                    ui.end_row();
                    for (cycle, event) in &self.events {
                        ui.label(cycle);
                        ui.label(event);
                        ui.end_row();
                    }
                });
            });
    }

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Schedule View")
            .open(open)
            .resizable(true)
            .default_width(180.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                self.show(ui);
            });
    }
}
