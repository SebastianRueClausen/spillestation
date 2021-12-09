use super::App;
use crate::gpu::Gpu;
use std::fmt::Write;

pub struct GpuStatus {
    fields: [String; FIELD_COUNT],
}

impl GpuStatus {
    pub fn new() -> Self {
        Self {
            fields: Default::default(),
        }
    }

    fn write_fields(&mut self, gpu: &Gpu) -> Result<(), std::fmt::Error> {
        write!(self.fields[0], "{:08x}", gpu.draw_x_offset)?;
        write!(self.fields[1], "{:08x}", gpu.draw_y_offset)?;
        write!(self.fields[2], "{:08x}", gpu.display_vram_x_start)?;
        write!(self.fields[3], "{:08x}", gpu.display_vram_y_start)?;
        write!(self.fields[4], "{:08x}", gpu.display_column_start)?;
        write!(self.fields[5], "{:08x}", gpu.display_column_end)?;
        write!(self.fields[6], "{:08x}", gpu.display_line_start)?;
        write!(self.fields[7], "{:08x}", gpu.display_line_end)?;
        write!(self.fields[8], "{:08x}", gpu.status.texture_page_x_base())?;
        write!(self.fields[9], "{:08x}", gpu.status.texture_page_y_base())?;
        write!(self.fields[10], "{}", gpu.status.trans_blending())?;
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
        write!(self.fields[22], "{}", gpu.status.vertical_interlace_enabled())?;
        write!(self.fields[23], "{}", gpu.status.display_enabled())?;
        write!(self.fields[24], "{}", gpu.status.interrupt_request_enabled())?;
        write!(self.fields[25], "{}", gpu.status.cmd_ready())?;
        write!(self.fields[26], "{}", gpu.status.vram_to_cpu_ready())?;
        write!(self.fields[27], "{}", gpu.status.dma_block_ready())?;
        write!(self.fields[28], "{}", gpu.status.dma_direction())?;
        Ok(())
    }

    pub fn update_fields(&mut self, gpu: &Gpu) {
        self.fields.iter_mut().for_each(|field| field.clear());
        if let Err(err) = self.write_fields(gpu) {
            eprintln!("{}", err);
        }
    }
}

impl Default for GpuStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl App for GpuStatus {
    fn update(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("gpu_status_grid").show(ui, |ui| {
                    for (field, label) in self.fields.iter().zip(FIELD_LABELS.iter()) {
                        ui.label(label);
                        ui.label(&field);
                        ui.end_row();
                    }
                });
            });
    }

    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("GPU Status")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                self.update(ui);
            });
    }
}

const FIELD_LABELS: [&'static str; FIELD_COUNT] = [
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