//! GUI app that displays information about the GPU.

use super::App;

use splst_core::gpu::Gpu;
use splst_core::System;
use crate::render::Renderer;

use std::fmt::Write;
use std::time::Duration;

/// ['App'] which shows the current status of the ['Gpu'].
#[derive(Default)]
pub struct GpuStatus {
    /// All the fields. They get updated each frame, so saving them just avoids allocating a lot
    /// of strings each frame.
    fields: [String; FIELD_COUNT],
}

impl GpuStatus {
    /// Write information to all the fields.
    fn write_fields(&mut self, gpu: &Gpu) -> Result<(), std::fmt::Error> {
        write!(self.fields[0], "{:08x}", gpu.x_offset)?;
        write!(self.fields[1], "{:08x}", gpu.y_offset)?;
        write!(self.fields[2], "{:08x}", gpu.vram_x_start)?;
        write!(self.fields[3], "{:08x}", gpu.vram_y_start)?;
        write!(self.fields[4], "{:08x}", gpu.dis_x_start)?;
        write!(self.fields[5], "{:08x}", gpu.dis_x_end)?;
        write!(self.fields[6], "{:08x}", gpu.dis_y_start)?;
        write!(self.fields[7], "{:08x}", gpu.dis_y_end)?;
        write!(self.fields[8], "{:08x}", gpu.status.tex_page_x())?;
        write!(self.fields[9], "{:08x}", gpu.status.tex_page_y())?;
        write!(self.fields[10], "{}", gpu.status.blend_mode())?;
        write!(self.fields[11], "{}", gpu.status.texture_depth())?;
        write!(self.fields[12], "{}", gpu.status.dithering_enabled())?;
        write!(self.fields[13], "{}", gpu.status.draw_to_display())?;
        write!(self.fields[14], "{}", gpu.status.set_mask_bit())?;
        write!(self.fields[15], "{}", gpu.status.draw_masked_pixels())?;
        write!(self.fields[16], "{}", gpu.status.interlace_field())?;
        write!(self.fields[17], "{}", gpu.status.texture_disabled())?;
        write!(self.fields[18], "{}", gpu.status.horizontal_res())?;
        write!(self.fields[19], "{}", gpu.status.vertical_res())?;
        write!(self.fields[20], "{}", gpu.status.video_mode())?;
        write!(self.fields[21], "{}", gpu.status.color_depth())?;
        write!(self.fields[22], "{}", gpu.status.vertical_interlace())?;
        write!(self.fields[23], "{}", gpu.status.display_enabled())?;
        write!(self.fields[24], "{}", gpu.status.irq_enabled())?;
        write!(self.fields[25], "{}", gpu.status.cmd_ready())?;
        write!(self.fields[26], "{}", gpu.status.vram_to_cpu_ready())?;
        write!(self.fields[27], "{}", gpu.status.dma_block_ready())?;
        write!(self.fields[28], "{}", gpu.status.dma_direction())?;
        Ok(())
    }
}

impl App for GpuStatus {
    fn name(&self) -> &'static str {
        "GPU Status"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System, _: &mut Renderer) {
        self.fields.iter_mut().for_each(|field| field.clear());
        if let Err(err) = self.write_fields(system.cpu.bus().gpu()) {
            eprintln!("{}", err);
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("gpu_status_grid").show(ui, |ui| {
                    for (field, label) in self.fields.iter().zip(FIELD_LABELS) {
                        ui.label(label);
                        ui.label(field);
                        ui.end_row();
                    }
                });
            });
    }

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("GPU Status")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                self.show(ui);
            });
    }
}

/// The labels for all the fields. This must lign up with the order of which the fields in
/// ['GpuStatus'] is written to.
const FIELD_LABELS: [&str; FIELD_COUNT] = [
    "draw x offset",
    "draw y offset",
    "display vram x start",
    "display vram y start",
    "display column start",
    "display column end",
    "display line start",
    "display line end",
    "texture page x base",
    "texture page y base",
    "transparency blending",
    "texture depth",
    "dithering enabled",
    "draw to display",
    "set mask bit",
    "draw masked pixels",
    "interlace field",
    "texture disabled",
    "horizontal resolution",
    "vertical resolution",
    "video mode",
    "color depth",
    "vertical interlace enabled",
    "display enabled",
    "interrupt request enabled",
    "command ready",
    "VRAM to CPU ready",
    "DMA block ready",
    "DMA direction",
];

const FIELD_COUNT: usize = 29;
