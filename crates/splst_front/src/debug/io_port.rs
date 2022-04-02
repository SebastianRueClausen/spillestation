use super::DebugApp;

use splst_core::System;
use splst_core::io_port::{StatReg, CtrlReg, ModeReg};

use std::fmt::Write;
use std::time::Duration;

#[derive(Default)]
pub struct IoPortView {
    active_device: String,
    transfer: String,
    baud: String,
    stat_reg: [String; 5],
    ctrl_reg: [String; 10],
    mode_reg: [String; 2],
}

impl IoPortView {
    pub fn update_stat_reg(&mut self, stat: StatReg) -> Result<(), std::fmt::Error> {
        self.stat_reg.iter_mut().for_each(|field| field.clear());

        write!(&mut self.stat_reg[0], "{}", stat.tx_ready())?;
        write!(&mut self.stat_reg[1], "{}", stat.rx_fifo_not_empty())?;
        write!(&mut self.stat_reg[2], "{}", stat.tx_done())?;
        write!(&mut self.stat_reg[3], "{}", stat.ack_input_lvl())?;
        write!(&mut self.stat_reg[4], "{}", stat.irq())?;

        Ok(())
    }

    pub fn update_ctrl_reg(&mut self, ctrl: CtrlReg) -> Result<(), std::fmt::Error> {
        self.ctrl_reg.iter_mut().for_each(|field| field.clear());

        write!(&mut self.ctrl_reg[0], "{}", ctrl.tx_enabled())?;
        write!(&mut self.ctrl_reg[1], "{}", ctrl.select())?;
        write!(&mut self.ctrl_reg[2], "{}", ctrl.rx_enabled())?;
        write!(&mut self.ctrl_reg[3], "{}", ctrl.ack())?;
        write!(&mut self.ctrl_reg[4], "{}", ctrl.reset())?;
        write!(&mut self.ctrl_reg[5], "{}", ctrl.rx_irq_mode())?;
        write!(&mut self.ctrl_reg[6], "{}", ctrl.tx_irq_enabled())?;
        write!(&mut self.ctrl_reg[7], "{}", ctrl.rx_irq_enabled())?;
        write!(&mut self.ctrl_reg[8], "{}", ctrl.ack_irq_enabled())?;
        write!(&mut self.ctrl_reg[9], "{}", ctrl.io_slot())?;

        Ok(())
    }

    pub fn update_mode_reg(&mut self, mode: ModeReg) -> Result<(), std::fmt::Error> {
        self.mode_reg.iter_mut().for_each(|field| field.clear());

        write!(&mut self.mode_reg[0], "{}", mode.baud_reload_factor())?;
        write!(&mut self.mode_reg[1], "{}", mode.char_width())?;

        Ok(())
    }
    
}

impl DebugApp for IoPortView {
    fn name(&self) -> &'static str {
        "I/O View"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System) {
        let io_port = system.io_port();

        let err = self.update_stat_reg(io_port.stat_reg())
            .and(self.update_ctrl_reg(io_port.ctrl_reg()))
            .and(self.update_mode_reg(io_port.mode_reg()))
            .and({
                self.baud.clear();
                write!(&mut self.baud, "{}", io_port.baud())
            })
            .and({
                self.transfer.clear();

                let transfer = if io_port.waiting_for_ack() {
                    "waiting for acknowledgement"
                } else {
                    if io_port.in_transfer() {
                        "active"
                    } else {
                        "inactive"
                    }
                };

                write!(&mut self.transfer, "{transfer}")
            })
            .and({
                self.active_device.clear();
                if let Some(device) = io_port.active_device() {
                    write!(&mut self.active_device, "{device}")
                } else {
                    write!(&mut self.active_device, "none")
                }
            });

        if let Err(err) = err {
            eprintln!("{}", err);
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("grid").show(ui, |ui| {
                ui.label("baud reload value");
                ui.label(&self.baud);
                ui.end_row();

                ui.label("transfer");
                ui.label(&self.transfer);
                ui.end_row();

                ui.label("active device");
                ui.label(&self.active_device);
                ui.end_row();
            });
            egui::CollapsingHeader::new("Status Register").show(ui, |ui| {
                egui::Grid::new("stat_grid").show(ui, |ui| {
                    for (field, label) in self.stat_reg.iter().zip(STAT_REG_LABELS.iter()) {
                        ui.label(*label);
                        ui.label(field);
                        ui.end_row();
                    }
                })
            });
            egui::CollapsingHeader::new("Control Register").show(ui, |ui| {
                egui::Grid::new("ctrl_grid").show(ui, |ui| {
                    for (field, label) in self.ctrl_reg.iter().zip(CTRL_REG_LABELS.iter()) {
                        ui.label(*label);
                        ui.label(field);
                        ui.end_row();
                    }
                })
            });
            egui::CollapsingHeader::new("Mode Register").show(ui, |ui| {
                egui::Grid::new("mode_grid").show(ui, |ui| {
                    for (field, label) in self.mode_reg.iter().zip(MODE_REG_LABELS.iter()) {
                        ui.label(*label);
                        ui.label(field);
                        ui.end_row();
                    }
                })
            });
        });
    }

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("I/O Port View")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                self.show(ui);
            });
    }
}

const STAT_REG_LABELS: [&str; 5] = [
    "tx ready",
    "rx fifo empty",
    "tx done",
    "acknowledge input level",
    "interrupt",
];

const CTRL_REG_LABELS: [&str; 10] = [
    "tx enabled",
    "select",
    "rx enabled",
    "acknowledge",
    "reset",
    "rx interrupt mode",
    "tx interrupt enabled",
    "rx interrupt enabled",
    "acknowledge interrupt",
    "desired slot"
];

const MODE_REG_LABELS: [&str; 2] = [
    "baud reload factor",
    "character width",
];
