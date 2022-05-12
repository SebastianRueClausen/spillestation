//! GUI app that displays information about the GPU.

use super::DebugApp;

use splst_core::System;

/// ['App'] for shows the current status of the ['Gpu'].
#[derive(Default)]
pub struct GpuStatus;

impl DebugApp for GpuStatus {
    fn name(&self) -> &'static str {
        "GPU Status"
    }

    fn show(&mut self, system: &mut System, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("gpu_status_grid").show(ui, |ui| {
                    let gpu = system.gpu();
                    
                    ui.label("draw x offset");
                    ui.label(format!("{:08x}", gpu.x_offset));
                    ui.end_row();
                    
                    ui.label("draw y offset");
                    ui.label(format!("{:08x}", gpu.y_offset));
                    ui.end_row();

                    ui.label("display vram x start");
                    ui.label(format!("{:08x}", gpu.vram_x_start));
                    ui.end_row();

                    ui.label("display vram y start");
                    ui.label(format!("{:08x}", gpu.vram_y_start));
                    ui.end_row();

                    ui.label("display column start");
                    ui.label(format!("{:08x}", gpu.dis_x_start));
                    ui.end_row();

                    ui.label("display column end");
                    ui.label(format!("{:08x}", gpu.dis_x_end));
                    ui.end_row();

                    ui.label("display line start");
                    ui.label(format!("{:08x}", gpu.dis_y_start));
                    ui.end_row();

                    ui.label("display line end");
                    ui.label(format!("{:08x}", gpu.dis_y_end));
                    ui.end_row();
                    
                    let status = system.gpu().status();

                    ui.label("texture page x base");
                    ui.label(format!("{:08x}", status.tex_page_x()));
                    ui.end_row();

                    ui.label("texture page y base");
                    ui.label(format!("{:08x}", status.tex_page_y()));
                    ui.end_row();

                    ui.label("transparency blending");
                    ui.label(format!("{}", status.blend_mode()));
                    ui.end_row();

                    ui.label("texture depth");
                    ui.label(format!("{}", status.texture_depth()));
                    ui.end_row();

                    ui.label("dithering enabled");
                    ui.label(format!("{}", status.dithering_enabled()));
                    ui.end_row();

                    ui.label("draw to display");
                    ui.label(format!("{}", status.draw_to_display()));
                    ui.end_row();

                    ui.label("set mask bit");
                    ui.label(format!("{}", status.set_mask_bit()));
                    ui.end_row();

                    ui.label("draw masked pixels");
                    ui.label(format!("{}", status.draw_masked_pixels()));
                    ui.end_row();

                    ui.label("interlace field");
                    ui.label(format!("{}", status.interlace_field()));
                    ui.end_row();

                    ui.label("texture disabled");
                    ui.label(format!("{}", status.texture_disabled()));
                    ui.end_row();

                    ui.label("horizontal resolution");
                    ui.label(format!("{}", status.horizontal_res()));
                    ui.end_row();

                    ui.label("vertical resolution");
                    ui.label(format!("{}", status.vertical_res()));
                    ui.end_row();

                    ui.label("video mode");
                    ui.label(format!("{}", status.video_mode()));
                    ui.end_row();

                    ui.label("color depth");
                    ui.label(format!("{}", status.color_depth()));
                    ui.end_row();

                    ui.label("vertical interlace enabled");
                    ui.label(format!("{}", status.vertical_interlace()));
                    ui.end_row();

                    ui.label("display enabled");
                    ui.label(format!("{}", status.display_enabled()));
                    ui.end_row();

                    ui.label("interrupt request enabled");
                    ui.label(format!("{}", status.irq_enabled()));
                    ui.end_row();

                    ui.label("command ready");
                    ui.label(format!("{}", status.cmd_ready()));
                    ui.end_row();

                    ui.label("VRAM to CPU ready");
                    ui.label(format!("{}", status.vram_to_cpu_ready()));
                    ui.end_row();

                    ui.label("DMA block ready");
                    ui.label(format!("{}", status.dma_block_ready()));
                    ui.end_row();

                    ui.label("DMA direction");
                    ui.label(format!("{}", status.dma_direction()));
                    ui.end_row();
                });
            });
    }

    fn show_window(&mut self, system: &mut System, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("GPU Status")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| self.show(system, ui));
    }
}
