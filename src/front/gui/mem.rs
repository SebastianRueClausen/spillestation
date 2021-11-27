use crate::memory::{Bus, Byte, Word};
use crate::cpu::Opcode;
use super::App;
use std::str;
use std::fmt::Write;

const COLUMNS: usize = 8;
const ROWS: usize = 8;

type Cell = [u8; 2];

pub struct MemView {
    start_addr: u32,
    matrix: [[Cell; COLUMNS]; ROWS],
    instructions: [String; ROWS],
    ins_mode: bool,
}

impl MemView {
    pub fn new() -> Self {
        Self {
            start_addr: 0x0,
            matrix: [[[0x0; 2]; COLUMNS]; ROWS],
            instructions: Default::default(),
            ins_mode: false,
        }
    }

    pub fn update_info(&mut self, bus: &Bus) {
        if self.ins_mode {
            // Align to next multiple of 4.
            let aligned = (self.start_addr + 4 - 1) / 4 * 4;
            for (i, ins) in self.instructions.iter_mut().enumerate() {
                ins.clear();
                match bus.try_load::<Word>(aligned + (i * 4) as u32) {
                    Some(value) => {
                        write!(ins, "{}", Opcode::new(value)).unwrap();
                    },
                    None => {
                        write!(ins, "??").unwrap();
                    }
                }
            }
        } else {
            for (i, row) in self.matrix.iter_mut().enumerate() {
                for (j, col) in row.iter_mut().enumerate() {
                    match bus.try_load::<Byte>((i * COLUMNS + j) as u32 + self.start_addr) {
                        Some(value) => {
                            col[0] = HEX_ASCII[((value >> 4) & 0xf) as usize];
                            col[1] = HEX_ASCII[((value >> 0) & 0xf) as usize];
                        },
                        None => {
                            col[0] = '?' as u8;
                            col[1] = '?' as u8;
                        }
                    }
                }
            }
        }
    }
}

impl App for MemView {
    fn update(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.start_addr).speed(1.0));
            ui.radio_value(&mut self.ins_mode, true, "Instructions");
            ui.radio_value(&mut self.ins_mode, false, "Values");
        });
        if self.ins_mode {
            let aligned = (self.start_addr + 4 - 1) / 4 * 4;
            egui::Grid::new("Instructions Grid").show(ui, |ui| {
                for (i, ins) in self.instructions.iter().enumerate() {
                    ui.label(format!("{:06x}: ", aligned + (4 * i) as u32));
                    ui.label(ins); 
                    ui.end_row();
                }
            });
        } else {
            egui::Grid::new("Value Grid").show(ui, |ui| {
                for (i, row) in self.matrix.iter().enumerate() {
                    ui.label(format!("{:06x}:\t", self.start_addr + (ROWS * i) as u32));
                    for col in row {
                        ui.label(unsafe { str::from_utf8_unchecked(col) });
                    }
                    ui.end_row();
                }
            });
        }
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Memory View")
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

const HEX_ASCII: [u8; 16] = [
    '0' as u8,
    '1' as u8,
    '2' as u8,
    '3' as u8,
    '4' as u8,
    '5' as u8,
    '6' as u8,
    '7' as u8,
    '8' as u8,
    '9' as u8,
    'a' as u8,
    'b' as u8,
    'c' as u8,
    'd' as u8,
    'e' as u8,
    'f' as u8,
];

