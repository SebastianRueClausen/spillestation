//! # TODO
//! 
//! - Pressing enter to add a checkpoint should check at least that the App is focused or
//!   something.

use super::DebugApp;

use splst_core::cpu::REGISTER_NAMES;
use splst_core::{System, StopReason, Debugger};

use std::time::Duration;

#[derive(PartialEq, Eq)]
enum RunMode {
    Step {
        /// The amount of instruction for each step.
        amount: u64,
        /// If the step button has been pressed.
        stepped: bool,
    },
    Run {
        /// CPU Hz the system runs at.
        speed: u64,
        /// This is used to run at a more precise Hz. It's also required to run the
        /// CPU at a lower Hz than the update rate, since there may be multiple
        /// updates between each CPU cycle.
        remainder: Duration,
    },
}

impl RunMode {
    fn step() -> Self {
        RunMode::Step {
            amount: 1,
            stepped: false
        }
    }

    fn run() -> Self {
        RunMode::Run {
            speed: 1,
            remainder: Duration::ZERO
        }
    }
}

impl Default for RunMode {
    fn default() -> Self {
        Self::step()
    }
}

/// A single breakpoint. Used for instructions loads, data loads or data stores.
struct BreakPoint {
    name: String,
    addr: u32,
    /// Avoid removing the break when it get's hit.
    retain: bool, 
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BreakPointTy {
    Ins,
    Load,
    Store,
}

impl BreakPointTy {
    fn name(self) -> &'static str {
        match self {
            BreakPointTy::Ins => "Instruction Load",
            BreakPointTy::Load => "Data Load",
            BreakPointTy::Store => "Data store",
        }
    }
}

/// Represents a breakpoint which has been hit.
struct Break {
    addr: u32,
    kind: BreakPointTy,
}

/// Keeps track of all breakpoints and used as ['Debugger'] by the system. 
#[derive(Default)]
struct BreakPoints {
    ins: Vec<BreakPoint>,
    loads: Vec<BreakPoint>,
    stores: Vec<BreakPoint>,
    /// Keeps track of all breaks that has accured, but hasn't been reported.
    breaks: Vec<Break>,
}

impl Debugger for BreakPoints {
    fn instruction_load(&mut self, addr: u32) {
        self.ins.retain(|bp| {
            if bp.addr == addr {
                self.breaks.push(Break {
                    kind: BreakPointTy::Ins,
                    addr,
                });
                bp.retain
            } else {
                true
            }
        });
    }

    fn data_load(&mut self, addr: u32) {
        self.loads.retain(|bp| {
            if bp.addr == addr {
                self.breaks.push(Break {
                    kind: BreakPointTy::Load,
                    addr,
                });
                bp.retain
            } else {
                true
            }
        });
    }

    fn data_store(&mut self, addr: u32) {
        self.stores.retain(|bp| {
            if bp.addr == addr {
                self.breaks.push(Break {
                    kind: BreakPointTy::Store,
                    addr,
                });
                bp.retain
            } else {
                true
            }
        });
    }

    fn should_stop(&mut self) -> bool {
        !self.breaks.is_empty()
    }
}

/// Data for the part of the app for adding a new breakpoint.
struct BreakPointAdd {
    /// The address input.
    addr: String,
    kind: BreakPointTy,
    retain: bool,
}

impl Default for BreakPointAdd {
    fn default() -> Self {
        Self {
            addr: String::new(),
            kind: BreakPointTy::Ins,
            retain: false,
        }
    }
}

#[derive(Default)]
pub struct CpuApp {
    mode: RunMode,
    /// Message shown when a break point has been hit.
    bp_msg: Option<String>,
    bps: BreakPoints,
    bp_add: BreakPointAdd,
}

impl CpuApp {
    /// Show the breakpoints section.
    fn show_breakpoints(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [120.0, 20.0],
                    egui::TextEdit::singleline(&mut self.bp_add.addr),
                );
                egui::ComboBox::from_id_source("type_combo")
                    .selected_text(self.bp_add.kind.name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.bp_add.kind,
                            BreakPointTy::Ins,
                            BreakPointTy::Ins.name(),
                        );
                        ui.selectable_value(
                            &mut self.bp_add.kind,
                            BreakPointTy::Load,
                            BreakPointTy::Load.name(),
                        );
                        ui.selectable_value(
                            &mut self.bp_add.kind,
                            BreakPointTy::Store,
                            BreakPointTy::Store.name(),
                        );
                    });

                ui.checkbox(&mut self.bp_add.retain, "Retain");
                
                let enter = ui.input().key_pressed(egui::Key::Enter);
                if enter || ui.button("Add").clicked() {
                    // Parse the string as address in hex.
                    if let Ok(addr) = u32::from_str_radix(&self.bp_add.addr, 16) {
                        let vec = match self.bp_add.kind {
                            BreakPointTy::Ins => &mut self.bps.ins,
                            BreakPointTy::Load => &mut self.bps.loads,
                            BreakPointTy::Store => &mut self.bps.stores,
                        };

                        let bp = BreakPoint {
                            name: self.bp_add.addr.clone(),
                            retain: self.bp_add.retain,
                            addr,
                        };

                        vec.push(bp);
                        self.bp_add.addr.clear();
                    } else {
                        self.bp_msg = Some(
                            format!("Invalid Breakpoint Address: {}", self.bp_add.addr)
                        );
                    }
                }
            });

            ui.separator();

            egui::Grid::new("breakpoint_grid")
                .min_col_width(100.0)
                .show(ui, |ui| {
                    ui.label("Address");
                    ui.label("Kind");
                    ui.label("Retain");
                    ui.end_row();

                    show_bps(ui, &mut self.bps.ins, BreakPointTy::Ins.name());
                    show_bps(ui, &mut self.bps.loads, BreakPointTy::Load.name());
                    show_bps(ui, &mut self.bps.stores, BreakPointTy::Store.name());
                });
        });

    }
}

