use super::DebugApp;

use splst_core::cpu::Opcode;
use splst_core::System;

use std::str;

#[derive(PartialEq)]
enum DisplayMode {
    Value,
    Instruction,
}

/// An ['App'] used to view/display the memory of the Playstation.
pub struct MemView {
    /// The first address shown.
    start_addr: u32,
    /// Input for jumping to a specific address.
    addr_input: String,
    /// Error message if 'addr_input' is invalid.
    addr_input_msg: Option<String>,
    display_mode: DisplayMode,
}

impl Default for MemView {
    fn default() -> Self {
        Self {
            start_addr: 0x0,
            addr_input: String::new(),
            addr_input_msg: None,
            display_mode: DisplayMode::Value,
        }
    }
}

impl DebugApp for MemView {
    fn name(&self) -> &'static str {
        "Memory View"
    }

    fn show(&mut self, system: &mut System, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.display_mode, DisplayMode::Value, "Value");
            ui.radio_value(&mut self.display_mode, DisplayMode::Instruction, "Instruction");
        });
        
        ui.separator();

        ui.horizontal(|ui| {
            ui.add_sized([120.0, 20.0], egui::TextEdit::singleline(&mut self.addr_input));

            let find = ui.button("Find").clicked();

            if ui.button("⬆").clicked() || ui.input().key_pressed(egui::Key::ArrowUp) {
                self.start_addr = self.start_addr.saturating_sub(4);
            }

            if ui.button("⬇").clicked() || ui.input().key_pressed(egui::Key::ArrowDown) {
                self.start_addr = self.start_addr.saturating_add(4);
            }

            if ui.input().key_pressed(egui::Key::Enter) || find {
                self.addr_input_msg = match u32::from_str_radix(&self.addr_input, 16) {
                    Err(err) => Some(format!("Invalid Address: {}", err)),
                    Ok(addr) => {
                        self.start_addr = addr;
                        None
                    }
                };
            }

            if let Some(ref msg) = self.addr_input_msg {
                ui.label(msg);
            }
        });

        ui.separator();

        // Align address to the previous multiple of 4.
        let start_addr = self.start_addr & !3;
        
        match self.display_mode {
            DisplayMode::Instruction => {
                egui::Grid::new("instruction_grid").spacing([0.0; 2]).show(ui, |ui| {
                    for offset in (0..(ROWS * 4)).step_by(4) {
                        let addr = start_addr + offset as u32;
                        
                        ui.label(format!("{addr:06x}\t"));

                        match system.bus().peek::<u32>(addr) {
                            Some(val) => ui.label(format!("{}", Opcode::new(val))),
                            None => ui.label("???"),
                        };

                        ui.end_row();
                    }
                });
            }
            DisplayMode::Value => {
                egui::Grid::new("value_grid").show(ui, |ui| {
                    for offset in (0..(ROWS * 4)).step_by(4) {
                        let addr = start_addr + offset as u32;

                        ui.label(format!("{addr:06x}\t"));

                        match system.bus().peek::<u32>(addr) {
                            Some(val) => {
                                for shift in [24, 16, 8, 0].iter() {
                                    let val = (val >> shift) as usize;
                                    let hex = [
                                        HEX_ASCII[(val >> 4) & 0xf],
                                        HEX_ASCII[val & 0xf],
                                    ];
                                    ui.label(unsafe {
                                        str::from_utf8_unchecked(&hex)
                                    });
                                }
                            }
                            None => {
                                for _ in 0..4 {
                                    ui.label("???");
                                }
                            }
                        }
                        
                        ui.end_row();
                    }
                });
            }
        }
    }

    fn show_window(&mut self, system: &mut System, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("Memory View")
            .open(open)
            .resizable(true)
            .min_width(120.0)
            .show(ctx, |ui| self.show(system, ui));
    }
}

const ROWS: usize = 8;
const HEX_ASCII: &[u8] = "0123456789abcdef".as_bytes();
