use super::primitive::{Color, Point, TexCoord, Texel, PolyVertex, LineVertex};
use super::{Gpu, TexelDepth};
use super::gp0::draw_mode;
use crate::Cycle;

use ultraviolet::vec::Vec3;

impl Gpu {
    /// Draw a single pixel to the screen. It handles transparency and texture but not dithering.
    fn draw_pixel<Tran, Tex>(&mut self, x: i32, y: i32, color: Color, masked: bool)
    where
        Tran: draw_mode::Transparency,
        Tex: draw_mode::Textureing
    {
        let color = match Tran::IS_TRANSPARENT {
            false => color,
            true => {
                let bg = Color::from_u16(self.vram.load_16(x, y));
                match Tex::IS_TEXTURED {
                    true => {
                        if masked {
                            self.status.blend_mode().blend(color, bg)
                        } else {
                            color
                        }
                    }
                    false => {
                        self.status.blend_mode().blend(color, bg)
                    }
                }
            }
        };
        self.vram.store_16(x, y, color.as_u16());
    }

    /// Load a texel at a given texture coordinate.
    fn load_texel(
        &self,
        clut: Point,
        coord: TexCoord,
        tex_param_cache: TexParamCache
    ) -> Texel {
        let u = (coord.u & tex_param_cache.tex_win_u) as i32;
        let v = (coord.v & tex_param_cache.tex_win_v) as i32;
       
        match self.status.texture_depth() {
            TexelDepth::B4 => {
                let val = self.vram.load_16(
                    self.status.tex_page_x() + (u / 4),
                    self.status.tex_page_y() + v,
                );

                let offset = (val >> ((u & 3) * 4)) as i32 & 0xf;

                Texel::new(self.vram.load_16(clut.x + offset, clut.y))
            }
            TexelDepth::B8 => {
                let val = self.vram.load_16(
                    self.status.tex_page_x() + (u / 2),
                    self.status.tex_page_y() + v,
                );

                let offset = (val >> ((u & 1) * 8)) as i32 & 0xff;

                Texel::new(self.vram.load_16(clut.x + offset, clut.y))
            }
            TexelDepth::B15 => {
                let val = self.vram.load_16(
                    self.status.tex_page_x() + u,
                    self.status.tex_page_y() + v,
                );

                Texel::new(val)
            }
        }
    }

    /// Calculate the amount of GPU cycles to draw a triangle.
    fn triangle_draw_time<Shade, Tex, Trans>(&self, mut pixels: u64) -> Cycle
    where
        Shade: draw_mode::Shading,
        Tex: draw_mode::Textureing,
        Trans: draw_mode::Transparency,
    {
        let mut cycles = 0;

        // Start by calculating the constant factor.

        if Shade::IS_SHADED {
            cycles += 300;
        }

        if Tex::IS_TEXTURED {
            cycles += 150;
        }

        let mut pixel_cost = 1.0;

        if Tex::IS_TEXTURED || Shade::IS_SHADED {
            pixel_cost += 0.8;
        }

        if Trans::IS_TRANSPARENT {
            pixel_cost += 0.3;
        }

        // Hack for now until we emulate only drawing to not displayed lines.
        if !self.status.draw_to_display() && self.status.interlaced_480() {
            pixels /= 2;
        }

        cycles + (pixels as f64 * pixel_cost) as Cycle
    }

    /// Timings from mednafen.
    fn _triangle_draw_time_mdnf<Shade, Tex, Trans>(&self, pixels: u64) -> Cycle
    where
        Shade: draw_mode::Shading,
        Tex: draw_mode::Textureing,
        Trans: draw_mode::Transparency,
    {
        let mut draw_time = 0;

        draw_time += if Shade::IS_SHADED {
            if Tex::IS_TEXTURED {
                150 * 3
            } else {
                96 * 3
            }
        } else if Tex::IS_TEXTURED {
            60 * 3 
        } else {
            0
        };

        draw_time += if Tex::IS_TEXTURED || Shade::IS_SHADED {
            pixels * 2
        } else if Trans::IS_TRANSPARENT || !self.status.draw_masked_pixels() {
            (pixels / 2) * 3
        } else {
            pixels
        };

        if !self.status.draw_to_display() && self.status.interlaced_480() {
            draw_time /= 2;
        }

        draw_time
    }

