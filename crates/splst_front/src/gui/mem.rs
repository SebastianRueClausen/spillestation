use super::App;

use splst_core::bus::{Byte, Word};
use splst_core::cpu::Opcode;
use splst_core::System;
use crate::render::Renderer;

use std::fmt::Write;
use std::str;
use std::time::Duration;

/// One cell in the current value matrix. This is two hex characters which represent one byte.
type Cell = [u8; 2];

/// The ['MemView'] app has two modes. Value and Instruction. Value mode shows a matrix of byte
/// vales read from the BUS of the Playstation. Instruction Mode show a list of instruction from
/// the BUS. It obivously doesn't know if something is an instruction, so it might show junk.
enum Mode {
    Value {
        mat: [[Cell; 4]; ROWS],
        txt: [String; ROWS],
    },
    Instruction([String; ROWS]),
}

impl Mode {
    fn new_value() -> Self {
        Mode::Value {
            mat: [[[0x0; 2]; 4]; ROWS],
            txt: Default::default(),
        }
    }

    fn new_instruction() -> Self {
        Mode::Instruction(Default::default())
    }
}

/// An ['App'] used to view/display the memory of the Playstation.
pub struct MemView {
    start_addr: u32,
    addr_input: String,
    addr_input_msg: Option<String>,
    addresses: [String; ROWS],
    mode: Mode,
}

impl Default for MemView {
    fn default() -> Self {
        Self {
            start_addr: 0x0,
            addr_input: String::new(),
            addr_input_msg: None,
            addresses: Default::default(),
            mode: Mode::new_value(),
        }
    }
}

impl App for MemView {
    fn name(&self) -> &'static str {
        "Memory View"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System, _: &mut Renderer) {
        // The start address must be 4-byte aligned. This is a hacky way to round down to next
        // multiple of 4.
        let start_addr = ((self.start_addr + 4) & !3) - 4;
        for (i, addr) in self.addresses.iter_mut().enumerate() {
            addr.clear();
            write!(addr, "{:06x}:\t", start_addr + 4 * i as u32).unwrap();
        }
        match self.mode {
            Mode::Instruction(ref mut ins) => {
                for (i, ins) in ins.iter_mut().enumerate() {
                    ins.clear();
                    let addr = start_addr + (i * 4) as u32;
                    match system.cpu.bus_mut().load::<Word>(addr) {
                        Some(val) => write!(ins, "{}", Opcode::new(val)).unwrap(),
                        None => write!(ins, "??").unwrap(),
                    }
                }
            }
            Mode::Value { ref mut mat, ref mut txt } => {
                for (i, row) in mat.iter_mut().enumerate() {
                    let mut as_text = [0; 4];
                    for (j, col) in row.iter_mut().enumerate() {
                        let addr = (i * 4 + j) as u32 + start_addr;
                        match system.cpu.bus_mut().load::<Byte>(addr) {
                            Some(val) => {
                                as_text[j] = val as u8;
                                col[0] = HEX_ASCII[((val >> 4) & 0xf) as usize];
                                col[1] = HEX_ASCII[(val & 0xf) as usize];
                            }
                            None => {
                                as_text[j] = b'?';
                                col[0] = b'?';
                                col[1] = b'?';
                            }
                        }
                    }
                    txt[i] = String::from_utf8_lossy(&as_text).to_string();
                }
            }
        }
    }
    
    fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mut was_ins = matches!(self.mode, Mode::Instruction(..));
            if ui.selectable_value(&mut was_ins, false, "Value").clicked() {
                self.mode = Mode::new_value();
            }
            if ui.selectable_value(&mut was_ins, true, "Instruction").clicked() {
                self.mode = Mode::new_instruction();
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            let input = egui::TextEdit::singleline(&mut self.addr_input);
            ui.add_sized([120.0, 20.0], input);
            let find = ui.button("Find").clicked();
            if ui.button("⬆").clicked() || ui.input().key_pressed(egui::Key::ArrowUp) {
                self.start_addr = self.start_addr.saturating_sub(4);
            }
            if ui.button("⬇").clicked() || ui.input().key_pressed(egui::Key::ArrowDown) {
                self.start_addr = self.start_addr.saturating_add(4);
            }
            if ui.input().key_pressed(egui::Key::Enter) || find {
                if let Ok(addr) = u32::from_str_radix(&self.addr_input, 16) {
                    self.start_addr = addr;     
                    self.addr_input_msg = None;
                } else {
                    self.addr_input_msg = Some(format!("Invalid Address"))
                };
            }
            if let Some(ref msg) = self.addr_input_msg {
                ui.label(msg);
            }
        });
        ui.separator();
        match self.mode {
            Mode::Instruction(ref ins) => {
                egui::Grid::new("instruction_grid").show(ui, |ui| {
                    for (ins, addr) in ins.iter().zip(self.addresses.iter()) {
                        ui.label(addr);
                        ui.label(ins);
                        ui.end_row();
                    }
                });
            }
            Mode::Value { ref mat, ref txt } => {
                ui.horizontal(|ui| {
                    egui::Grid::new("value_grid")
                        .striped(true)
                        .spacing([0.0, 0.0])
                        .show(ui, |ui| {
                            for (row, addr) in mat.iter().zip(self.addresses.iter()) {
                                ui.label(addr);
                                for col in row {
                                    // It is guarenteed to be an utf8 string, since it only contains
                                    // chars from 'HEX_ASCII'.
                                    ui.label(unsafe { str::from_utf8_unchecked(col) });
                                }
                                ui.end_row();
                            }
                        });
                    ui.separator();
                    egui::Grid::new("text_grid")
                        .striped(true)
                        .spacing([0.0, 0.0])
                        .show(ui, |ui| {
                            for row in txt {
                                ui.label(row); 
                                ui.end_row();
                            }
                        });
                });
            }
        }
    }

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Memory View")
            .open(open)
            .resizable(true)
            .min_width(120.0)
            .show(ctx, |ui| {
                self.show(ui);
            });
    }
}

const ROWS: usize = 8;
const HEX_ASCII: &[u8] = "0123456789abcdef".as_bytes();
