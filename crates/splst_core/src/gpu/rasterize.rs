use super::primitive::{Color, Point, TexCoord, Texel, TextureParams, Vertex};
use super::{Gpu, TexelDepth};
use crate::Cycle;

use ultraviolet::vec::Vec3;

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

impl Gpu {
    /// Draw a pixel to the screen.
    fn draw_pixel(&mut self, point: Point, color: Color) {
        self.vram.store_16(point.x, point.y, color.as_u16());
    }

    /// Load a texel at a given texture coordinate.
    fn load_texel(&self, params: &TextureParams, coord: TexCoord) -> Texel {
        match params.texel_depth {
            TexelDepth::B4 => {
                let value = self.vram.load_16(
                    params.texture_x + (coord.u / 4) as i32,
                    params.texture_y + coord.v as i32,
                );
                let offset = (value >> ((coord.u & 0x3) * 4)) as i32 & 0xf;
                Texel::new(self.vram.load_16(params.clut_x + offset, params.clut_y))
            }
            TexelDepth::B8 => {
                let value = self.vram.load_16(
                    params.texture_x + (coord.u / 2) as i32,
                    params.texture_y + coord.v as i32,
                );
                let offset = (value >> ((coord.u & 0x1) * 8)) as i32 & 0xff;
                Texel::new(self.vram.load_16(params.clut_x + offset, params.clut_y))
            }
            TexelDepth::B15 => {
                let value = self.vram.load_16(
                    params.texture_x + coord.u as i32,
                    params.texture_y + coord.v as i32,
                );
                Texel::new(value)
            }
        }
    }

    // Calculate the amount of cycles to draw a triangle.
    fn calc_triangle_draw_time<Shade, Tex, Trans>(&self, pixels_drawn: u64) -> Cycle
    where
        Shade: Shading,
        Tex: Textureing,
        Trans: Transparency,
    {
        // First of all there is a constant factor for shading and texturing, likely just from
        // decoding the command. Shading and texturing spend some time fetching texels and background
        // colors, which could depend on the texture cache, which seems pretty easy to emulate when
        // that get's implemented.
        //
        // TODO: How much time does transparency take?
        let cycles = match (Shade::IS_SHADED, Tex::IS_TEXTURED) {
            (true, true) => 500 + pixels_drawn * 2,
            (false, true) => 300 + pixels_drawn * 2,
            (true, false) => 180 + pixels_drawn * 2,
            (false, false) => {
                if Trans::IS_TRANSPARENT {
                    (pixels_drawn * 3) / 2
                } else {
                    pixels_drawn
                }
            }
        };
        if !self.status.draw_to_displayed() && self.status.interlaced480() {
            cycles / 2
        } else {
            cycles
        }
    }

