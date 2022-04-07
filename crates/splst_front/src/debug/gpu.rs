//! GUI app that displays information about the GPU.

use super::DebugApp;

use splst_core::gpu::Gpu;
use splst_core::System;

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
    fn update_fields(&mut self, gpu: &Gpu) -> Result<(), std::fmt::Error> {
        write!(self.fields[0], "{:08x}", gpu.x_offset)?;
        write!(self.fields[1], "{:08x}", gpu.y_offset)?;
        write!(self.fields[2], "{:08x}", gpu.vram_x_start)?;
        write!(self.fields[3], "{:08x}", gpu.vram_y_start)?;
        write!(self.fields[4], "{:08x}", gpu.dis_x_start)?;
        write!(self.fields[5], "{:08x}", gpu.dis_x_end)?;
        write!(self.fields[6], "{:08x}", gpu.dis_y_start)?;
        write!(self.fields[7], "{:08x}", gpu.dis_y_end)?;

        let status = gpu.status();

        write!(self.fields[8], "{:08x}", status.tex_page_x())?;
        write!(self.fields[9], "{:08x}", status.tex_page_y())?;
        write!(self.fields[10], "{}", status.blend_mode())?;
        write!(self.fields[11], "{}", status.texture_depth())?;
        write!(self.fields[12], "{}", status.dithering_enabled())?;
        write!(self.fields[13], "{}", status.draw_to_display())?;
        write!(self.fields[14], "{}", status.set_mask_bit())?;
        write!(self.fields[15], "{}", status.draw_masked_pixels())?;
        write!(self.fields[16], "{}", status.interlace_field())?;
        write!(self.fields[17], "{}", status.texture_disabled())?;
        write!(self.fields[18], "{}", status.horizontal_res())?;
        write!(self.fields[19], "{}", status.vertical_res())?;
        write!(self.fields[20], "{}", status.video_mode())?;
        write!(self.fields[21], "{}", status.color_depth())?;
        write!(self.fields[22], "{}", status.vertical_interlace())?;
        write!(self.fields[23], "{}", status.display_enabled())?;
        write!(self.fields[24], "{}", status.irq_enabled())?;
        write!(self.fields[25], "{}", status.cmd_ready())?;
        write!(self.fields[26], "{}", status.vram_to_cpu_ready())?;
        write!(self.fields[27], "{}", status.dma_block_ready())?;
        write!(self.fields[28], "{}", status.dma_direction())?;
        Ok(())
    }
}

impl DebugApp for GpuStatus {
    fn name(&self) -> &'static str {
        "GPU Status"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System) {
        self.fields.iter_mut().for_each(|field| field.clear());
        if let Err(err) = self.update_fields(system.gpu()) {
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
