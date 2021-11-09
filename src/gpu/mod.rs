#![allow(dead_code)]

mod fifo;
mod primitive;
mod vram;
mod rasterize;

use crate::util::bits::BitExtract;
use fifo::Fifo;
use vram::Vram;
use primitive::{
    Vertex,
    Point,
    //Color,
};

/// How many Hz to output.
enum VideoMode {
    /// 60 Hz.
    Ntsc = 60,
    /// 50 Hz.
    Pal = 50,
}

impl VideoMode {
    fn from_value(value: bool) -> Self {
        match value {
            false => VideoMode::Ntsc,
            true => VideoMode::Pal,
        }
    }
}

enum DmaDirection {
    Off = 0,
    Fifo = 1,
    CpuToGp0 = 2,
    VramToCpu = 3,
}

impl DmaDirection {
    fn from_value(value: u32) -> Self {
        match value {
            0 => DmaDirection::Off,
            1 => DmaDirection::Fifo,
            2 => DmaDirection::CpuToGp0,
            3 => DmaDirection::VramToCpu,
            _ => unreachable!("Invalid dma direction"),
        }
    }
}

/// Interlace output split into two fields.
enum InterlaceField {
    Bottom = 0,
    Top = 1,
}

impl InterlaceField {
    fn from_value(value: bool) -> Self {
        match value {
            false => InterlaceField::Bottom,
            true => InterlaceField::Top,
        }
    }
}

/// Number of bits used to represent 1 pixel.
enum ColorDepth {
    B15 = 15,
    B24 = 24,
}

impl ColorDepth {
    fn from_value(value: bool) -> Self {
        match value {
            false => ColorDepth::B15,
            true => ColorDepth::B24,
        }
    }
}

/// Number of bits used to represent 1 texture pixel.
enum TextureDepth {
    B4 = 4,
    B8 = 8,
    B15 = 15,
}

impl TextureDepth {
    fn from_value(value: u32) -> Self {
        match value {
            0 => TextureDepth::B4,
            1 => TextureDepth::B8,
            2 => TextureDepth::B15,
            _ => unreachable!("Invalid texture depth"),
        }
    }
}

/// Status register of the GPU.
struct Status(u32);

impl Status {
    /// Texture page x base coordinate. N * 64.
    fn texture_page_x_base(self) -> u32 {
        self.0.extract_bits(0, 3) * 64
    }

    /// Texture page y base coordinate. N * 256.
    fn texture_page_y_base(self) -> u32 {
        self.0.extract_bit(4) * 256
    }

    /// How to blend source and destination colors.
    fn semi_transparency(self) -> u32 {
        self.0.extract_bits(5, 6)
    }

    /// Depth of the texture colors.
    fn texture_depth(self) -> TextureDepth {
        TextureDepth::from_value(self.0.extract_bits(7, 8))
    }

    fn dithering_enabled(self) -> bool {
        self.0.extract_bit(9) == 1
    }

    /// Draw pixels to display if true.
    fn draw_to_display(self) -> bool {
        self.0.extract_bit(10) == 1
    }

    /// Set the mask bit of each pixel when writing to VRAM.
    fn set_mask_bit(self) -> bool {
        self.0.extract_bit(11) == 1
    }

    /// Draw pixels with mask bit set if true.
    fn draw_masked_pixels(self) -> bool {
        self.0.extract_bit(12) == 1
    }

    /// The interlace field currently being displayed.
    fn interlace_fields(self) -> InterlaceField {
        InterlaceField::from_value(self.0.extract_bit(13) == 1)
    }

    /// ?
    fn reversed(self) -> bool {
        self.0.extract_bit(14) == 1
    }

    fn texture_disabled(self) -> bool {
        self.0.extract_bit(15) == 1
    }

    fn horizontal_res(self) -> u32 {
        self.0.extract_bits(16, 18)
    }

    fn vertical_res(self) -> u32 {
        240 * (self.0.extract_bit(19) + 1)
    }

    fn video_mode(self) -> VideoMode {
        VideoMode::from_value(self.0.extract_bit(20) == 1)
    }

