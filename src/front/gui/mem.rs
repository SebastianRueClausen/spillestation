//! Memory viewer/debugger GUI app. Can be configured to show either data or instructions from a
//! given address on the Playstations BUS.

use super::App;
use crate::cpu::Opcode;
use crate::memory::{Bus, Byte, Word};
use std::fmt::Write;
use std::str;

/// One cell in the current value matrix. This is two hex characters which represent one byte.
type Cell = [u8; 2];

/// The ['MemView'] app has two modes. Value and Instruction. Value mode shows a matrix of byte
/// vales read from the BUS of the Playstation. Instruction Mode show a list of instruction from
/// the BUS. It obivously doesn't know if something is an instruction, so it might show junk.
enum Mode {
    Value { matrix: [[Cell; COLUMNS]; ROWS] },
    Instruction { instructions: [String; ROWS] },
}

/// An ['App'] used to view/display the memory of the Playstation.
pub struct MemView {
    /// The first address being shown.
    start_addr: u32,
    /// Labels of all the addresses being shown. They get updated each frame, but saving them
    /// avoids allocating new string each frame.
    addresses: [String; ROWS],
    /// The current mode.
    mode: Mode,
}

impl MemView {
    pub fn new() -> Self {
        Self {
            start_addr: 0x0,
            addresses: Default::default(),
            // Default to value mode.
            mode: Mode::Value {
                matrix: [[[0x0; 2]; COLUMNS]; ROWS],
            },
        }
    }

    pub fn update_info(&mut self, bus: &mut Bus) {
        // The address get's aligned if it's in instruction mode, since instructions must start on
        // 4-byte aligned address.
        let (start_addr, delta) = match self.mode {
            Mode::Instruction { .. } => ((self.start_addr + 4 - 1) / 4 * 4, 4),
            Mode::Value { .. } => (self.start_addr, ROWS),
        };
        for (i, address) in self.addresses.iter_mut().enumerate() {
            address.clear();
            write!(address, "{:06x}:\t", start_addr + (delta * i) as u32).unwrap();
        }
        match self.mode {
            Mode::Instruction {
                ref mut instructions,
            } => {
                for (i, ins) in instructions.iter_mut().enumerate() {
                    ins.clear();
                    match bus.try_load::<Word>(start_addr + (i * 4) as u32) {
                        Some(value) => {
                            write!(ins, "{}", Opcode::new(value)).unwrap();
                        }
                        None => {
                            write!(ins, "??").unwrap();
                        }
                    }
                }
            }
            Mode::Value { ref mut matrix } => {
                for (i, row) in matrix.iter_mut().enumerate() {
                    for (j, col) in row.iter_mut().enumerate() {
                        match bus.try_load::<Byte>((i * COLUMNS + j) as u32 + start_addr) {
                            Some(value) => {
                                col[0] = HEX_ASCII[((value >> 4) & 0xf) as usize];
                                col[1] = HEX_ASCII[(value & 0xf) as usize];
                            }
                            None => {
                                col[0] = b'?';
                                col[1] = b'?';
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for MemView {
    fn default() -> Self {
        Self::new()
    }
}

impl App for MemView {
    fn update(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.start_addr).speed(1.0));
            let mut ins_mode = match self.mode {
                Mode::Instruction { .. } => true,
                Mode::Value { .. } => false,
            };
            ui.selectable_value(&mut ins_mode, false, "Value");
            ui.selectable_value(&mut ins_mode, true, "Instruction");
            match self.mode {
                Mode::Instruction { .. } if !ins_mode => {
                    self.mode = Mode::Value {
                        matrix: [[[0x0; 2]; COLUMNS]; ROWS],
                    }
                }
                Mode::Value { .. } if ins_mode => {
                    self.mode = Mode::Instruction {
                        instructions: Default::default(),
                    }
                }
                _ => {}
            }
        });
        ui.separator();
        match self.mode {
            Mode::Instruction { ref instructions } => {
                egui::Grid::new("instruction_grid").show(ui, |ui| {
                    for (ins, addr) in instructions.iter().zip(self.addresses.iter()) {
                        ui.label(addr);
                        ui.label(ins);
                        ui.end_row();
                    }
                });
            }
            Mode::Value { ref matrix } => {
                egui::Grid::new("mem_value_grid").show(ui, |ui| {
                    for (row, addr) in matrix.iter().zip(self.addresses.iter()) {
                        ui.label(addr);
                        for col in row {
                            ui.label(unsafe { str::from_utf8_unchecked(col) });
                        }
                        ui.end_row();
                    }
                });
            }
        }
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Memory View")
            .open(open)
            .resizable(true)
            .min_width(120.0)
            .show(ctx, |ui| {
                self.update(ui);
            });
    }
}

const COLUMNS: usize = 8;
const ROWS: usize = 8;
const HEX_ASCII: &[u8] = "0123456789abcdef".as_bytes();
