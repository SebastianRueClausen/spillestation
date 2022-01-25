use super::App;

use crate::cpu::{Cpu, REGISTER_NAMES};
use crate::system::{System, DebugStop, Breaks};
use crate::render::Renderer;
use crate::timing::CPU_HZ;

use std::fmt::Write;
use std::time::Duration;

/// ['App'] to shows the status of the CPU. It shows the value of all the registers and PC and
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
        write!(&mut self.fields[3], "{}", cpu.curr_ins())?;
        Ok(())
    }
}

impl App for CpuStatus {
    fn name(&self) -> &'static str {
        "CPU Status"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System, _: &Renderer) {
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
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.collapsing("Status", |ui| {
                    egui::Grid::new("cpu_status_grid").show(ui, |ui| {
                        for (field, label) in self.fields.iter().zip(FIELD_LABELS) {
                            ui.label(label);
                            ui.label(field);
                            ui.end_row();
                        }
                    });
                });
                ui.collapsing("Registers", |ui| {
                    egui::Grid::new("cpu_register_grid").show(ui, |ui| {
                        for (value, name) in self.registers.iter().zip(REGISTER_NAMES) {
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
    Step {
        /// The amount of cycles each step.
        amount: u64,
        /// If the step button has been pressed.
        stepped: bool,
    },
    Run {
        /// The speed of which the run the system, in CPU cycles per second.
        speed: u64,
        /// This is used to run at a more precise HZ. It's also required to run the
        /// CPU at a lower Hz than the update rate, since there may be multiple
        /// updates between each CPU cycle.
        remainder: Duration,
    },
}

impl RunMode {
    fn default_step() -> Self {
        RunMode::Step { amount: 1, stepped: false }
    }

    fn default_run() -> Self {
        RunMode::Run { speed: 1, remainder: Duration::ZERO }
    }
}

impl Default for RunMode {
    fn default() -> Self {
        Self::default_step()
    }
}

#[derive(Default)]
struct Breakpoints {
    addrs: Vec<u32>,
    names: Vec<String>,
}

impl Breakpoints {
    fn push(&mut self, addr: u32, name: String) {
        self.addrs.push(addr);
        self.names.push(name);
    }
}

/// ['App'] for controlling the ['System'] when it's in debug mode. It has two differnent modes.
///  - Run - Automatically runs the CPU at a given speed.
///  - Step - Manually step through each cycle.
#[derive(Default)]
pub struct CpuCtrl {
    mode: RunMode,
    break_message: Option<String>,
    code_bps: Breakpoints,
    bp_add: String,
}

impl CpuCtrl {
    /// Show the breakpoint section.
    fn show_breakpoint(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("breakpoint_grid").show(ui, |ui| {
                let input = egui::TextEdit::singleline(&mut self.bp_add);
                ui.add_sized([120.0, 20.0], input);
                let add = ui.button("Add").clicked();
                if ui.input().key_pressed(egui::Key::Enter) || add {
                    // Parse the string as address in hex.
                    if let Ok(addr) = u32::from_str_radix(&self.bp_add, 16) {
                        self.code_bps.push(addr, self.bp_add.clone());
                        self.bp_add.clear();
                    } else {
                        self.break_message =
                            Some(format!("Invalid Breakpoint Address: {}", self.bp_add));
                    }
                }
                ui.end_row();

                // Check if any breakpoints should be removed.
                let mut index = 0;
                self.code_bps.names.retain(|bp| {
                    ui.label(bp); 
                    let retain = if ui.button("Remove").clicked() {
                        self.code_bps.addrs.remove(index);
                        false
                    } else {
                        index += 1;
                        true
                    };
                    ui.end_row();
                    retain
                });
            });
        });
    }
}

impl App for CpuCtrl {
    fn name(&self) -> &'static str {
        "CPU Control"
    }

    fn update_tick(&mut self, dt: Duration, sys: &mut System, renderer: &Renderer) {
        let stop = match self.mode {
            RunMode::Step { ref mut stepped, amount } => {
                if *stepped {
                    *stepped = false;
                    self.break_message = None;
                    sys.step_debug(amount, renderer, Breaks {
                        code: self.code_bps.addrs.as_slice(),
                        store: &[],
                        load: &[],
                    })
                } else {
                    DebugStop::Time 
                }
            }
            RunMode::Run { speed, ref mut remainder } => {
                self.break_message = None;
                let time = *remainder + dt;
                let (rem, stop) = sys.run_debug(speed, time, renderer, Breaks {
                    code: self.code_bps.addrs.as_slice(),
                    store: &[],
                    load: &[],
                });
                // Clear the break message if running, since the user must have clicked run again
                // after a breakpoint.
                *remainder = rem;
                stop
            }
        };
        if let DebugStop::Breakpoint(addr) = stop {
            self.mode = RunMode::default_step();
            self.break_message = Some(
                format!("Stopped at breakpoint on address: {:08x}", addr)
            );
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        let was_step = matches!(self.mode, RunMode::Step { .. });
        let mut is_step = was_step;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut is_step, true, "Step");
            ui.selectable_value(&mut is_step, false, "Run");
        });
        if was_step != is_step {
            if is_step {
                self.mode = RunMode::default_step();
            } else {
                self.mode = RunMode::default_run();
            }
        }
        ui.separator();
        match self.mode {
            RunMode::Step { ref mut amount, ref mut stepped } => {
                let suffix = match amount {
                    0 | 2.. => " cycles",
                    1 => " cycle",
                };
                ui.add(egui::Slider::new(amount, 1..=CPU_HZ)
                    .suffix(suffix)
                    .logarithmic(true)
                    .clamp_to_range(true)
                    .smart_aim(true)
                    .text("Step Amount")
                );
                *stepped = ui.button("Step").clicked();
            }
            RunMode::Run { ref mut speed, ..  } => {
                ui.add(egui::Slider::new(speed, 1..=CPU_HZ)
                    .suffix("Hz")
                    .logarithmic(true)
                    .clamp_to_range(true)
                    .smart_aim(true)
                    .text("CPU Speed")
                );
            }
        }
        ui.separator();
        if let Some(ref msg) = self.break_message {
            ui.label(msg);
        }
        ui.collapsing("Breakpoints", |ui| {
            self.show_breakpoint(ui);
        });
    }

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("CPU Control")
            .open(open)
            .resizable(true)
            .default_width(100.0)
            .default_height(300.0)
            .show(ctx, |ui| {
                self.show(ui);
            });
    }
}

const FIELD_LABELS: [&str; 4] = ["hi", "lo", "pc", "ins"];
