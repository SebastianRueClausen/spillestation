use super::App;
use crate::{bus::{Byte, Word}, cpu::Opcode, system::System};
use std::{fmt::Write, str, time::Duration};

/// One cell in the current value matrix. This is two hex characters which represent one byte.
type Cell = [u8; 2];

/// The ['MemView'] app has two modes. Value and Instruction. Value mode shows a matrix of byte
/// vales read from the BUS of the Playstation. Instruction Mode show a list of instruction from
/// the BUS. It obivously doesn't know if something is an instruction, so it might show junk.
enum Mode {
    Value([[Cell; COLUMNS]; ROWS]),
    Instruction([String; ROWS]),
}

/// An ['App'] used to view/display the memory of the Playstation.
pub struct MemView {
    start_addr: u32,
    addresses: [String; ROWS],
    mode: Mode,
}

impl Default for MemView {
    fn default() -> Self {
        Self {
            start_addr: 0x0,
            addresses: Default::default(),
            mode: Mode::Value([[[0x0; 2]; COLUMNS]; ROWS]),
        }
    }
}

impl App for MemView {
    fn name(&self) -> &'static str {
        "Memory View"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System) {
        // The address get's aligned if it's in instruction mode, since instructions must start on
        // 4-byte aligned address.
        let (start_addr, delta) = match self.mode {
            Mode::Instruction(..) => ((self.start_addr + 4 - 1) / 4 * 4, 4),
            Mode::Value(..) => (self.start_addr, ROWS),
        };
        for (i, address) in self.addresses.iter_mut().enumerate() {
            address.clear();
            write!(address, "{:06x}:\t", start_addr + (delta * i) as u32).unwrap();
        }
        match self.mode {
            Mode::Instruction(ref mut ins) => {
                for (i, ins) in ins.iter_mut().enumerate() {
                    ins.clear();
                    match system.cpu
                        .bus_mut()
                        .try_load::<Word>(start_addr + (i * 4) as u32)
                    {
                        Some(value) => {
                            write!(ins, "{}", Opcode::new(value)).unwrap();
                        }
                        None => {
                            write!(ins, "??").unwrap();
                        }
                    }
                }
            }
            Mode::Value(ref mut matrix) => {
                for (i, row) in matrix.iter_mut().enumerate() {
                    for (j, col) in row.iter_mut().enumerate() {
                        match system.cpu
                            .bus_mut()
                            .try_load::<Byte>((i * COLUMNS + j) as u32 + start_addr)
                        {
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
    
    fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.start_addr).speed(1.0));
            let mut ins_mode = match self.mode {
                Mode::Instruction(..) => true,
                Mode::Value(..) => false,
            };
            ui.selectable_value(&mut ins_mode, false, "Value");
            ui.selectable_value(&mut ins_mode, true, "Instruction");
            match self.mode {
                Mode::Instruction(..) if !ins_mode => {
                    self.mode = Mode::Value([[[0x0; 2]; COLUMNS]; ROWS]);
                }
                Mode::Value { .. } if ins_mode => {
                    self.mode = Mode::Instruction(Default::default());
                }
                _ => ()
            }
        });
        ui.separator();
        match self.mode {
            Mode::Instruction(ref ins) => {
                egui::Grid::new("instruction_grid").striped(true).show(ui, |ui| {
                    for (ins, addr) in ins.iter().zip(self.addresses.iter()) {
                        ui.label(addr);
                        ui.label(ins);
                        ui.end_row();
                    }
                });
            }
            Mode::Value(ref matrix) => {
                egui::Grid::new("mem_value_grid")
                    .striped(true)
                    .spacing([0.0, 0.0])
                    .show(ui, |ui| {
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

const COLUMNS: usize = 8;
const ROWS: usize = 8;
const HEX_ASCII: &[u8] = "0123456789abcdef".as_bytes();
