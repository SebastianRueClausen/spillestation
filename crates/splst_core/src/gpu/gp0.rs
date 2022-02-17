use splst_util::{Bit, BitSet};

use crate::Cycle;
use crate::timing;

use super::{Gpu, State, MemTransfer};
use super::primitive::{Vertex, Point, Color, TexCoord};

impl Gpu {
    /// Represent multiple commands which does nothing.
    pub fn gp0_nop(&mut self) {
        self.fifo.pop(); 
    }

    // GP0(01) - Clear texture cache.
    pub fn gp0_clear_texture_cache(&mut self) {
        self.fifo.pop();

        // TODO: Clear texture cache.
    }

    // GP0(02) - Fill rectanlge in VRAM.
    pub fn gp0_fill_rect(&mut self) {
        let color = Color::from_cmd(self.fifo.pop());

        let val = self.fifo.pop() as i32;
        let start = Point {
            x: val & 0x3f0,
            y: (val >> 16) & 0x3ff,
        };

        let val = self.fifo.pop() as i32;
        let dim = Point {
            x: ((val & 0x3ff) + 0xf) & !0xf,
            y: (val >> 16) & 0x1ff,
        };

        self.fill_rect(start, dim, color); 
    }

    // GP0(e1) - Draw Mode Setting.
    // - 0..10 - Same as status register.
    // - 11 - Texture disabled.
    // - 12 - Texture rectangle x-flip.
    // - 13 - Texture rectangle y-flip.
    // - 14..23 - Not used.
    pub fn gp0_draw_mode(&mut self) {
        let val = self.fifo.pop();
        let stat = val.bit_range(0, 10);

        self.status.0 = self.status.0
            .set_bit_range(0, 10, stat)
            .set_bit(15, val.bit(11));

        self.tex_x_flip = val.bit(12);
        self.tex_y_flip = val.bit(13);
         
    }

    // GP0(e2) - Texture window setting.
    // - 0..4 - Texture window mask x.
    // - 5..9 - Texture window mask y.
    // - 10..14 - Texture window offset x.
    // - 15..19 - Texture window offset y.
    pub fn gp0_texture_window_settings(&mut self) {
        let val = self.fifo.pop();

        self.tex_win_w = val.bit_range(0, 4) as u8;
        self.tex_win_h = val.bit_range(5, 9) as u8;
        self.tex_win_x = val.bit_range(10, 14) as u8;
        self.tex_win_y = val.bit_range(15, 19) as u8;
    }

    // GP0(e3) - Set draw area top left.
    // - 0..9 - Draw area left.
    // - 10..18 - Draw area top.
    pub fn gp0_draw_area_top_left(&mut self) {
        let val = self.fifo.pop();

        // TODO: This differs between GPU versions.
        self.da_x_min = val.bit_range(0, 9) as i32;
        self.da_y_min = val.bit_range(10, 18) as i32;
    }

    // GP0(e4) - Set draw area bottom right.
    // - 0..9 - Draw area right.
    // - 10..18 - Draw area bottom.
    pub fn gp0_draw_area_bottom_right(&mut self) {
        let val = self.fifo.pop();

        self.da_x_max = val.bit_range(0, 9) as i32;
        self.da_y_max = val.bit_range(10, 18) as i32;
    }

    // GP0(e5) - Set drawing offset.
    // - 0..10 - x-offset.
    // - 11..21 - y-offset.
    // - 24..23 - Not used.
    pub fn gp0_draw_offset(&mut self) {
        let val = self.fifo.pop();

        let x_offset = val.bit_range(0, 10) as u16;
        let y_offset = val.bit_range(11, 21) as u16;

        // Because the command stores the values as 11 bit signed integers, the values have to be
        // bit-shifted to the most significant bits in order to make Rust generate sign extension.
        self.x_offset = ((x_offset << 5) as i16) >> 5;
        self.y_offset = ((y_offset << 5) as i16) >> 5;
    }

    // GP0(e6) - Mask bit setting.
    // - 0 - Set mask while drawing.
    // - 1 - Check mask before drawing.
    pub fn gp0_mask_bit_setting(&mut self) {
        let val = self.fifo
            .pop()
            .bit_range(0, 1);

        self.status.0 = self.status.0.set_bit_range(11, 12, val);
    }

