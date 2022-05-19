use super::DebugApp;

use splst_core::System;
use splst_core::io_port::{pad, IoSlot};

#[derive(Default)]
pub struct IoPortView;

impl DebugApp for IoPortView {
    fn name(&self) -> &'static str {
        "I/O Port"
    }
    
    fn show(&mut self, system: &mut System, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("grid").show(ui, |ui| {
                let io_port = system.io_port();
                
                ui.label("baud reload factor");
                ui.label(format!("{}", io_port.baud()));
                ui.end_row();

                ui.label("transfer");

                let transfer = if io_port.waiting_for_ack() {
                    "waiting for acknowledgement"
                } else {
                    if io_port.in_transfer() {
                        "active"
                    } else {
                        "inactive"
                    }
                };

                ui.label(transfer);
                ui.end_row();

                ui.label("active device");
                let active_device = io_port
                    .active_device()
                    .map(|dev| format!("{dev}"))
                    .unwrap_or(String::from("none"));
                ui.label(active_device);
                ui.end_row();
            });

            egui::CollapsingHeader::new("Status Register").show(ui, |ui| {
                egui::Grid::new("stat_grid").show(ui, |ui| {
                    let status = system.io_port().stat_reg();
                    
                    ui.label("tx ready");
                    ui.label(format!("{}", status.tx_ready()));
                    ui.end_row();
                    
                    ui.label("rx empty");
                    ui.label(format!("{}", !status.rx_fifo_not_empty()));
                    ui.end_row();
                    
                    ui.label("tx done");
                    ui.label(format!("{}", status.tx_done()));
                    ui.end_row();
                    
                    ui.label("acknowledge input level");
                    ui.label(format!("{}", status.ack_input_lvl()));
                    ui.end_row();

                    ui.label("interrupt");
                    ui.label(format!("{}", status.irq()));
                    ui.end_row();
                })
            });

            egui::CollapsingHeader::new("Control Register").show(ui, |ui| {
                egui::Grid::new("ctrl_grid").show(ui, |ui| {
                    let ctrl = system.io_port().ctrl_reg();
                    
                    ui.label("tx enabled");
                    ui.label(format!("{}", ctrl.tx_enabled()));
                    ui.end_row();
                    
                    ui.label("select");
                    ui.label(format!("{}", ctrl.select()));
                    ui.end_row();

                    ui.label("rx enabled");
                    ui.label(format!("{}", ctrl.rx_enabled()));
                    ui.end_row();

                    ui.label("acknowledge");
                    ui.label(format!("{}", ctrl.ack()));
                    ui.end_row();

                    ui.label("reset");
                    ui.label(format!("{}", ctrl.reset()));
                    ui.end_row();

                    ui.label("rx interrupt mode");
                    ui.label(format!("{}", ctrl.rx_irq_mode()));
                    ui.end_row();
                    
                    ui.label("tx interrupt enabled");
                    ui.label(format!("{}", ctrl.tx_irq_enabled()));
                    ui.end_row();

                    ui.label("rx interrupt enabled");
                    ui.label(format!("{}", ctrl.rx_irq_enabled()));
                    ui.end_row();

                    ui.label("acknowledge interrupt enabled");
                    ui.label(format!("{}", ctrl.ack_irq_enabled()));
                    ui.end_row();

                    ui.label("I/O slot");
                    ui.label(format!("{}", ctrl.io_slot()));
                    ui.end_row();
                })
            });

            egui::CollapsingHeader::new("Mode Register").show(ui, |ui| {
                egui::Grid::new("mode_grid").show(ui, |ui| {
                    let mode = system.io_port().mode_reg();
                    
                    ui.label("baud reload factor");
                    ui.label(format!("{}", mode.baud_reload_factor()));
                    ui.end_row();

                    ui.label("character width");
                    ui.label(format!("{}", mode.char_width()));
                    ui.end_row();
                })
            });

            if let pad::Connection::Digital(ctrl) = system.io_port().pad_at(IoSlot::Slot1) {
                egui::CollapsingHeader::new("Joy 1").show(ui, |ui| {
                    show_button_state(ctrl.button_state(), ui);
                });
            }

            if let pad::Connection::Digital(ctrl) = system.io_port().pad_at(IoSlot::Slot2) {
                egui::CollapsingHeader::new("Joy 2").show(ui, |ui| {
                    show_button_state(ctrl.button_state(), ui);
                });
            }
        });
    }

    fn show_window(&mut self, system: &mut System, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("I/O Port")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| self.show(system, ui));
    }
}

fn show_button_state(button_state: pad::ButtonState, ui: &mut egui::Ui) {
    egui::Grid::new("button_state").show(ui, |ui| {
        for button in pad::Button::ALL.iter() {
            ui.label(format!("{button}"));
            if button_state.is_pressed(*button) {
                ui.label("✔");
            } else {
                ui.label(" ");
            }
            ui.end_row();
        }
    });
}