impl DebugApp for CpuApp {
    fn name(&self) -> &'static str {
        "CPU"
    }

    fn update_tick(&mut self, dt: Duration, sys: &mut System) {
        let stop = match self.mode {
            RunMode::Step { ref mut stepped, amount } => {
                if *stepped {
                    *stepped = false;
                    self.bp_msg = None;
                    sys.step_debug(amount, &mut self.bps)
                } else {
                    StopReason::Timeout
                }
            }
            RunMode::Run { speed, ref mut remainder } => {
                // Clear the break message if running, since the user must have clicked run again
                // after a breakpoint.
                self.bp_msg = None;
                let time = *remainder + dt;
                let (rem, stop) = sys.run_debug(speed, time, &mut self.bps);
                *remainder = rem;
                stop
            }
        };

        if stop == StopReason::Break {
            self.mode = RunMode::step();

            let message: String = self.bps.breaks
                .drain(..)
                .map(|b| {
                    let kind = match b.kind {
                        BreakPointTy::Ins => "loading instruction",
                        BreakPointTy::Load => "loading data",
                        BreakPointTy::Store => "storing data",
                    };
                    format!("Broke {kind} at '{:08x}'\n", b.addr)
                })
                .collect();
            
            self.bp_msg = Some(message);
        }
    }

    fn show(&mut self, system: &mut System, ui: &mut egui::Ui) {
        let was_step = matches!(self.mode, RunMode::Step { .. });
        let mut is_step = was_step;

        ui.horizontal(|ui| {
            ui.selectable_value(&mut is_step, true, "Step");
            ui.selectable_value(&mut is_step, false, "Run");
        });

        if was_step != is_step {
            if is_step {
                self.mode = RunMode::step();
            } else {
                self.mode = RunMode::run();
            }
        }

        ui.add_space(6.0);

        match self.mode {
            RunMode::Step { ref mut amount, ref mut stepped } => {
                let suffix = match amount {
                    0 | 2.. => " cycles",
                    1 => " cycle",
                };
                ui.horizontal(|ui| {
                    let slider = egui::Slider::new(amount, 1..=40_000_000)
                        .suffix(suffix)
                        .logarithmic(true)
                        .clamp_to_range(true)
                        .smart_aim(true)
                        .text("Step Amount");

                    ui.add(slider);

                    *stepped = ui.button("Step").clicked();
                });
            }
            RunMode::Run { ref mut speed, ..  } => {
                let slider = egui::Slider::new(speed, 1..=40_000_000)
                    .suffix("Hz")
                    .logarithmic(true)
                    .clamp_to_range(true)
                    .smart_aim(true)
                    .text("CPU Speed");
                
                ui.add(slider);
            }
        }

        ui.add_space(6.0);
        
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(ref msg) = self.bp_msg {
                ui.label(msg);
            }

            ui.collapsing("Breakpoints", |ui| {
                self.show_breakpoints(ui);
            });

            ui.collapsing("Status", |ui| {
                egui::Grid::new("cpu_status_grid").show(ui, |ui| {
                    ui.label("hi");
                    ui.label(format!("{}", system.cpu.hi));
                    ui.end_row();

                    ui.label("lo");
                    ui.label(format!("{}", system.cpu.lo));
                    ui.end_row();

                    ui.label("pc");
                    ui.label(format!("{:08x}", system.cpu.pc));
                    ui.end_row();

                    ui.label("instruction");
                    ui.label(format!("{}", system.cpu.curr_ins()));
                    ui.end_row();
                    
                    ui.label("icache misses");
                    ui.label(format!("{}", system.cpu.icache_misses()));
                    ui.end_row();
                    
                    // Show registers.
                    ui.end_row();

                    for (val, name) in system.cpu.registers.iter().zip(REGISTER_NAMES) {
                        ui.label(format!("${name}"));
                        ui.label(format!("{val:08x}"));
                        ui.end_row();
                    }
                });
            });
        });
    }

    fn show_window(&mut self, system: &mut System, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("CPU")
            .open(open)
            .resizable(true)
            .default_width(100.0)
            .default_height(300.0)
            .show(ctx, |ui| self.show(system, ui));
    }
}

fn show_bps(ui: &mut egui::Ui, bps: &mut Vec<BreakPoint>, kind: &str) {
    bps.retain_mut(|bp| {
        ui.label(&bp.name);
        ui.label(kind);
        ui.checkbox(&mut bp.retain, "");
        let retain = !ui.button("Remove").clicked();
        ui.end_row();
        retain
    });
}
