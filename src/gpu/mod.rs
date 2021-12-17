//! Represent's the GPU of the Playstation 1.

mod fifo;
mod primitive;
mod rasterize;
pub mod vram;

use crate::front::DrawInfo;
use crate::util::bits::BitExtract;
use fifo::Fifo;
use primitive::{Color, Point, TexCoord, TextureParams, Vertex};
use rasterize::{
    Opaque, Shaded, Shading, Textured, Textureing, Transparency, UnShaded, UnTextured,
};
use std::fmt;
pub use vram::Vram;

/// How to blend two colors. Used mainly for blending the color of a shape being drawn with the color
/// behind it.
pub enum TransBlend {
    Avg = 0,
    Add = 1,
    Sub = 2,
    AddDiv = 3,
}

impl TransBlend {
    fn from_value(value: u32) -> Self {
        match value {
            0 => TransBlend::Avg,
            1 => TransBlend::Add,
            2 => TransBlend::Sub,
            3 => TransBlend::AddDiv,
            _ => unreachable!("Invalid transparency blending"),
        }
    }

    pub fn blend(self, a: Color, b: Color) -> Color {
        match self {
            TransBlend::Avg => a.avg_blend(b),
            TransBlend::Add => a.add_blend(b),
            TransBlend::Sub => a.sub_blend(b),
            TransBlend::AddDiv => a.add_div_blend(b),
        }
    }
}

impl fmt::Display for TransBlend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                TransBlend::Avg => "average",
                TransBlend::Add => "add",
                TransBlend::Sub => "subtract",
                TransBlend::AddDiv => "add and divide",
            }
        )
    }
}

/// The frequency of the frames being drawn to the screen.
pub enum VideoMode {
    /// 60 Hz.
    Ntsc = 60,
    /// 50 Hz.
    Pal = 50,
}

impl fmt::Display for VideoMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                VideoMode::Ntsc => "NTSC(60hz)",
                VideoMode::Pal => "PAL(50hz)",
            }
        )
    }
}

pub enum DmaDirection {
    Off = 0,
    Fifo = 1,
    CpuToGp0 = 2,
    VramToCpu = 3,
}

impl fmt::Display for DmaDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                DmaDirection::Off => "off",
                DmaDirection::Fifo => "Fifo",
                DmaDirection::CpuToGp0 => "CPU to GP0",
                DmaDirection::VramToCpu => "VRAM to CPU",
            }
        )
    }
}

/// Interlace output split into two fields.
pub enum InterlaceField {
    Bottom = 0,
    Top = 1,
}

impl fmt::Display for InterlaceField {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                InterlaceField::Bottom => "top/even",
                InterlaceField::Top => "bottom/odd",
            }
        )
    }
}

/// Number of bits used to represent 1 pixel.
pub enum ColorDepth {
    B15 = 15,
    B24 = 24,
}

impl fmt::Display for ColorDepth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                ColorDepth::B15 => "15 bit",
                ColorDepth::B24 => "24 bit",
            }
        )
    }
}

/// Number of bits used to represent 1 texture pixel.
pub enum TextureDepth {
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

impl fmt::Display for TextureDepth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                TextureDepth::B4 => "4 bit",
                TextureDepth::B8 => "8 bit",
                TextureDepth::B15 => "15 bit",
            }
        )
    }
}

/// An ongoing CPU to Vram transfer. Data is recieved via DMA transfers, and the GPU is paused
/// waiting for the data.
#[derive(Clone, Copy)]
struct VramTransfer {
    /// The current x coordinate.
    pub x: i32,
    /// The current y coordinate.
    pub y: i32,
    /// The first x coordinate.
    x_start: i32,
    /// The last x coordinate.
    x_end: i32,
    /// The last y coordinate.
    y_end: i32,
}

impl VramTransfer {
    fn new(x_start: i32, y_start: i32, width: i32, height: i32) -> Self {
        Self {
            x: x_start,
            y: y_start,
            x_start,
            x_end: x_start + width,
            y_end: y_start + height,
        }
    }

    fn next(&mut self) -> Option<()> {
        self.x += 1;
        if self.x == self.x_end {
            self.x = self.x_start;
            self.y += 1;
            if self.y == self.y_end {
                return None;
            }
        }
        Some(())
    }
}