    /// The amount of GPU cycles to draw a rectangle.
    fn rect_draw_time<Tex, Trans>(&self, mut pixels: u64) -> Cycle
    where
        Tex: draw_mode::Textureing,
        Trans: draw_mode::Transparency,
    {
        let cycles = 30;

        let mut pixel_cost = 1.0;

        if Tex::IS_TEXTURED {
            pixel_cost += 0.4;
        }

        if Trans::IS_TRANSPARENT {
            pixel_cost += 0.2;
        }

        if !self.status.draw_to_display() && self.status.interlaced_480() {
            pixels /= 2;
        }

        cycles + (pixels as f64 * pixel_cost) as Cycle
    }

    /// Amount of GPU cycles to draw a line.
    fn line_draw_time<Shade, Trans>(&self, mut pixels: u64) -> Cycle
    where
        Shade: draw_mode::Shading,
        Trans: draw_mode::Transparency,
    {
        let cycles = 30;
        let mut pixel_cost = 1.0;

        if Shade::IS_SHADED {
            pixel_cost += 0.5;
        }

        if Trans::IS_TRANSPARENT {
            pixel_cost += 0.5;
        }

        if !self.status.draw_to_display() && self.status.interlaced_480() {
            pixels /= 2;
        }

        cycles + (pixels as f64 * pixel_cost) as Cycle
    }


