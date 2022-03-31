use splst_core::System;
use super::DebugApp;

use std::fmt::Write;
use std::time::Duration;

#[derive(Default)]
pub struct ScheduleView {
    /// The amount of cycles since startup.
    cycles: String,
    /// The duration since startup.
    run_time: String,
    /// Show the kind of event, the repeat mode and the when it will 
    events: Vec<(String, String, String)>,
}

impl DebugApp for ScheduleView {
    fn name(&self) -> &'static str {
        "Schedule View"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System) {
        let now = system.schedule().since_startup();

        self.cycles.clear();
        self.run_time.clear();

        let (mins, secs, millis) = {
            let dur = now.as_duration();
            (dur.as_secs() / 60, dur.as_secs() % 60, dur.subsec_millis())
        };

        write!(&mut self.run_time, "{}.{}.{}", mins, secs, millis).unwrap();
        write!(&mut self.cycles, "{}", now.as_cpu_cycles()).unwrap();

        self.events = system.schedule()
            .iter_event_entries()
            .map(|entry| {
                let cycles_until = entry.ready
                    .saturating_sub(now)
                    .as_cpu_cycles()
                    .to_string();
                (cycles_until, format!("{}", entry.mode), format!("{}", entry.event))
            })
            .collect();
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("time_grid").show(ui, |ui| {
            ui.label("CPU Cycles");
            ui.label(&self.cycles);
            ui.end_row();
            ui.label("Run Time");
            ui.label(&self.run_time);
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("schdule_grid").show(ui, |ui| {
                    ui.label("Ready");
                    ui.label("Repeat Mode");
                    ui.label("Event");
                    ui.end_row();
                    for (ready, mode, event) in &self.events {
                        ui.label(ready);
                        ui.label(mode);
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