/// Current state of the GPU.
#[derive(Clone, Copy)]
enum GpuState {
    WaitForData(VramTransfer),
    WaitForCmd,
}

/// Status register of the GPU.
#[derive(Clone, Copy)]
pub struct Status(u32);

impl Status {
    /// Texture page x base coordinate. N * 64.
    pub fn texture_page_x_base(self) -> u32 {
        self.0.extract_bits(0, 3) * 64
    }

    /// Texture page y base coordinate. N * 256.
    pub fn texture_page_y_base(self) -> u32 {
        self.0.extract_bit(4) * 256
    }

    /// How to blend source and destination colors.
    pub fn trans_blending(self) -> TransBlend {
        TransBlend::from_value(self.0.extract_bits(5, 6))
    }

    /// Depth of the texture colors.
    pub fn texture_depth(self) -> TextureDepth {
        TextureDepth::from_value(self.0.extract_bits(7, 8))
    }

    pub fn dithering_enabled(self) -> bool {
        self.0.extract_bit(9) == 1
    }

    /// Draw pixels to display if true.
    pub fn draw_to_display(self) -> bool {
        self.0.extract_bit(10) == 1
    }

    /// Set the mask bit of each pixel when writing to VRAM.
    pub fn set_mask_bit(self) -> bool {
        self.0.extract_bit(11) == 1
    }

    /// Draw pixels with mask bit set if true.
    pub fn draw_masked_pixels(self) -> bool {
        self.0.extract_bit(12) == 1
    }

    /// The interlace field currently being displayed.
    pub fn interlace_field(self) -> InterlaceField {
        match self.0.extract_bit(13) {
            0 => InterlaceField::Bottom,
            1 => InterlaceField::Top,
            _ => unreachable!("Invalid interlace field."),
        }
    }

    #[allow(dead_code)]
    fn reversed(self) -> bool {
        self.0.extract_bit(14) == 1
    }

    pub fn texture_disabled(self) -> bool {
        self.0.extract_bit(15) == 1
    }

    pub fn horizontal_res(self) -> u32 {
        match self.0.extract_bit(16) {
            1 => 368,
            _ => match self.0.extract_bits(17, 18) {
                0 => 256,
                1 => 480,
                2 => 512,
                3 => 640,
                _ => unreachable!("Invalid vres."),
            },
        }
    }

    pub fn vertical_res(self) -> u32 {
        240 * (self.0.extract_bit(19) + 1)
    }

    pub fn video_mode(self) -> VideoMode {
        match self.0.extract_bit(20) {
            0 => VideoMode::Ntsc,
            1 => VideoMode::Pal,
            _ => unreachable!("Invalid video mode."),
        }
    }

    /// Depth of each pixel being drawn.
    pub fn color_depth(self) -> ColorDepth {
        match self.0.extract_bit(21) {
            0 => ColorDepth::B15,
            1 => ColorDepth::B24,
            _ => unreachable!("Invalid color depth."),
        }
    }

    /// Draw interlaced instead of progressive.
    pub fn vertical_interlace_enabled(self) -> bool {
        self.0.extract_bit(22) == 1
    }

    pub fn display_enabled(self) -> bool {
        self.0.extract_bit(23) == 1
    }

    pub fn interrupt_request_enabled(self) -> bool {
        self.0.extract_bit(24) == 1
    }

    // DMA request.

    /// Ready to recieve commands.
    pub fn cmd_ready(self) -> bool {
        self.0.extract_bit(26) == 1
    }

    /// Ready to transfer from vram to CPU/Memory.
    pub fn vram_to_cpu_ready(self) -> bool {
        self.0.extract_bit(27) == 1
    }

    /// Ready to do DMA block transfer.
    pub fn dma_block_ready(self) -> bool {
        self.0.extract_bit(28) == 1
    }

    /// Direction of DMA request.
    pub fn dma_direction(self) -> DmaDirection {
        match self.0.extract_bits(29, 30) {
            0 => DmaDirection::Off,
            1 => DmaDirection::Fifo,
            2 => DmaDirection::CpuToGp0,
            3 => DmaDirection::VramToCpu,
            _ => unreachable!("Invalid dma direction"),
        }
    }
}

