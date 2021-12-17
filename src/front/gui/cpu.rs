use super::App;
/// GUI Apps to view and interact with the Playstations CPU.
use crate::cpu::{Cpu, REGISTER_NAMES};
use std::fmt::Write;
use std::time::Duration;

/// ['App'] which shows the status of the CPU. It shows the value of all the registers and PC and
/// such.
pub struct CpuStatus {
    registers: [String; 32],
    /// Fields which aren't a registers, such as the PC and the disassembled instruction current being
    /// run.
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
        self.registers
            .iter_mut()
            .for_each(|register| register.clear());
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
                    egui::Grid::new("cpu_status_grid").show(ui, |ui| {
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
                    egui::Grid::new("cpu_register_grid").show(ui, |ui| {
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

/// ['App'] for controlling the CPU. It has two modes:
///  - Run. Automatically runs the CPU at a given speed.
///  - Step. Manually step through each cycle.
pub struct CpuCtrl {
    /// Cycles per second in run mode.
    cycle_hz: usize,
    /// The amount of cycles which is run each time step.
    step_amount: usize,
    /// Paused aka. in step mode.
    paused: bool,
    /// If the step button has been pressed since last update.
    stepped: bool,
    /// This is used to run at a more precise HZ. It's also required to run the CPU at a lower HZ
    /// than the update rate, since there may be multiple updates between each CPU cycle.
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
            ui.add(
                egui::Slider::new(&mut self.step_amount, 1..=MAX_CYCLE_HZ)
                    .suffix(suffix)
                    .logarithmic(true)
                    .clamp_to_range(true)
                    .smart_aim(true)
                    .text("Step amount"),
            );
            if ui.button("Step").clicked() {
                self.stepped = true;
            }
        } else {
            ui.add(
                egui::Slider::new(&mut self.cycle_hz, 1..=MAX_CYCLE_HZ)
                    .suffix("hz")
                    .logarithmic(true)
                    .clamp_to_range(true)
                    .smart_aim(true)
                    .text("CPU Speed"),
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

const FIELD_LABELS: [&str; 4] = ["hi", "lo", "pc", "ins"];

/// This is the native speed the Playstation.
const MAX_CYCLE_HZ: usize = 30_000_000;