    /// Rasterize a triangle to the screen. It finds the bounding box and checks for each pixel if
    /// it's inside the triangle using barycentric coordinates.    ///
    ///
    /// This could be optimized in a few different ways. Most obviously using simd to rasterize
    /// multiple pixels at once.
    ///
    /// TODO: Add support for drawing only to non-interlaced fields.
    pub fn draw_triangle<Shade, Tex, Trans>(
        &mut self,
        shade: Color,
        clut: Point,
        v1: &PolyVertex,
        v2: &PolyVertex,
        v3: &PolyVertex,
    ) -> Cycle
    where
        Shade: draw_mode::Shading,
        Tex: draw_mode::Textureing,
        Trans: draw_mode::Transparency,
    {
        let tex_param_cache = TexParamCache::new(
            self.tex_win_w,
            self.tex_win_h,
            self.tex_win_x,
            self.tex_win_y,
        );

        let points = [v1.point, v2.point, v3.point];

        // Calculate bounding box. The GPU doesn't draw the bottom or right most pixels.
        let bb_max = Point {
            x: i32::max(points[0].x, i32::max(points[1].x, points[2].x)) - 1,
            y: i32::max(points[0].y, i32::max(points[1].y, points[2].y)) - 1,
        };

        let bb_min = Point {
            x: i32::min(points[0].x, i32::min(points[1].x, points[2].x)),
            y: i32::min(points[0].y, i32::min(points[1].y, points[2].y)),
        };

        // Clip screen bounds.
        let min = Point {
            x: i32::max(bb_min.x, self.da_x_min),
            y: i32::max(bb_min.y, self.da_y_min),
        };

        let max = Point {
            x: i32::min(bb_max.x, self.da_x_max + 1),
            y: i32::min(bb_max.y, self.da_y_max + 1),
        };

        // This is to keep track of how many pixels gets drawn to calculate timing.
        let mut pixels_drawn: u64 = 0;

        // Loop through all points in the bounding box, and draw the pixel if it's inside the
        // triangle.
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let res = barycentric(&points, x, y);

                if res.x < 0.0 || res.y < 0.0 || res.z < 0.0 {
                    continue;
                }

                // If the triangle is shaded, we interpolate between the colors of each vertex.
                // Otherwise the shade is just the base color / shade.
                let shade = if Shade::IS_SHADED {
                    let r = v1.color.r as f32 * res.x
                        + v2.color.r as f32 * res.y
                        + v3.color.r as f32 * res.z;

                    let g = v1.color.g as f32 * res.x
                        + v2.color.g as f32 * res.y
                        + v3.color.g as f32 * res.z;

                    let b = v1.color.b as f32 * res.x
                        + v2.color.b as f32 * res.y
                        + v3.color.b as f32 * res.z;

                    Color::from_rgb(r as u8, g as u8, b as u8)
                } else {
                    shade
                };

                let (color, masked) = if Tex::IS_TEXTURED {
                    let u = v1.texcoord.u as f32 * res.x
                        + v2.texcoord.u as f32 * res.y
                        + v3.texcoord.u as f32 * res.z;

                    let v = v1.texcoord.v as f32 * res.x
                        + v2.texcoord.v as f32 * res.y
                        + v3.texcoord.v as f32 * res.z;

                    let uv = TexCoord {
                        u: u as u8,
                        v: v as u8,
                    };

                    let texel = self.load_texel(clut, uv, tex_param_cache);

                    if texel.is_invisible() {
                        continue;
                    }

                    // If the triangle is not textured raw, the texture color get's blended with the
                    // shade. Otherwise it doesn't.
                    let color = match Tex::IS_RAW {
                        true => texel.as_color(),
                        false => texel.as_color().shade_blend(shade),
                    };

                    (color, texel.is_transparent())
                } else {
                    (shade, false)
                };


                let color = match self.status.dithering_enabled() {
                    true => color.dither(x, y),
                    false => color,
                };

                pixels_drawn += 1;
                self.draw_pixel::<Trans, Tex>(x, y, color, masked);
            }
        }

        self.triangle_draw_time::<Shade, Tex, Trans>(pixels_drawn)
    }

    /// Clamp a point to the draw area.
    fn clamp_to_da(&self, point: Point) -> Point {
        Point {
            x: point.x.clamp(self.da_x_min, self.da_x_max),
            y: point.y.clamp(self.da_y_min, self.da_y_max),
        }
    }

    /// Draw line using bresenham algorithm.
    ///
    /// TODO: Redo all of this.
    pub fn draw_line<Shade, Trans>(
        &mut self,
        mut start: LineVertex,
        mut end: LineVertex,
        shade: Color
    ) -> Cycle
    where
        Shade: draw_mode::Shading,
        Trans: draw_mode::Transparency,
    {
        start.point = self.clamp_to_da(start.point);
        end.point = self.clamp_to_da(end.point);

        let dx = end.point.x - start.point.x;
        let dy = end.point.y - start.point.y;

        let abs_dx = dx.abs();
        let abs_dy = dy.abs();

        let longest = abs_dx.max(abs_dy) as u8;

        // Color delta values.
        // FIXME: Pretty sure this is wrong.
        let (dr, dg, db) = match Shade::IS_SHADED {
            false => (0, 0, 0),
            true => (
                end.color.r.abs_diff(start.color.r) / longest,
                end.color.g.abs_diff(start.color.g) / longest,
                end.color.b.abs_diff(start.color.b) / longest,
            ),
        };

        // Keep track of position.
        let mut x = start.point.x;
        let mut y = start.point.y;

        // Keep track of color values. Only used if the line is shaded.
        let (mut r, mut g, mut b) = (
            start.color.r,
            start.color.g,
            start.color.b,
        );

        // Lines are always dithered.
        self.draw_pixel::<Trans, draw_mode::UnTextured>(
            x, y, shade.dither(x, y), false
        );

        let mut pixels_drawn = 1;

        if abs_dx > abs_dy {
            let mut d = 2 * abs_dy - abs_dx; 

            for _ in 0..abs_dx {
                x = if dx < 0 { x - 1 } else { x + 1 };

                if d < 0 {
                    d += 2 * abs_dy; 
                } else {
                    y = if dy < 0 { y - 1 } else { y + 1 };
                    d += 2 * abs_dy - 2 * abs_dx;
                }

                let color = match Shade::IS_SHADED {
                    false => shade,
                    true => {
                        r = r.wrapping_add(dr);
                        g = g.wrapping_add(dg);
                        b = b.wrapping_add(db);

                        Color::from_rgb(r, g, b)
                    }
                };

                pixels_drawn += 1;
            
                self.draw_pixel::<Trans, draw_mode::UnTextured>(
                    x, y, color.dither(x, y), false
                );
            }
        } else {
            let mut d = 2 * abs_dx - abs_dy;

            for _ in 0..abs_dy {
                y = if dy < 0 { y - 1 } else { y + 1 };

                if d < 0 {
                    d += 2 * abs_dx; 
                } else {
                    x = if dx < 0 { x - 1 } else { x + 1 };
                    d += 2 * abs_dx - 2 * abs_dy;
                }

                let color = match Shade::IS_SHADED {
                    false => shade,
                    true => {
                        r = r.wrapping_add(dr);
                        g = g.wrapping_add(dg);
                        b = b.wrapping_add(db);

                        Color::from_rgb(r, g, b)
                    }
                };

                pixels_drawn += 1;

                self.draw_pixel::<Trans, draw_mode::UnTextured>(
                    x, y, color.dither(x, y), false
                );
            }
        }

        self.line_draw_time::<Shade, Trans>(pixels_drawn)
    }

    /// Fill rectangle in VRAM with a solid color.
    pub fn fill_rect(&mut self, start: Point, dim: Point, color: Color) {
        let color = color.as_u16();
        for y in 0..dim.y {
            for x in 0..dim.x {
                self.vram.store_16(start.x + x, start.y + y, color);
            }
        }
    }

    pub fn draw_rect<Tex, Trans>(
        &mut self,
        start: Point,
        dim: Point,
        shade: Color,
        mut tc_start: TexCoord,
        clut: Point,
    ) -> Cycle
    where
        Tex: draw_mode::Textureing,
        Trans: draw_mode::Transparency,
    {
        let tex_param_cache = TexParamCache::new(
            self.tex_win_w,
            self.tex_win_h,
            self.tex_win_x,
            self.tex_win_y,
        );

        // Calculate the uv delta for each step in x and y direction. Nocash specifies that the
        // texture flipping for rectangles doesn't work on older Playstation models, but older
        // games shouldn't be using the feature anyway.
        let (u_delta, v_delta) = match Tex::IS_TEXTURED {
            false => (0, 0),
            true => (
                if self.tex_x_flip { -1 } else { 1 },
                if self.tex_y_flip { -1 } else { 1 },
            ),
        };

        // Clip to left and bottom draw area limits.
        let end_x = i32::min(start.x + dim.x, self.da_x_max + 1);
        let end_y = i32::min(start.y + dim.y, self.da_y_max + 1);

        // Clip to right draw area limit.
        let start_x = if start.x < self.da_x_min.into() {
            if Tex::IS_TEXTURED {
                let delta = (self.da_x_min - start.x) * u_delta;
                tc_start.u = tc_start.u.wrapping_add(delta as u8);
            }
            self.da_x_min.into()
        } else {
            start.x 
        };

        // Clip to top draw area limit.
        let start_y = if start.y < self.da_y_min.into() {
            if Tex::IS_TEXTURED {
                let delta = (self.da_y_min - start.y) * v_delta;
                tc_start.v = tc_start.v.wrapping_add(delta as u8);
            }
            self.da_y_min.into()
        } else {
            start.y 
        };

        let mut tc = tc_start;

        let mut pixels_drawn = 0;

        for y in start_y..end_y {
            tc.u = tc_start.u;

            for x in start_x..end_x {
                let (color, masked) = if Tex::IS_TEXTURED {
                    let texel = self.load_texel(clut, tc, tex_param_cache);

                    if texel.is_invisible() {
                        tc.u = tc.u.wrapping_add(u_delta as u8);
                        continue;
                    }

                    let tex_color = match Tex::IS_RAW {
                        true => texel.as_color(),
                        false => texel.as_color().shade_blend(shade),
                    };

                    let color = match Trans::IS_TRANSPARENT {
                        false => tex_color,
                        true => {
                            let bg = Color::from_u16(self.vram.load_16(x, y));
                            self.status.blend_mode().blend(tex_color, bg)
                        }
                    };

                    (color, texel.is_transparent())
                } else {
                    (shade, false)
                };

                pixels_drawn += 1;
                self.draw_pixel::<Trans, Tex>(x, y, color, masked);

                tc.u = tc.u.wrapping_add(u_delta as u8);
            }
            tc.v = tc.v.wrapping_add(v_delta as u8);
        }

        self.rect_draw_time::<Tex, Trans>(pixels_drawn)
    }
}