    /// Rasterize a triangle to the screen. It finds the bounding box and checks for each pixel if
    /// it's inside the triangle using barycentric coordinates. Since the Playstation renders
    /// many different kind triangles, this function takes template arguments descriping how the
    /// triangle should be rendered, to avoid a lot of run-time branching. Colors and texture coordinates
    /// get interpolated using the barycentric coordinates.
    ///
    /// This could be optimized in a few different ways. Most obviously using simd to rasterize
    /// multiple pixels at once.
    ///
    /// TODO: Add support for drawing only to non-interlaced fields.
    pub fn draw_triangle<Shade: Shading, Tex: Textureing, Trans: Transparency>(
        &mut self,
        shade: Color,
        params: &TextureParams,
        v1: &Vertex,
        v2: &Vertex,
        v3: &Vertex,
    ) -> Cycle {
        let points = [v1.point, v2.point, v3.point];
        // Calculate bounding box.
        let max = Point {
            x: i32::max(points[0].x, i32::max(points[1].x, points[2].x)),
            y: i32::max(points[1].y, i32::max(points[1].y, points[2].y)),
        };
        let min = Point {
            x: i32::min(points[0].x, i32::min(points[1].x, points[2].x)),
            y: i32::min(points[0].y, i32::min(points[1].y, points[2].y)),
        };
        // Clip screen bounds.
        let max = Point {
            x: i32::max(max.x, self.draw_area_right as i32),
            y: i32::max(max.y, self.draw_area_top as i32),
        };
        let min = Point {
            x: i32::min(min.x, self.draw_area_left as i32),
            y: i32::min(min.y, self.draw_area_bottom as i32),
        };
        // This is to keep track of how many pixels gets drawn to calculate timing.
        let mut pixels_drawn: u64 = 0;
        // Loop through all points in the bounding box, and draw the pixel if it's inside the
        // triangle.
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let p = Point::new(x, y);
                let res = barycentric(&points, &p);
                if res.x < 0.0 || res.y < 0.0 || res.z < 0.0 {
                    continue;
                }
                // If the triangle is shaded, we interpolate between the colors of each vertex.
                // Otherwise the shade is just the base color/shade.
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

                let color = if Tex::IS_TEXTURED {
                    let uv = TexCoord {
                        u: (v1.texcoord.u as f32 * res.x
                            + v2.texcoord.u as f32 * res.y
                            + v3.texcoord.u as f32 * res.z) as u8,
                        v: (v1.texcoord.v as f32 * res.x
                            + v2.texcoord.v as f32 * res.y
                            + v3.texcoord.v as f32 * res.z) as u8,
                    };
                    let texel = self.load_texel(params, uv);
                    if texel.is_invisible() {
                        continue;
                    }
                    // If the triangle is not textured raw, the texture color get's blended with the
                    // shade. Otherwise it doesn't.
                    let texture_color = if Tex::IS_RAW {
                        texel.as_color()
                    } else {
                        texel.as_color().shade_blend(shade)
                    };
                    // If both the triangle and the texel is transparent, the texture color
                    // get's blended with the background using the blending function specified in
                    // the status register.
                    if Trans::IS_TRANSPARENT && texel.is_transparent() {
                        let background = Color::from_u16(self.vram.load_16(p.x, p.y));
                        params.blend_mode.blend(texture_color, background)
                        /*
                        self.status
                            .trans_blending()
                            .blend(texture_color, background)
                        */
                    } else {
                        texture_color
                    }
                } else {
                    // If the triangle isn't textured, but transparent, the shade get's blended with
                    // the background color.
                    if Trans::IS_TRANSPARENT {
                        let background = Color::from_u16(self.vram.load_16(p.x, p.y));
                        self.status.trans_blending().blend(shade, background)
                    } else {
                        shade
                    }
                };
                let color = if self.status.dithering_enabled() {
                    color.dither(p)
                } else {
                    color
                };
                pixels_drawn += 1;
                self.draw_pixel(p, color);
            }
        }
        self.calc_triangle_draw_time::<Shade, Tex, Trans>(pixels_drawn)
    }

    pub fn draw_line(&mut self, _start: Point, _end: Point) {}

    pub fn fill_rect(&mut self, color: Color, start: Point, dim: Point) {
        let color = color.as_u16();
        for y in 0..dim.y {
            for x in 0..dim.x {
                self.vram.store_16(start.x + x, start.y + y, color);
            }
        }
    }
}

/// Calculate barycentric coordinates.
#[inline]
fn barycentric(points: &[Point; 3], p: &Point) -> Vec3 {
    let v1 = Vec3::new(
        (points[2].x - points[0].x) as f32,
        (points[1].x - points[0].x) as f32,
        (points[0].x - p.x) as f32,
    );
    let v2 = Vec3::new(
        (points[2].y - points[0].y) as f32,
        (points[1].y - points[0].y) as f32,
        (points[0].y - p.y) as f32,
    );
    let u = v1.cross(v2);
    if f32::abs(u.z) < 1.0 {
        Vec3::new(-1.0, 1.0, 1.0)
    } else {
        Vec3::new(1.0 - (u.x + u.y) / u.z, u.y / u.z, u.x / u.z)
    }
}