pub struct Gpu {
    /// The current state of the GPU.
    state: GpuState,
    /// Fifo command buffer.
    fifo: Fifo,
    /// Video RAM.
    vram: Vram,
    /// To CPU Transfer.
    to_cpu_transfer: Option<VramTransfer>,
    /// The status register.
    pub status: Status,
    /// Mirros textured rectangles on the x axis if true,
    rect_tex_x_flip: bool,
    /// Mirros textured rectangles on the y axis if true,
    #[allow(dead_code)]
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
    pub draw_x_offset: i16,
    pub draw_y_offset: i16,
    /// The first column display area in VRAM.
    pub display_vram_x_start: u16,
    /// The first line display area in VRAM.
    pub display_vram_y_start: u16,
    pub display_column_start: u16,
    pub display_column_end: u16,
    pub display_line_start: u16,
    pub display_line_end: u16,
}

impl Gpu {
    pub fn new() -> Self {
        // Sets the reset values.
        Self {
            state: GpuState::WaitForCmd,
            fifo: Fifo::new(),
            vram: Vram::new(),
            to_cpu_transfer: None,
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

    pub fn load(&mut self, offset: u32) -> u32 {
        match offset {
            0 => self.gpu_read(),
            4 => self.status_read(),
            _ => unreachable!("Invalid GPU load at offset {:08x}.", offset),
        }
    }

    pub fn dma_store(&mut self, value: u32) {
        self.gp0_store(value);
    }

    fn load_from_transfer(&mut self) -> u16 {
        if let Some(ref mut transfer) = self.to_cpu_transfer {
            let value = self.vram.load_16(transfer.x, transfer.y);
            if transfer.next().is_none() {
                self.to_cpu_transfer = None;
            }
            value
        } else {
            // TODO: This should return GPU info generated from GP1(10).
            0
        }
    }

    fn gpu_read(&mut self) -> u32 {
        let low = self.load_from_transfer() as u32;
        let high = self.load_from_transfer() as u32;
        (high << 16) | low
    }

    fn status_read(&mut self) -> u32 {
        self.status.0 & !(1 << 19) | ((self.to_cpu_transfer.is_some() as u32) << 27)
    }

    pub fn dma_load(&mut self) -> u32 {
        self.gpu_read()
    }

    /// Store command in GP0 register. This is called from DMA linked transfer directly.
    pub fn gp0_store(&mut self, value: u32) {
        match self.state {
            GpuState::WaitForCmd => {
                self.fifo.push(value);
                if self.fifo.has_full_cmd() {
                    self.gp0_exec();
                    self.fifo.clear();
                }
            }
            GpuState::WaitForData(ref mut transfer) => {
                for (lo, hi) in [(0, 15), (16, 31)] {
                    let value = value.extract_bits(lo, hi) as u16;
                    self.vram.store_16(transfer.x, transfer.y, value);
                    if transfer.next() == None {
                        self.state = GpuState::WaitForCmd;
                        break;
                    }
                }
            }
        }
    }

    pub fn vram(&self) -> &Vram {
        &self.vram
    }

    pub fn draw_info(&self) -> DrawInfo {
        DrawInfo {
            x_start: self.display_vram_x_start as u32,
            y_start: self.display_vram_y_start as u32,
        }
    }

    fn gp1_store(&mut self, value: u32) {
        let command = value.extract_bits(24, 31);
        match command {
            0x0 => self.gp1_reset(),
            0x3 => self.gp1_display_enable(value),
            0x4 => self.gp1_dma_direction(value),
            0x5 => self.gp1_start_display_area(value),
            0x6 => self.gp1_horizontal_display_range(value),
            0x7 => self.gp1_vertical_display_range(value),
            0x8 => self.gp1_display_mode(value),
            _ => unimplemented!("Invalid GP1 command {:08x}.", command),
        }
    }

    fn gp0_exec(&mut self) {
        let command = self.fifo[0].extract_bits(24, 31);
        match command {
            0x0 => {}
            0x1 => {
                // TODO: clear cache.
            }
            0xe1 => self.gp0_draw_mode_setting(),
            0xe2 => self.gp0_texture_window_setting(),
            0xe3 => self.gp0_draw_area_top_left(),
            0xe4 => self.gp0_draw_area_bottom_right(),
            0xe5 => self.gp0_draw_offset(),
            0xe6 => self.gp0_mask_bit_setting(),
            // Draw commands.
            0x28 => self.gp0_four_point_poly::<UnShaded, UnTextured, Opaque>(),
            0x2c => self.gp0_four_point_poly::<UnShaded, Textured, Opaque>(),
            0x30 => self.gp0_three_point_poly::<Shaded, UnTextured, Opaque>(),
            0x38 => self.gp0_four_point_poly::<Shaded, UnTextured, Opaque>(),
            // Opaque no shading.
            0x44 => self.gp0_line(),
            // Copy react command.
            0xa0 => self.gp0_copy_rect_cpu_to_vram(),
            0xc0 => self.gp0_copy_rect_vram_to_cpu(),
            _ => unimplemented!("Invalid GP0 command {:08x}.", command),
        }
    }

    fn gp0_three_point_poly<S: Shading, Tex: Textureing, Trans: Transparency>(&mut self) {
        let mut verts = [Vertex::default(); 3];
        let mut params = TextureParams::default();
        let mut color = Color::from_rgb(0, 0, 0);
        if !S::is_shaded() {
            color = Color::from_u32(self.fifo.pop());
        }
        for (i, vertex) in verts.iter_mut().enumerate() {
            if S::is_shaded() {
                vertex.color = Color::from_u32(self.fifo.pop());
            }
            vertex.point = Point::from_u32(self.fifo.pop());
            vertex.point.x += self.draw_x_offset as i32;
            vertex.point.y += self.draw_y_offset as i32;
            if Tex::is_textured() {
                let value = self.fifo.pop();
                match i {
                    0 => {
                        params.clut_x = (value.extract_bits(16, 21) / 16) as i32;
                        params.clut_y = value.extract_bits(22, 30) as i32;
                    }
                    1 => {
                        params.texture_x = value.extract_bits(16, 19) as i32 * 64;
                        params.texture_y = value.extract_bit(20) as i32 * 256;
                        params.blend_mode = TransBlend::from_value(value.extract_bits(21, 22));
                        params.texture_depth = TextureDepth::from_value(value.extract_bits(23, 24));
                    }
                    _ => {}
                }
                vertex.texcoord = TexCoord {
                    u: value.extract_bits(0, 7) as u8,
                    v: value.extract_bits(8, 15) as u8,
                };
            }
        }
        self.draw_triangle::<S, Tex, Trans>(color, &params, &verts[0], &verts[1], &verts[2]);
    }

    fn gp0_four_point_poly<S: Shading, Tex: Textureing, Trans: Transparency>(&mut self) {
        let mut verts = [Vertex::default(); 4];
        let mut params = TextureParams::default();
        let color = if !S::is_shaded() {
            Color::from_u32(self.fifo.pop())
        } else {
            Color::from_rgb(0, 0, 0)
        };
        for (i, vertex) in verts.iter_mut().enumerate() {
            // If it's shaded the color is always the first.
            if S::is_shaded() {
                vertex.color = Color::from_u32(self.fifo.pop());
            }
            vertex.point = Point::from_u32(self.fifo.pop());
            vertex.point.x += self.draw_x_offset as i32;
            vertex.point.y += self.draw_y_offset as i32;
            if Tex::is_textured() {
                let value = self.fifo.pop();
                match i {
                    0 => {
                        params.clut_x = value.extract_bits(16, 21) as i32 * 16;
                        params.clut_y = value.extract_bits(22, 30) as i32;
                    }
                    1 => {
                        params.texture_x = value.extract_bits(16, 19) as i32 * 64;
                        params.texture_y = value.extract_bit(20) as i32 * 256;
                        params.blend_mode = TransBlend::from_value(value.extract_bits(21, 22));
                        params.texture_depth = TextureDepth::from_value(value.extract_bits(23, 24));
                    }
                    _ => {}
                }
                vertex.texcoord = TexCoord {
                    u: value.extract_bits(0, 7) as u8,
                    v: value.extract_bits(8, 15) as u8,
                };
            }
        }
        if Tex::is_textured() {
            // println!("pal x = {} pal y = {} tex x = {} tex y = {}", params.clut_x, params.clut_y, params.texture_x, params.texture_y);
            for _vert in verts {
                // println!("{:?}", vert.texcoord);
            }
        }
        self.draw_triangle::<S, Tex, Trans>(color, &params, &verts[0], &verts[1], &verts[2]);
        self.draw_triangle::<S, Tex, Trans>(color, &params, &verts[1], &verts[2], &verts[3]);
    }

    fn gp0_line(&mut self) {
        self.fifo.pop();
        let mut start = Point::from_u32(self.fifo.pop());
        start.x += self.draw_x_offset as i32;
        start.y += self.draw_y_offset as i32;
        let mut end = Point::from_u32(self.fifo.pop());
        end.x += self.draw_x_offset as i32;
        end.y += self.draw_y_offset as i32;
        self.draw_line(start, end);
        println!("Drawing line");
    }

    /// [GP0 - Draw Mode Setting] - Set various flags in the GPU.
    ///  - [0..10] - Same as status register.
    ///  - [11] - Texture disabled.
    ///  - [12] - Texture rectangle x-flip.
    ///  - [13] - Texture rectangle y-flip.
    ///  - [14..23] - Not used.
    fn gp0_draw_mode_setting(&mut self) {
        let value = self.fifo.pop();
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
    fn gp0_texture_window_setting(&mut self) {
        let value = self.fifo.pop();
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
    fn gp0_mask_bit_setting(&mut self) {
        let value = self.fifo.pop();
        self.status.0 |= value.extract_bit(0) << 11;
        self.status.0 |= value.extract_bit(1) << 12;
    }

    /// [GP0 - Set draw area top left].
    /// - [0..9] - Draw area left.
    /// - [10..18] - Draw area top.
    /// TODO this differs between GPU versions.
    fn gp0_draw_area_top_left(&mut self) {
        let value = self.fifo.pop();
        self.draw_area_left = value.extract_bits(0, 9) as u16;
        self.draw_area_top = value.extract_bits(10, 18) as u16;
    }

    /// [GP0 - Set draw area bottom right].
    /// - [0..9] - Draw area right.
    /// - [10..18] - Draw area bottom.
    /// TODO this differs between GPU versions.
    fn gp0_draw_area_bottom_right(&mut self) {
        let value = self.fifo.pop();
        self.draw_area_right = value.extract_bits(0, 9) as u16;
        self.draw_area_bottom = value.extract_bits(10, 18) as u16;
    }

    /// [GP0 - Set drawing offset].
    ///  - [0..10] - x-offset.
    ///  - [11..21] - y-offset.
    ///  - [24..23] - Not used.
    fn gp0_draw_offset(&mut self) {
        let value = self.fifo.pop();
        let x_offset = value.extract_bits(0, 10) as u16;
        let y_offset = value.extract_bits(11, 21) as u16;
        // Because the command stores the values as 11 bit signed integers, the values have to be
        // bit-shifted to the most significant bits in order to make Rust generate sign extension.
        self.draw_x_offset = ((x_offset << 5) as i16) >> 5;
        self.draw_y_offset = ((y_offset << 5) as i16) >> 5;
    }

    fn gp0_copy_rect_cpu_to_vram(&mut self) {
        self.fifo.pop();
        let (pos, dim) = (self.fifo.pop(), self.fifo.pop());
        let x = pos.extract_bits(00, 15) as i32;
        let y = pos.extract_bits(16, 31) as i32;
        let w = dim.extract_bits(00, 15) as i32;
        let h = dim.extract_bits(16, 31) as i32;
        self.state = GpuState::WaitForData(VramTransfer::new(x, y, w, h));
    }

    fn gp0_copy_rect_vram_to_cpu(&mut self) {
        self.fifo.pop();
        let (pos, dim) = (self.fifo.pop(), self.fifo.pop());
        let x = pos.extract_bits(00, 15) as i32;
        let y = pos.extract_bits(16, 31) as i32;
        let w = dim.extract_bits(00, 15) as i32;
        let h = dim.extract_bits(16, 31) as i32;
        self.to_cpu_transfer = Some(VramTransfer::new(x, y, w, h));
    }

    /// [GP1 - Reset] - Resets the state of the GPU.
    fn gp1_reset(&mut self) {
        *self = Self::new();
    }

    /// [GP1 - DMA Direction] - Sets the DMA direction.
    ///  - [0..1] - DMA direction.
    ///  - [2..23] - Not used.
    fn gp1_dma_direction(&mut self, value: u32) {
        self.status.0 |= value.extract_bits(0, 1) << 29;
    }

    /// [GP1 - Display Enable] - Toggles display enable bit.
    ///  - [0] - Display On/Off.
    fn gp1_display_enable(&mut self, value: u32) {
        self.status.0 |= value.extract_bit(0) << 23;
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