    /// Depth of each pixel being drawn.
    fn color_depth(self) -> ColorDepth {
        ColorDepth::from_value(self.0.extract_bit(21) == 1)
    }

    /// Draw interlaced instead of progressive.
    fn vertical_interlace_enabled(self) -> bool {
        self.0.extract_bit(22) == 1
    }

    fn display_enabled(self) -> bool {
        self.0.extract_bit(23) == 1
    }

    fn interrupt_request_enabled(self) -> bool {
        self.0.extract_bit(24) == 1
    }

    // DMA request.

    /// Ready to recieve commands.
    fn cmd_ready(self) -> bool {
        self.0.extract_bit(26) == 1
    }

    /// Ready to transfer from vram to CPU/Memory.
    fn vram_to_cpu_ready(self) -> bool {
        self.0.extract_bit(27) == 1
    }

    /// Ready to do DMA block transfer.
    fn dma_block_ready(self) -> bool {
        self.0.extract_bit(28) == 1
    }

    /// Direction of DMA request.
    fn dma_direction(self) -> DmaDirection {
        DmaDirection::from_value(self.0.extract_bits(29, 30))
    }
}

/// Number of words in each GP0 command.
const GP0_CMD_LEN: [u8; 0x100] = [
    1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    4, 4, 4, 4, 7, 7, 7, 7, 5, 5, 5, 5, 9, 9, 9, 9, 6, 6, 6, 6, 9, 9, 9, 9, 8, 8, 8, 8, 12, 12, 12,
    12, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 3, 3, 3, 3, 4, 4, 4, 4, 2, 2, 2, 2, 3, 3, 3, 3, 2, 2, 2, 2, 3, 3, 3, 3, 2, 2, 2, 2, 3, 3,
    3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1,
];

pub struct Gpu {
    fifo: Fifo,
    vram: Vram,
    status: Status,
    /// Mirros textured rectangles on the x axis if true,
    rect_tex_x_flip: bool,
    /// Mirros textured rectangles on the y axis if true,
    rect_tex_y_flip: bool,
    /// Texture window x mask.
    tex_window_x_mask: u8,
    /// Texture window y mask.
    tex_window_y_mask: u8,
    /// Texture window x offset.
    tex_window_x_offset: u8,
    /// Texture window y offset.
    tex_window_y_offset: u8,
    draw_area_left: u16,
    draw_area_right: u16,
    draw_area_top: u16,
    draw_area_bottom: u16,
    draw_x_offset: i16,
    draw_y_offset: i16,
    /// The first column display area in VRAM.
    display_vram_x_start: u16,
    /// The first line display area in VRAM.
    display_vram_y_start: u16,
    display_column_start: u16,
    display_column_end: u16,
    display_line_start: u16,
    display_line_end: u16,
}

impl Gpu {
    pub fn new() -> Self {
        // Sets the reset values.
        Self {
            fifo: Fifo::new(),
            vram: Vram::new(),
            status: Status(0x14802000),
            rect_tex_x_flip: false,
            rect_tex_y_flip: false,
            tex_window_x_mask: 0x0,
            tex_window_y_mask: 0x0,
            tex_window_x_offset: 0x0,
            tex_window_y_offset: 0x0,
            draw_area_left: 0x0,
            draw_area_right: 0x0,
            draw_area_top: 0x0,
            draw_area_bottom: 0x0,
            draw_x_offset: 0x0,
            draw_y_offset: 0x0,
            display_vram_x_start: 0x0,
            display_vram_y_start: 0x0,
            display_column_start: 0x200,
            display_column_end: 0xc00,
            display_line_start: 0x10,
            display_line_end: 0x100,
        }
    }

    pub fn store(&mut self, offset: u32, value: u32) {
        match offset {
            0 => self.gp0_store(value),
            4 => self.gp1_store(value),
            _ => unimplemented!("Invalid GPU store at offset {:08x}.", offset),
        }
    }

    pub fn load(&mut self, _offset: u32) -> u32 {
        0
    }

