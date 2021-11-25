use crate::cpu::{Cpu, REGISTER_NAMES};
use std::fmt::Write;
use std::time::Duration;
use super::App;

pub struct CpuStatus {
    registers: [String; 32],
    hi: String,
    lo: String,
    pc: String,
    ins: String,
}

impl CpuStatus {
    pub fn new() -> Self {
        Self {
            registers: Default::default(),
            hi: String::with_capacity(16),
            lo: String::with_capacity(16),
            pc: String::with_capacity(16),
            ins: String::with_capacity(32),
        }
    }

    pub fn update_info(&mut self, cpu: &Cpu) {
        for (show, value) in self.registers.iter_mut().zip(cpu.registers.iter()) {
            show.clear();
            write!(show, "{}", value).unwrap();
        }
        self.hi.clear();
        self.lo.clear();
        self.pc.clear();
        self.ins.clear();
        write!(&mut self.hi, "{:08x}", cpu.hi).unwrap();
        write!(&mut self.lo, "{:08x}", cpu.lo).unwrap();
        write!(&mut self.pc, "{:08x}", cpu.pc).unwrap();
        write!(&mut self.ins, "{}", cpu.current_instruction()).unwrap();
    }
}


impl App for CpuStatus {
    fn update(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Status", |ui| {
            egui::Grid::new("Status Grid").show(ui, |ui| {
                ui.label("hi");
                ui.label(&self.hi);
                ui.end_row();
                ui.label("lo");
                ui.label(&self.lo);
                ui.end_row();
                ui.label("pc");
                ui.label(&self.pc);
                ui.end_row();
                ui.label("ins");
                ui.label(&self.ins);
                ui.end_row();
            });
        });
        ui.collapsing("Registers", |ui| {
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .auto_shrink([false, false])
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

const MAX_CYCLE_HZ: usize = 30_000_000;

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
            cycle_hz: 100000,
            step_amount: 1,
            paused: true,
            stepped: false,
            remainder: Duration::ZERO,
        }
    }

    pub fn run_cpu(&mut self, mut dt: Duration, cpu: &mut Cpu) {
        if !self.paused {
            dt += self.remainder;
            while let Some(new) = dt.checked_sub(Duration::from_secs(1) / self.cycle_hz as u32) {
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
