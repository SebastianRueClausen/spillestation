//! GUI app to view the content of the playstations VRAM.

use super::DebugApp;

use splst_core::System;

use std::str;

/// ['App'] to view the content of ['gpu::Vram'].
#[derive(Default)]
pub struct VramView {
    /// The first x address.
    x: i32,
    /// The first y address.
    y: i32,
    /// Image of the VRAM.
    image: Option<egui::TextureHandle>,
    /// Scale of which the 'image' should be shown.
    image_scale: f32,
}

impl DebugApp for VramView {
    fn name(&self) -> &'static str {
        "VRAM View"
    }

    fn show(&mut self, system: &mut System, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.x).speed(1.0));
            ui.add(egui::DragValue::new(&mut self.y).speed(1.0));
        });

        ui.separator();

        egui::Grid::new("vram_value_grid").striped(true).show(ui, |ui| {
            ui.label("");
            
            // X-coord header.
            for i in 0..COLUMNS {
                ui.label(format!("{:06x}", self.x + i as i32));
            }

            ui.end_row();

            for y in 0..ROWS {
                // Y-coord gutter.
                ui.label(format!("{:06x}:\t", self.y + y as i32));
                
                for x in 0..COLUMNS {
                    let val = system
                        .gpu()
                        .vram()
                        .load_16(self.x + x as i32, self.y + y as i32);

                    // One cell which is 16-bits represented as 4 hex chars.
                    let mut cols: [u8; 4] = [0x0; 4];

                    for (col, shift) in cols.iter_mut().zip([12, 8, 4, 0].iter()) {
                        let hex = (val >> shift) & 0xf;
                        *col = HEX_ASCII[hex as usize];
                    }

                    // It's safe since all the chars are hex.
                    ui.label(unsafe {
                        str::from_utf8_unchecked(&cols)
                    });
                }
                
                ui.end_row();
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Dump VRAM").clicked() {
                let raw = system.gpu().vram().to_rgba();
                let image = egui::ColorImage::from_rgba_unmultiplied([1024, 512], &raw);
                self.image = Some(ui.ctx().load_texture("vram", image));
            }
            if self.image.is_some() {
                ui.add(egui::Slider::new(&mut self.image_scale, 0.1..=1.0).text("Scale"));
            }
        });

        if let Some(image) = &self.image {
            ui.image(image, egui::Vec2::new(1024.0 * self.image_scale, 512.0 * self.image_scale));
        }
    }

    fn show_window(&mut self, system: &mut System, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("VRAM View")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| self.show(system, ui));
    }
}

const COLUMNS: usize = 8;
const ROWS: usize = 8;
const HEX_ASCII: &[u8] = "0123456789abcdef".as_bytes();