    /// Store command in GP0 register. This is called from DMA linked transfer directly.
    pub fn gp0_store(&mut self, value: u32) {
        self.fifo.push(value);
        let len = GP0_CMD_LEN[self.fifo[0].extract_bits(24, 31) as usize];
        if self.fifo.len() == len as usize {
            self.gp0_exec(value);
            self.fifo.clear();
        }
    }

    fn gp1_store(&mut self, value: u32) {
        match value.extract_bits(24, 31) {
            0x0 => self.gp1_reset(),
            0x4 => self.gp1_dma_direction(value),
            0x5 => self.gp1_start_display_area(value),
            0x6 => self.gp1_horizontal_display_range(value),
            0x7 => self.gp1_vertical_display_range(value),
            0x8 => self.gp1_display_mode(value),
            _ => unimplemented!("Invalid GP1 command {:08x}.", value),
        }
    }

    fn gp0_exec(&mut self, value: u32) {
        let command = self.fifo[0].extract_bits(24, 31);
        // println!("{:08x}", command);
        match command{
            0x0 => {}
            0xe1 => self.gp0_draw_mode_setting(value),
            0xe2 => self.gp0_texture_window_setting(value),
            0xe3 => self.gp0_draw_area_top_left(value),
            0xe4 => self.gp0_draw_area_bottom_right(value),
            0xe5 => self.gp0_draw_offset(value),
            0xe6 => self.gp0_mask_bit_setting(value),
            // Draw commands.
            0x28 => self.gp0_four_point_poly(),
            _ => unimplemented!("Invalid GP0 command {:08x}.", command),
        }
    }

    fn gp0_three_point_poly(&mut self) {
        let mut verts = [Vertex::default(); 3];
        for vertex in verts.iter_mut() {
            vertex.point = Point::from_cmd(self.fifo.pop());
            vertex.point.x += i32::from(self.draw_x_offset);
            vertex.point.y += i32::from(self.draw_y_offset);
        }
        self.draw_triangle(&verts[0], &verts[1], &verts[2]);
    }

    fn gp0_four_point_poly(&mut self) {
        let mut verts = [Vertex::default(); 4];
        for vertex in verts.iter_mut() {
            vertex.point = Point::from_cmd(self.fifo.pop());
            vertex.point.x += i32::from(self.draw_x_offset);
            vertex.point.y += i32::from(self.draw_y_offset);
        }
        self.draw_triangle(&verts[0], &verts[1], &verts[2]);
        self.draw_triangle(&verts[1], &verts[2], &verts[3]);
    }

    /// [GP0 - Draw Mode Setting] - Set various flags in the GPU.
    ///  - [0..10] - Same as status register.
    ///  - [11] - Texture disabled.
    ///  - [12] - Texture rectangle x-flip.
    ///  - [13] - Texture rectangle y-flip.
    ///  - [14..23] - Not used.
    fn gp0_draw_mode_setting(&mut self, value: u32) {
        self.status.0 |= value.extract_bits(0, 10);
        self.status.0 |= value.extract_bit(11) << 15;
        self.rect_tex_x_flip = value.extract_bit(12) == 1;
        self.rect_tex_x_flip = value.extract_bit(13) == 1;
    }

    //p [GP0 - Texture window setting].
    ///  - [0..4] - Texture window mask x.
    ///  - [5..9] - Texture window mask y.
    ///  - [10..14] - Texture window offset x.
    ///  - [15..19] - Texture window offset y.
    ///  - [20..23] - Not used.
    fn gp0_texture_window_setting(&mut self, value: u32) {
        self.tex_window_x_mask = value.extract_bits(0, 4) as u8;
        self.tex_window_y_mask = value.extract_bits(5, 9) as u8;
        self.tex_window_x_offset = value.extract_bits(10, 14) as u8;
        self.tex_window_y_offset = value.extract_bits(15, 19) as u8;
    }

    /// [GP0 - Mask bit setting] - Do/don't set the mask bit while drawing and do/don't check
    /// before drawing.
    ///  - [0] - Set mask while drawing.
    ///  - [1] - Check mask before drawing.
    ///  - [2..23] - Not used.
    fn gp0_mask_bit_setting(&mut self, value: u32) {
        self.status.0 |= value.extract_bit(0) << 11;
        self.status.0 |= value.extract_bit(1) << 12;
    }