    // GP0(a0) - Copy rectangle from CPU to VRAM.
    pub fn gp0_copy_rect_cpu_to_vram(&mut self) {
        self.fifo.pop();
        let (pos, dim) = (self.fifo.pop(), self.fifo.pop());

        let (x, y, w, h) = (
            pos.bit_range(00, 15) as i32,
            pos.bit_range(16, 31) as i32,
            dim.bit_range(00, 15) as i32,
            dim.bit_range(16, 31) as i32,
        );

        let x = x & 0x3ff;
        let y = y & 0x1ff;

        let w = ((w - 1) & 0x3ff) + 1;
        let h = ((h - 1) & 0x1ff) + 1;

        self.state = State::VramStore(MemTransfer::new(x, y, w, h));
    }

    // GP0(c0) - Copy rectanlge from VRAM to CPU.
    pub fn gp0_copy_rect_vram_to_cpu(&mut self) {
        self.fifo.pop();
        let (pos, dim) = (self.fifo.pop(), self.fifo.pop());

        let (x, y, w, h) = (
            pos.bit_range(00, 15) as i32,
            pos.bit_range(16, 31) as i32,
            dim.bit_range(00, 15) as i32,
            dim.bit_range(16, 31) as i32,
        );

        let x = x & 0x3ff;
        let y = y & 0x1ff;

        let w = ((w - 1) & 0x3ff) + 1;
        let h = ((h - 1) & 0x1ff) + 1;

        self.state = State::VramLoad(MemTransfer::new(x, y, w, h));
    }

    /// Handle GP0 triangle polygon commands.
    pub fn gp0_tri_poly<Shade, Tex, Trans>(&mut self) -> Cycle
    where
        Shade: draw_mode::Shading,
        Tex: draw_mode::Textureing,
        Trans: draw_mode::Transparency,
    {
        let mut verts = [Vertex::default(); 3];
        let mut clut = Point::default();

        let color = match Shade::IS_SHADED {
            true => Color::from_rgb(0, 0, 0),
            false => Color::from_cmd(self.fifo.pop()),
        };

        for (i, vertex) in verts.iter_mut().enumerate() {
            if Shade::IS_SHADED {
                vertex.color = Color::from_cmd(self.fifo.pop());
            }

            let pos = self.fifo.pop();

            vertex.point = Point::from_cmd(pos).with_offset(
                self.x_offset as i32,
                self.y_offset as i32,
            );

            if Tex::IS_TEXTURED {
                let val = self.fifo.pop();
                match i {
                    0 => {
                        clut.x = val.bit_range(16, 21) as i32 * 16;
                        clut.y = val.bit_range(22, 30) as i32;
                    }
                    1 => {
                        let val = val >> 16;

                        self.status.0 = self.status.0
                            .set_bit_range(0, 8, val.bit_range(0, 8))
                            .set_bit(11, val.bit(11));
                    }
                    _ => {}
                }

                vertex.texcoord = TexCoord {
                    u: val.bit_range(0, 7) as u8,
                    v: val.bit_range(8, 15) as u8,
                };
            }
        }

        let cycles = self.draw_triangle::<Shade, Tex, Trans>(
            color, clut, &verts[0], &verts[1], &verts[2]
        );

        timing::gpu_to_cpu_cycles(cycles)
    }

    /// Handle GP0 quad(Four point) polygon command.
    pub fn gp0_quad_poly<Shade, Tex, Trans>(&mut self) -> Cycle
    where
        Shade: draw_mode::Shading,
        Tex: draw_mode::Textureing,
        Trans: draw_mode::Transparency,
    {
        let mut verts = [Vertex::default(); 4];
        let mut clut = Point::default();

        let color = match Shade::IS_SHADED {
            true => Color::from_rgb(0, 0, 0),
            false => Color::from_cmd(self.fifo.pop()),
        };

        for (i, vertex) in verts.iter_mut().enumerate() {
            // If it's shaded the color is always the first attribute.
            if Shade::IS_SHADED {
                vertex.color = Color::from_cmd(self.fifo.pop());
            }

            let pos = self.fifo.pop();

            vertex.point = Point::from_cmd(pos).with_offset(
                self.x_offset as i32,
                self.y_offset as i32,
            );

            if Tex::IS_TEXTURED {
                let val = self.fifo.pop();
                match i {
                    0 => {
                        clut.x = val.bit_range(16, 21) as i32 * 16;
                        clut.y = val.bit_range(22, 30) as i32;
                    }
                    1 => {
                        let val = val >> 16;

                        self.status.0 = self.status.0
                            .set_bit_range(0, 8, val.bit_range(0, 8))
                            .set_bit(11, val.bit(11));
                    }
                    _ => {}
                }

                vertex.texcoord = TexCoord {
                    u: val.bit_range(0, 7) as u8,
                    v: val.bit_range(8, 15) as u8,
                };
            }
        }

        let tri1 = self.draw_triangle::<Shade, Tex, Trans>(
            color, clut, &verts[0], &verts[1], &verts[2],
        );

        let tri2 = self.draw_triangle::<Shade, Tex, Trans>(
            color, clut, &verts[1], &verts[2], &verts[3]
        );

        timing::gpu_to_cpu_cycles(tri1 + tri2)
    }

