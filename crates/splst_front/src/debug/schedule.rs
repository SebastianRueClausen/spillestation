use splst_core::System;
use super::DebugApp;

#[derive(Default)]
pub struct ScheduleView;

impl DebugApp for ScheduleView {
    fn name(&self) -> &'static str {
        "Schedule"
    }

    fn show(&mut self, system: &mut System, ui: &mut egui::Ui) {
        let now = system.schedule().now().time_since_startup();

        egui::Grid::new("time_grid").show(ui, |ui| {
            ui.label("CPU Cycles");
            ui.label(format!("{}", now.as_cpu_cycles()));
            ui.end_row();

            let (mins, secs, millis) = {
                let dur = now.as_duration();
                (dur.as_secs() / 60, dur.as_secs() % 60, dur.subsec_millis())
            };

            ui.label("Run Time");
            ui.label(format!("{mins},{secs}.{millis}"));
            ui.end_row();
        });

        ui.separator();

        let mut events: Vec<_> = system.schedule()
            .iter_event_entries()
            .map(|entry| {
                let cycles_until = entry.ready
                    .time_since_startup()
                    .saturating_sub(now)
                    .as_cpu_cycles()
                    .to_string();
                (cycles_until, format!("{}", entry.mode), format!("{}", entry.event))
            })
            .collect::<Vec<_>>();
        
        events.sort();

        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("schdule_grid").show(ui, |ui| {
                    ui.label("Ready");
                    ui.label("Repeat Mode");
                    ui.label("Event");
                    ui.end_row();
                    for (ready, mode, event) in events.into_iter() {
                        ui.label(ready);
                        ui.label(mode);
                        ui.label(event);
                        ui.end_row();
                    }
                });
            });
    }

    fn show_window(&mut self, system: &mut System, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("Schedule View")
            .open(open)
            .resizable(true)
            .default_width(180.0)
            .default_height(480.0)
            .show(ctx, |ui| self.show(system, ui));
    }
}
