use super::App;
use crate::{cpu::Cpu, timer::Timers};
use std::fmt::Write;

pub struct TimerView {
    fields: [[String; 13]; 3],  
}

impl TimerView {
    pub fn new() -> Self {
        Self {
            fields: Default::default(),
        }
    }

    pub fn write_fields(&mut self, timers: &Timers) -> Result<(), std::fmt::Error> {
        Ok(for (timer, fields) in timers.timers.iter().zip(self.fields.iter_mut()) {
            write!(fields[0], "{}", timer.counter)?;
            write!(fields[1], "{}", timer.target)?;
            write!(fields[2], "{}", timer.mode.sync_enabled())?;
            write!(fields[3], "{}", timer.mode.sync_mode(timer.id))?;
            write!(fields[4], "{}", timer.mode.reset_on_target())?;
            write!(fields[5], "{}", timer.mode.irq_on_target())?;
            write!(fields[6], "{}", timer.mode.irq_on_overflow())?;
            write!(fields[7], "{}", timer.mode.irq_repeat())?;
            write!(fields[8], "{}", timer.mode.irq_toggle_mode())?;
            write!(fields[9], "{}", timer.mode.clock_source(timer.id))?;
            write!(fields[10], "{}", timer.mode.master_irq_flag())?;
            write!(fields[11], "{}", timer.mode.target_reached())?;
            write!(fields[12], "{}", timer.mode.overflow_reached())?;
        })
    }

    pub fn update_fields(&mut self, cpu: &Cpu) {
        self.fields.iter_mut().for_each(|fields|
            fields.iter_mut().for_each(|field| field.clear())
        );
        if let Err(err) = self.write_fields(cpu.bus().timers()) {
            eprintln!("{}", err);
        }
    }
}

impl Default for TimerView {
    fn default() -> Self {
        Self::new()
    }
}

impl App for TimerView {
    fn update(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for ((header, grid), timer) in UI_IDS.iter().zip(self.fields.iter()) {
                egui::CollapsingHeader::new(header).show(ui, |ui| {
                    egui::Grid::new(grid).show(ui, |ui| {
                        for (field, label) in timer.iter().zip(FIELD_LABELS.iter()) {
                            ui.label(label);
                            ui.label(&field);
                            ui.end_row();
                        }
                    });
                });
            }
        });
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Timer View")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                self.update(ui);
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
