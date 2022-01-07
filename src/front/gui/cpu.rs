use super::App;
use crate::{cpu::{Cpu, REGISTER_NAMES}, system::{System, DebugStop}, timing};
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

impl Default for RunMode {
    fn default() -> Self {
        RunMode::Step { amount: 1, stepped: false }
    }
}

struct Breakpoint {
    addr: u32,
    name: String,
}

impl Breakpoint {
    fn new(addr: u32, name: String) -> Self {
        Self { addr, name }
    }
}

/// ['App'] for controlling the ['System'] when it's in debug mode. It has two differnent modes.
///  - Run - Automatically runs the CPU at a given speed.
///  - Step - Manually step through each cycle.
#[derive(Default)]
pub struct CpuCtrl {
    mode: RunMode,
    break_message: Option<String>,
    bp: Vec<Breakpoint>, 
    bp_add: String,
}

impl App for CpuCtrl {
    fn name(&self) -> &'static str {
        "CPU Control"
    }

    fn update_tick(&mut self, dt: Duration, sys: &mut System) {
        sys.dbg.breakpoints.clear();
        for bp in self.bp.iter() {
            sys.dbg.breakpoints.push(bp.addr); 
        }
        let stop = match self.mode {
            RunMode::Step { amount, stepped } if stepped => {
                self.break_message = None;
                sys.step_debug(amount)
            }
            RunMode::Run { speed, ref mut remainder } => {
                self.break_message = None;
                let (rem, stop) = sys.run_debug(speed, *remainder + dt);
                *remainder = rem;
                stop
            }
            _ => DebugStop::Time,
        };
        if let DebugStop::Breakpoint(addr) = stop {
            self.mode = RunMode::default();
            self.break_message = Some(
                format!("Stopped at breakpoint on address: {:08x}", addr)
            );
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        let mut step_mode = match self.mode {
            RunMode::Step { .. } => true,
            RunMode::Run { .. } => false,
        };
        ui.horizontal(|ui| {
            ui.selectable_value(&mut step_mode, true, "Step");
            ui.selectable_value(&mut step_mode, false, "Run");
        });
        match self.mode {
            RunMode::Step { .. } if !step_mode => {
                self.mode = RunMode::Run { speed: 1, remainder: Duration::ZERO }     
            }
            RunMode::Run { .. } if step_mode => {
                self.mode = RunMode::Step { amount: 1, stepped: false }
            }
            _ => (),
        }
        ui.separator();
        match self.mode {
            RunMode::Step { ref mut amount, ref mut stepped } => {
                let suffix = match amount {
                    0 | 2.. => " cycles",
                    1 => " cycle",
                };
                let slider = egui::Slider::new(amount, 1..=timing::CPU_HZ)
                    .suffix(suffix)
                    .logarithmic(true)
                    .clamp_to_range(true)
                    .smart_aim(true)
                    .text("Step amount");
                ui.add(slider);
                *stepped = ui.button("Step").clicked();
            }
            RunMode::Run { ref mut speed, ..  } => {
                let slider = egui::Slider::new(speed, 1..=timing::CPU_HZ)
                    .suffix("Hz")
                    .logarithmic(true)
                    .clamp_to_range(true)
                    .smart_aim(true)
                    .text("CPU Speed");
                ui.add(slider);
            }
        }
        ui.separator();
        if let Some(ref msg) = self.break_message {
            ui.label(msg);
        }
        ui.collapsing("Breakpoints", |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("breakpoint_grid").show(ui, |ui| {
                    ui.add_sized(
                        [120.0, 20.0],
                        egui::TextEdit::singleline(&mut self.bp_add),
                    );
                    let add = ui.button("Add").clicked();
                    if ui.input().key_pressed(egui::Key::Enter) || add {
                        // Parse the string as address in hex.
                        match u32::from_str_radix(&self.bp_add, 16) {
                            Ok(addr) => {
                                self.bp.push(Breakpoint::new(addr, self.bp_add.clone()));
                                self.bp_add.clear();
                            }
                            Err(..) => {
                                self.break_message = Some(
                                    format!("Invalid Breakpoint address: {}", self.bp_add)
                                );
                            }
                        };
                    }
                    ui.end_row();
                    self.bp.retain(|bp| {
                        ui.label(&bp.name); 
                        let remove = !ui.button("Remove").clicked();
                        ui.end_row();
                        remove
                    });
                });
            });
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
