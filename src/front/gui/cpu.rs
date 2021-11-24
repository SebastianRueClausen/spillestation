use crate::cpu::{Cpu, REGISTER_NAMES};
use std::fmt::Write;
use super::App;

pub struct CpuInfo {
    registers: [String; 32],
    hi: String,
    lo: String,
    pc: String,
    ins: String,
}

impl CpuInfo {
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


impl App for CpuInfo {
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
        egui::Window::new("CPU Info")
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