#[inline]
fn barycentric(points: &[Point; 3], x: i32, y: i32) -> Vec3 {
    let v1 = Vec3::new(
        (points[2].x - points[0].x) as f32,
        (points[1].x - points[0].x) as f32,
        (points[0].x - x) as f32,
    );

    let v2 = Vec3::new(
        (points[2].y - points[0].y) as f32,
        (points[1].y - points[0].y) as f32,
        (points[0].y - y) as f32,
    );

    let u = v1.cross(v2);

    if u.z.abs() < 1.0 {
        Vec3::new(-1.0, 1.0, 1.0)
    } else {
        Vec3::new(1.0 - (u.x + u.y) / u.z, u.y / u.z, u.x / u.z)
    }
}

/// Cache for static info used to render each textured pixel.
#[derive(Clone, Copy)]
struct TexParamCache {
    /// !(self.tex_win_w * 8) | ((self.tex_win_x & self.tex_win_w) * 8).
    tex_win_u: u8,
    /// !(self.tex_win_h * 8) | ((self.tex_win_y & self.tex_win_h) * 8).
    tex_win_v: u8,
}

impl TexParamCache {
    fn new(
        tex_win_w: u8,
        tex_win_h: u8,
        tex_win_x: u8,
        tex_win_y: u8,
    ) -> Self {
        Self {
            tex_win_u: !(tex_win_w * 8) | ((tex_win_x & tex_win_w) * 8),
            tex_win_v: !(tex_win_h * 8) | ((tex_win_y & tex_win_h) * 8),
        }
    }
}