    /// [GP0 - Set draw area top left].
    /// - [0..9] - Draw area left.
    /// - [10..18] - Draw area top.
    /// TODO this differs between GPU versions.
    fn gp0_draw_area_top_left(&mut self, value: u32) {
        self.draw_area_left = value.extract_bits(0, 9) as u16;
        self.draw_area_top = value.extract_bits(10, 18) as u16;
    }

    /// [GP0 - Set draw area bottom right].
    /// - [0..9] - Draw area right.
    /// - [10..18] - Draw area bottom.
    /// TODO this differs between GPU versions.
    fn gp0_draw_area_bottom_right(&mut self, value: u32) {
        self.draw_area_right = value.extract_bits(0, 9) as u16;
        self.draw_area_bottom = value.extract_bits(10, 18) as u16;
    }

    /// [GP0 - Set drawing offset].
    ///  - [0..10] - x-offset.
    ///  - [11..21] - y-offset.
    ///  - [24..23] - Not used.
    fn gp0_draw_offset(&mut self, value: u32) {
        let x_offset = value.extract_bits(0, 10) as u16;
        let y_offset = value.extract_bits(11, 21) as u16;
        // Because the command stores the values as 11 bit signed integers, the values have to be
        // bit-shifted to the most significant bits in order to make Rust generate sign extension.
        self.draw_x_offset = ((x_offset << 5) as i16) >> 5;
        self.draw_y_offset = ((y_offset << 5) as i16) >> 5;
    }

    /// [GP1 - Reset] - Resets the state of the GPU.
    fn gp1_reset(&mut self) {
        *self = Self::new();
        // TODO Flush FIFO.
    }

    /// [GP1 - DMA Direction] - Sets the DMA direction.
    ///  - [0..1] - DMA direction.
    ///  - [2..23] - Not used.
    fn gp1_dma_direction(&mut self, value: u32) {
        self.status.0 |= value.extract_bits(0, 1) << 29;
    }

    /// [GP1 - Start display area in VRAM] - What area of the VRAM to display.
    ///  - [0..9] - x (address in VRAM).
    ///  - [10..18] - y (address in VRAM).
    ///  - [19..23] - Not used.
    fn gp1_start_display_area(&mut self, value: u32) {
        self.display_vram_x_start = value.extract_bits(0, 9) as u16;
        self.display_vram_y_start = value.extract_bits(10, 18) as u16;
    }

    /// [GP1 - Horizontal display range] - Sets the vertical range of the display area in screen.
    ///  - [0..11] - column start.
    ///  - [12..23] - column end.
    fn gp1_horizontal_display_range(&mut self, value: u32) {
        self.display_column_start = value.extract_bits(0, 11) as u16;
        self.display_column_end = value.extract_bits(12, 23) as u16;
    }

    /// [GP1 - Vertical display range] - Sets the horizontal range of the display area in screen.
    ///  - [0..11] - line start.
    ///  - [12..23] - line end.
    fn gp1_vertical_display_range(&mut self, value: u32) {
        self.display_line_start = value.extract_bits(0, 11) as u16;
        self.display_line_end = value.extract_bits(12, 23) as u16;
    }

    /// [GP1 - Display Mode] - Sets display mode, video mode, resolution and interlacing.
    ///  - [0..1] - Horizontal resolution 1.
    ///  - [2] - Vertical resolution.
    ///  - [3] - Display mode.
    ///  - [4] - Display area color depth.
    ///  - [5] - Horizontal interlace.
    ///  - [6] - Horizontal resolution 2.
    ///  - [7] - Reverseflag.
    ///  - [8..23] - Not used.
    fn gp1_display_mode(&mut self, value: u32) {
        self.status.0 |= value.extract_bits(0, 5) << 17;
        self.status.0 |= value.extract_bit(6) << 16;
        self.status.0 |= value.extract_bit(7) << 14;
    }
}
