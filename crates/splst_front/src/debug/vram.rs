//! GUI app to view the content of the playstations VRAM.

use super::DebugApp;

use splst_core::System;

use std::str;
use std::time::Duration;

/// One cell in the value matrix, which represent a 16-bit integer represented as 4 hex digits.
type Cell = [u8; 4];

/// ['App'] to view the content of ['gpu::Vram'].
#[derive(Default)]
pub struct VramView {
    /// The first x address.
    x: i32,
    /// The first y address.
    y: i32,
    /// Value matrix starting at 'x' and 'y'.
    matrix: [[Cell; COLUMNS]; ROWS],
    dump: bool,
    new_image: Option<egui::ColorImage>,
    image: Option<egui::TextureHandle>,
    image_scale: f32,
}

impl DebugApp for VramView {
    fn name(&self) -> &'static str {
        "VRAM View"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System) {
        for (i, row) in self.matrix.iter_mut().enumerate() {
            for (j, col) in row.iter_mut().enumerate() {
                let value = system
                    .gpu()
                    .vram()
                    .load_16(self.x + j as i32, self.y + i as i32);
                for (c, i) in col.iter_mut().zip([12, 8, 4, 0]) {
                    *c = HEX_ASCII[((value >> i) & 0xf) as usize];
                }
            }
        }
        
        if self.dump {
            self.dump = false;
            self.new_image = Some(egui::ColorImage::from_rgba_unmultiplied(
                [1024, 512], &system.gpu().vram().to_rgba(),
            ));
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.x).speed(1.0));
            ui.add(egui::DragValue::new(&mut self.y).speed(1.0));
        });

        ui.separator();

        egui::Grid::new("vram_value_grid").striped(true).show(ui, |ui| {
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

        ui.separator();

        ui.horizontal(|ui| {
            self.dump |= ui.button("Dump VRAM").clicked();
            if self.image.is_some() {
                ui.add(egui::Slider::new(&mut self.image_scale, 0.1..=1.0).text("Scale"));
            }
        });

        if let Some(image) = self.new_image.take() {
            self.image = Some(ui.ctx().load_texture("vram", image));
        }
        
        if let Some(image) = &self.image {
            ui.image(image, egui::Vec2::new(1024.0 * self.image_scale, 512.0 * self.image_scale));
        }
    }

    fn show_window(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("VRAM View")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| self.show(ui));
    }
}

const COLUMNS: usize = 8;
const ROWS: usize = 8;
const HEX_ASCII: &[u8] = "0123456789abcdef".as_bytes();
