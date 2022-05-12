use super::DebugApp;

use splst_core::timer::TimerId;
use splst_core::System;

#[derive(Default)]
pub struct TimerView;

impl DebugApp for TimerView {
    fn name(&self) -> &'static str {
        "Timer View"
    }

    fn show(&mut self, system: &mut System, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for id in [TimerId::Tmr0, TimerId::Tmr1, TimerId::Tmr2].iter() {
                egui::CollapsingHeader::new(format!("{id}")).show(ui, |ui| {
                    let timer = system.timers().timer(*id);
                    egui::Grid::new(format!("grid_{id}")).show(ui, |ui| {
                        ui.label("counter");
                        ui.label(format!("{}", timer.counter));
                        ui.end_row();

                        ui.label("target");
                        ui.label(format!("{}", timer.target));
                        ui.end_row();

                        ui.label("sync enabled");
                        ui.label(format!("{}", timer.mode.sync_enabled()));
                        ui.end_row();

                        ui.label("sync mode");
                        ui.label(format!("{}", timer.mode.sync_mode(*id)));
                        ui.end_row();

                        ui.label("reset on target");
                        ui.label(format!("{}", timer.mode.reset_on_target()));
                        ui.end_row();

                        ui.label("irq on target");
                        ui.label(format!("{}", timer.mode.irq_on_target()));
                        ui.end_row();

                        ui.label("irq on overflow");
                        ui.label(format!("{}", timer.mode.irq_on_overflow()));
                        ui.end_row();

                        ui.label("irq repeat");
                        ui.label(format!("{}", timer.mode.irq_repeat()));
                        ui.end_row();

                        ui.label("irq toggle mode");
                        ui.label(format!("{}", timer.mode.irq_toggle_mode()));
                        ui.end_row();

                        ui.label("clock source");
                        ui.label(format!("{}", timer.mode.clock_source(*id)));
                        ui.end_row();

                        ui.label("master irq flag");
                        ui.label(format!("{}", timer.mode.master_irq_flag()));
                        ui.end_row();

                        ui.label("target reached");
                        ui.label(format!("{}", timer.mode.target_reached()));
                        ui.end_row();

                        ui.label("overflow reached");
                        ui.label(format!("{}", timer.mode.overflow_reached()));
                        ui.end_row();
                    });
                });
            }
        });
    }

    fn show_window(&mut self, system: &mut System, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("Timer View")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| self.show(system, ui));
    }
}
