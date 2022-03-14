use super::DebugApp;

use splst_core::timer::{Timers, TimerId};
use splst_core::System;

use std::fmt::Write;
use std::time::Duration;

#[derive(Default)]
pub struct TimerView {
    fields: [[String; 13]; 3],  
}

impl TimerView {
    pub fn write_fields(&mut self, timers: &Timers) -> Result<(), std::fmt::Error> {
        let timer_ids = [TimerId::Tmr0, TimerId::Tmr1, TimerId::Tmr2];
        for (id, fields) in timer_ids.iter().zip(self.fields.iter_mut()) {
            let tmr = timers.timer(*id);
            write!(fields[0], "{}", tmr.counter)?;
            write!(fields[1], "{}", tmr.target)?;
            write!(fields[2], "{}", tmr.mode.sync_enabled())?;
            write!(fields[3], "{}", tmr.mode.sync_mode(*id))?;
            write!(fields[4], "{}", tmr.mode.reset_on_target())?;
            write!(fields[5], "{}", tmr.mode.irq_on_target())?;
            write!(fields[6], "{}", tmr.mode.irq_on_overflow())?;
            write!(fields[7], "{}", tmr.mode.irq_repeat())?;
            write!(fields[8], "{}", tmr.mode.irq_toggle_mode())?;
            write!(fields[9], "{}", tmr.mode.clock_source(*id))?;
            write!(fields[10], "{}", tmr.mode.master_irq_flag())?;
            write!(fields[11], "{}", tmr.mode.target_reached())?;
            write!(fields[12], "{}", tmr.mode.overflow_reached())?;
        }
        Ok(())
    }
}

impl DebugApp for TimerView {
    fn name(&self) -> &'static str {
        "Timer View"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System) {
        self.fields.iter_mut().for_each(|fields|
            fields.iter_mut().for_each(|field| field.clear())
        );
        if let Err(err) = self.write_fields(system.timers()) {
            eprintln!("{}", err);
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (timer, (header, grid)) in self.fields.iter().zip(UI_IDS) {
                egui::CollapsingHeader::new(header).show(ui, |ui| {
                    egui::Grid::new(grid).show(ui, |ui| {
                        for (field, label) in timer.iter().zip(FIELD_LABELS) {
                            ui.label(label);
                            ui.label(field);
                            ui.end_row();
                        }
                    });
                });
            }
        });
    }

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Timer View")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                self.show(ui);
            });
    }
}

const FIELD_LABELS: [&str; 13] = [
    "counter",
    "target",
    "sync enabled",
    "sync mode",
    "reset on target",
    "irq on target",
    "irq on overflow",
    "irq repeat",
    "irq toggle mode",
    "clock source",
    "master irq flag",
    "target reached",
    "overflow reached",
];

const UI_IDS: [(&str, &str); 3] = [
    ("TMR0", "tmr0_grid"),
    ("TMR1", "tmr1_grid"),
    ("TMR2", "tmr2_grid"),
];
