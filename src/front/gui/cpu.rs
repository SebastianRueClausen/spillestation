use super::App;
use crate::{cpu::{Cpu, REGISTER_NAMES}, system::System};
use std::fmt::Write;
use std::time::Duration;

/// ['App'] which shows the status of the CPU. It shows the value of all the registers and PC and
/// such.
#[derive(Default)]
pub struct CpuStatus {
    registers: [String; 32],
    /// Fields which aren't a registers, such as the PC and the disassembled instruction current being
    /// run.
    fields: [String; 4],
}

impl CpuStatus {
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
}

impl App for CpuStatus {
    fn name(&self) -> &'static str {
        "CPU Status"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System) {
        self.fields
            .iter_mut()
            .for_each(|field| field.clear());
        self.registers
            .iter_mut()
            .for_each(|register| register.clear());
        if let Err(err) = self.write_fields(&mut system.cpu) {
            eprintln!("{}", err);
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
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

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("CPU Status")
            .open(open)
            .resizable(true)
            .min_width(120.0)
            .default_width(240.0)
            .default_height(240.0)
            .show(ctx, |ui| {
                self.show(ui);
            });
    }
}

#[derive(PartialEq, Eq)]
enum RunMode {
    Step,
    Run,
}

/// ['App'] for controlling the ['System'] when it's in debug mode. It has two differnent modes.
///  - Run - Automatically runs the CPU at a given speed.
///  - Step - Manually step through each cycle.
pub struct CpuCtrl {
    /// Cycles per second in run mode.
    cycle_hz: u64,
    /// The amount of cycles which is run each time step.
    step_amount: u64,
    /// If the step button has been pressed.
    stepped: bool,
    /// Paused aka. in step mode.
    mode: RunMode,
    /// This is used to run at a more precise HZ. It's also required to run the
    /// CPU at a lower Hz than the update rate, since there may be multiple
    /// updates between each CPU cycle.
    remainder: Duration,
}

impl Default for CpuCtrl {
    fn default() -> Self {
        Self {
            cycle_hz: MAX_CYCLE_HZ,
            step_amount: 1,
            mode: RunMode::Run,
            stepped: false,
            remainder: Duration::ZERO,
        }
    }
}

impl App for CpuCtrl {
    fn name(&self) -> &'static str {
        "CPU Control"
    }

    fn update_tick(&mut self, dt: Duration, system: &mut System) {
        match self.mode {
            RunMode::Step if self.stepped => {
                system.step_debug(self.step_amount);
            }
            RunMode::Run  => {
                self.remainder = system.run_debug(self.cycle_hz, self.remainder + dt);
            }
            _ => {},
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.mode,
                RunMode::Step,
                "Step"
            );
            ui.radio_value(
                &mut self.mode,
                RunMode::Run,
                "Run"
            );
        });
        ui.separator();
        match self.mode {
            RunMode::Step => {
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
                self.stepped = ui.button("Step").clicked();
            }
            RunMode::Run => {
                ui.add(
                    egui::Slider::new(&mut self.cycle_hz, 1..=MAX_CYCLE_HZ)
                        .suffix("Hz")
                        .logarithmic(true)
                        .clamp_to_range(true)
                        .smart_aim(true)
                        .text("CPU Speed"),
                );
            }
        }
    }

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("CPU Control")
            .open(open)
            .resizable(true)
            .min_width(80.0)
            .default_width(100.0)
            .default_height(100.0)
            .show(ctx, |ui| {
                self.show(ui);
            });
    }
}

const FIELD_LABELS: [&str; 4] = ["hi", "lo", "pc", "ins"];

/// This is the native speed the Playstation.
const MAX_CYCLE_HZ: u64 = 30_000_000;
