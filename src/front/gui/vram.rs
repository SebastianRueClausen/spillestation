use super::App;
use crate::gpu::Gpu;
use std::str;

type Cell = [u8; 4];

pub struct VramView {
    x: i32,
    y: i32,
    matrix: [[Cell; COLUMNS]; ROWS],
}

impl VramView {
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            matrix: [[[0x0; 4]; COLUMNS]; ROWS],
        }
    }

    pub fn update_matrix(&mut self, gpu: &Gpu) {
        for (i, row) in self.matrix.iter_mut().enumerate() {
            for (j, col) in row.iter_mut().enumerate() {
                let value = gpu.vram().load_16(self.x + j as i32, self.y + i as i32);
                for (c, i) in col.iter_mut().zip([12, 8, 4, 0]) {
                    *c = HEX_ASCII[((value >> i) & 0xf) as usize];
                }
            }
        }
    }
}

impl Default for VramView {
    fn default() -> Self {
        Self::new()
    }
}

impl App for VramView {
    fn update(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.x).speed(1.0));
            ui.add(egui::DragValue::new(&mut self.y).speed(1.0));
        });
        ui.separator();
        egui::Grid::new("vram_value_grid").show(ui, |ui| {
            ui.label("");
            for i in 0..COLUMNS {
                ui.label(format!("{:06x}", self.x + i as i32));
            }
            ui.end_row();
            for (i, row) in self.matrix.iter().enumerate() {
                ui.label(format!("{:06x}:\t", self.y + i as i32));
                for col in row {
                    ui.label(unsafe { str::from_utf8_unchecked(col) });
                }
                ui.end_row();
            }
        });
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("VRAM View")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                self.update(ui);
            });
    }
}

const COLUMNS: usize = 8;
const ROWS: usize = 8;

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
