use splst_util::{Bit, BitSet};

use super::Gpu;

impl Gpu {
    // GP1(0) - Resets the state of the GPU.
    pub fn gp1_reset(&mut self) {
        self.fifo.clear();

        self.status.0 = 0x14802000;

        self.vram_x_start = 0;
        self.vram_y_start = 0;

        self.dis_x_start = 0x200;
        self.dis_x_end = 0xc00;

        self.dis_y_start = 0x10;
        self.dis_y_end = 0x100;

        self.tex_x_flip = false;
        self.tex_y_flip = false;

        self.tex_win_w = 0;
        self.tex_win_h = 0;

        self.tex_win_x = 0;
        self.tex_win_y = 0;

        self.da_x_max = 0;
        self.da_x_min = 0;

        self.da_y_max = 0;
        self.da_y_min = 0;

        self.x_offset = 0;
        self.y_offset = 0;

        self.timing.update_video_mode(self.status.video_mode());
    }

    // GP1(1) - Reset command buffer.
    pub fn gp1_reset_fifo(&mut self) {
        self.fifo.clear();
    }

    // GP1(2) - Acknowledge GPU Interrupt.
    pub fn gp1_ack_gpu_irq(&mut self) {
        self.status.0 &= !(1 << 24); 
    }

    // GP1(3) - Display Enable.
    // - 0 - Display On/Off.
    pub fn gp1_display_enable(&mut self, val: u32) {
        self.status.0 = self.status.0.set_bit(23, val.bit(0));
    }

    // GP1(4) - Set DMA Direction.
    // - 0..1 - DMA direction.
    pub fn gp1_dma_direction(&mut self, val: u32) {
        let val = val.bit_range(0, 1);
        self.status.0 = self.status.0.set_bit_range(29, 30, val);
    }

    // GP1(5) - Start display area in VRAM.
    // - 0..9 - x (address in VRAM).
    // - 10..18 - y (address in VRAM).
    pub fn gp1_display_start(&mut self, val: u32) {
        self.vram_x_start = val.bit_range(0, 9) as u16;
        self.vram_y_start = val.bit_range(10, 18) as u16;
    }

    // GP1(6) - Horizontal display range.
    // - 0..11 - column start.
    // - 12..23 - column end.
    pub fn gp1_horizontal_display_range(&mut self, val: u32) {
        self.dis_x_start = val.bit_range(0, 11) as u16;
        self.dis_x_end = val.bit_range(12, 23) as u16;
    }

    // GP1(7) - Vertical display range.
    // - 0..11 - line start.
    // - 12..23 - line end.
    pub fn gp1_vertical_display_range(&mut self, val: u32) {
        self.dis_y_start = val.bit_range(0, 11) as u16;
        self.dis_y_end = val.bit_range(12, 23) as u16;
    }

    // GP1(8) - Set display mode.
    // - 0..1 - Horizontal resolution 1.
    // - 2 - Vertical resolution.
    // - 3 - Display mode.
    // - 4 - Display area color depth.
    // - 5 - Vertical interlace.
    // - 6 - Horizontal resolution 2.
    // - 7 - Reverseflag.
    pub fn gp1_display_mode(&mut self, val: u32) {
        self.status.0 = self.status.0
            .set_bit_range(17, 22, val.bit_range(0, 5))
            .set_bit(16, val.bit(6))
            .set_bit(14, val.bit(7));

        let video_mode = self.status.video_mode();

        self.timing.update_video_mode(video_mode);
    }
}