    /// Handle GP0 line commands.
    pub fn gp0_line<Shade, Trans>(&mut self) -> Cycle
    where
        Shade: draw_mode::Shading,
        Trans: draw_mode::Transparency
    {
        let color = match Shade::IS_SHADED {
            false => Color::from_cmd(self.fifo.pop()),
            true => Color::from_rgb(0, 0, 0),
        };

        let start = Point::from_cmd(self.fifo.pop()).with_offset(
            self.x_offset as i32,
            self.y_offset as i32,
        );

        let end = Point::from_cmd(self.fifo.pop()).with_offset(
            self.x_offset as i32,
            self.y_offset as i32,
        );

        let cycles = self.draw_line::<Shade, Trans>(start, end, color);

        timing::gpu_to_cpu_cycles(cycles)
    }

    /// Handle GP0 rectangle commands.
    pub fn gp0_rect<Tex, Tran>(&mut self, size: Option<i32>) -> Cycle
    where
        Tex: draw_mode::Textureing,
        Tran: draw_mode::Transparency,
    {
        let mut uv = TexCoord::default();
        let mut clut = Point::default();

        let color = Color::from_cmd(self.fifo.pop());

        let start = Point::from_cmd(self.fifo.pop()).with_offset(
            self.x_offset as i32,
            self.y_offset as i32,
        );

        if Tex::IS_TEXTURED {
            let val = self.fifo.pop();

            uv.u = val.bit_range(0, 07) as u8;
            uv.v = val.bit_range(8, 15) as u8;

            clut.x = val.bit_range(16, 21) as i32 * 16;
            clut.y = val.bit_range(22, 30) as i32;
        }

        let dim = match size {
            Some(s) => Point::new(s, s),
            None => Point::from_cmd(self.fifo.pop()),
        };

        let cycles = self.draw_rect::<Tex, Tran>(start, dim, color, uv, clut);

        timing::gpu_to_cpu_cycles(cycles)
    }
}

pub mod draw_mode {
    //! Type parameters for draw commands.

    /// The shading mode of a draw call.
    pub trait Shading {
        const IS_SHADED: bool;
    }

    /// Not shaded ie. each vertex doesn't have a color attribute.
    pub struct UnShaded;

    impl Shading for UnShaded {
        const IS_SHADED: bool = false;
    }

    /// Shaded i.e. each vertex has a color attribute. The colors get's interpolated between each
    /// vertex using linear interpolation.
    pub struct Shaded;

    impl Shading for Shaded {
        const IS_SHADED: bool = true;
    }

    /// The texture mode of a draw call.
    pub trait Textureing {
        const IS_TEXTURED: bool;
        const IS_RAW: bool;
    }

    /// The shap is only colored by shading.
    pub struct UnTextured;

    impl Textureing for UnTextured {
        const IS_TEXTURED: bool = false;
        const IS_RAW: bool = false;
    }

    /// The shape is textured and get's blended with with shading.
    pub struct Textured;

    impl Textureing for Textured {
        const IS_TEXTURED: bool = true;
        const IS_RAW: bool = false;
    }

    /// The shape is textured and doesn't get blended with shading.
    pub struct TexturedRaw;

    impl Textureing for TexturedRaw {
        const IS_TEXTURED: bool = true;
        const IS_RAW: bool = true;
    }

    /// The transparency mode of a draw call, basically how the color of a shape get's blended with the
    /// background color.
    pub trait Transparency {
        const IS_TRANSPARENT: bool;
    }

    /// The shape is transparent or semi-transparent, which means the color of the shape get's blended
    /// with the backgroud color.
    pub struct Transparent;

    impl Transparency for Transparent {
        const IS_TRANSPARENT: bool = true;
    }

    /// The shape is opaque and doesn't get blended with the background.
    pub struct Opaque;

    impl Transparency for Opaque {
        const IS_TRANSPARENT: bool = false;
    }
}
