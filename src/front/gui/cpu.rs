use crate::cpu::{Cpu, REGISTER_NAMES};
use std::fmt::Write;
use std::time::Duration;
use super::App;

pub struct CpuStatus {
    registers: [String; 32],
    fields: [String; 4],
}

impl CpuStatus {
    pub fn new() -> Self {
        Self {
            registers: Default::default(),
            fields: Default::default(),
        }
    }

    pub fn write_fields(&mut self, cpu: &mut Cpu) -> Result<(), std::fmt::Error> {
        for (show, value) in self.registers.iter_mut().zip(cpu.registers.iter()) {
            write!(show, "{}", value)?;
        }
        write!(&mut self.fields[0], "{:08x}", cpu.hi)?;
        write!(&mut self.fields[1], "{:08x}", cpu.lo)?;
        write!(&mut self.fields[2], "{:08x}", cpu.pc)?;
        write!(&mut self.fields[3], "{}", cpu.current_instruction())?;
        Ok(()) 
    }

    pub fn update_fields(&mut self, cpu: &mut Cpu) {
        self.fields.iter_mut().for_each(|field| field.clear());
        self.registers.iter_mut().for_each(|register| register.clear());
        if let Err(err) = self.write_fields(cpu) {
            eprintln!("{}", err);
        }
    }
}

impl Default for CpuStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl App for CpuStatus {
    fn update(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Status", |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .max_height(200.0)
                .show(ui, |ui| {
                    egui::Grid::new("Status Grid").show(ui, |ui| {
                        for (field, label) in self.fields.iter_mut().zip(FIELD_LABELS.iter()) {
                            ui.label(label); 
                            ui.label(field); 
                            ui.end_row();
                        }
                    });
                });
        });
        ui.collapsing("Registers", |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .max_height(480.0)
                .show(ui, |ui| {
                    egui::Grid::new("Register Grid").show(ui, |ui| {
                        for (value, name) in self.registers.iter().zip(REGISTER_NAMES.iter()) {
                            ui.label(name);
                            ui.label(value);
                            ui.end_row();
                        }
                    });
                });
        });
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("CPU Status")
            .open(open)
            .resizable(true)
            .min_width(120.0)
            .default_width(240.0)
            .default_height(240.0)
            .show(ctx, |ui| {
                self.update(ui);
            });
    }
}

pub struct CpuCtrl {
    cycle_hz: usize,
    step_amount: usize,
    paused: bool,
    stepped: bool,
    remainder: Duration,
}

impl CpuCtrl {
    pub fn new() -> Self {
        Self {
            cycle_hz: MAX_CYCLE_HZ,
            step_amount: 1,
            paused: false,
            stepped: false,
            remainder: Duration::ZERO,
        }
    }

    pub fn run_cpu(&mut self, mut dt: Duration, cpu: &mut Cpu) {
        if !self.paused {
            dt += self.remainder;
            let cycle_time = Duration::from_secs(1) / self.cycle_hz as u32;
            while let Some(new) = dt.checked_sub(cycle_time) {
                dt = new;
                cpu.fetch_and_exec();
            }
            self.remainder = dt;
        } else if self.stepped {
            for _ in 0..self.step_amount {
                cpu.fetch_and_exec();
            }
            self.stepped = false;
        }
    }
}

impl Default for CpuCtrl {
    fn default() -> Self {
        Self::new()
    }
}

impl App for CpuCtrl {
    fn update(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.paused, true, "Step"); 
            ui.radio_value(&mut self.paused, false, "Run"); 
        });
        ui.separator();
        if self.paused {
            let suffix = if self.step_amount > 1 {
                " cycles"
            } else {
                " cycle"
            };
            ui.add(egui::Slider::new(&mut self.step_amount, 1..=MAX_CYCLE_HZ)
                .suffix(suffix)
                .logarithmic(true)
                .clamp_to_range(true)
                .smart_aim(true)
                .text("Step amount")
            );
            if ui.button("Step").clicked() {
                self.stepped = true;
            }
        } else {
            ui.add(egui::Slider::new(&mut self.cycle_hz, 1..=MAX_CYCLE_HZ)
                .suffix("hz")
                .logarithmic(true)
                .clamp_to_range(true)
                .smart_aim(true)
                .text("CPU Speed")
            );
        }
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("CPU Control")
            .open(open)
            .resizable(true)
            .min_width(80.0)
            .default_width(100.0)
            .default_height(100.0)
            .show(ctx, |ui| {
                self.update(ui);
            });
    }
}

const FIELD_LABELS: [&'static str; 4] = [
   "hi",
   "lo",
   "pc",
   "ins",
];

const MAX_CYCLE_HZ: usize = 30_000_000;
