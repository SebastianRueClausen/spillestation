use super::{Gpu, TextureDepth};
use super::primitive::{Color, Texel, Point, TexCoord, Vertex, TextureParams};
use ultraviolet::vec::Vec3;

pub trait Shading {
    fn is_shaded() -> bool;
}

pub struct UnShaded;

impl Shading for UnShaded {
    fn is_shaded() -> bool {
        false
    }
}

pub struct Shaded;

impl Shading for Shaded {
    fn is_shaded() -> bool {
        true
    }
}

pub trait Textureing {
    fn is_textured() -> bool;
    fn is_raw() -> bool;
}

pub struct UnTextured;

impl Textureing for UnTextured {
    fn is_textured() -> bool {
        false
    }
    
    fn is_raw() -> bool {
        false
    }
}

pub struct Textured;

impl Textureing for Textured {
    fn is_textured() -> bool {
        true
    }

    fn is_raw() -> bool {
        false
    }
}

pub struct TexturedRaw;

impl Textureing for TexturedRaw {
    fn is_textured() -> bool {
        true
    }

    fn is_raw() -> bool {
        true
    }
}

pub trait Transparency {
    fn is_transparent() -> bool;
}

pub struct Transparent;

impl Transparency for Transparent {
    fn is_transparent() -> bool {
        true
    }
}

pub struct Opaque;

impl Transparency for Opaque {
    fn is_transparent() -> bool {
        false
    }
}

pub fn barycentric(points: &[Point; 3], p: &Point) -> Vec3 {
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

impl Gpu {
    fn draw_pixel(&mut self, point: Point, color: Color) {
        self.vram.store_16(point.x, point.y, color.as_u16());
    }

    fn load_texture_color(&self, params: &TextureParams, coord: TexCoord) -> Texel {
        match self.status.texture_depth() {
            TextureDepth::B4 => {
                let value = self.vram.load_16(
                    params.texture_x + (coord.u >> 2) as i32,
                    params.texture_y + coord.v as i32,
                );
                let offset = (value >> ((coord.u & 0x3) << 2)) & 0xf;
                Texel::new(self.vram.load_16(params.clut_x + offset as i32, params.clut_y))
            },
            TextureDepth::B8 => {
                let value = self.vram.load_16(
                    params.texture_x + (coord.u >> 1) as i32,
                    params.texture_y + coord.v as i32,
                );
                let offset = (value >> ((coord.u & 0x1) << 3)) & 0xff;
                Texel::new(self.vram.load_16(params.clut_x + offset as i32, params.clut_y))
            },
            TextureDepth::B15 => {
                let value = self.vram.load_16(
                    params.texture_x + coord.u as i32,
                    params.texture_y + coord.v as i32,
                );
                Texel::new(value)
            },
        }
    }

    pub fn draw_triangle<S: Shading, Tex: Textureing, Trans: Transparency>(
        &mut self,
        shade: Color,
        params: &TextureParams,
        v1: &Vertex,
        v2: &Vertex,
        v3: &Vertex,
    ) {
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
        // Rasterize.
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let p = Point::new(x, y);
                let res = barycentric(&points, &p);
                if res.x < 0.0 || res.y < 0.0 || res.z < 0.0 {
                    continue;
                }
                let color = if S::is_shaded() {
                    let r = v1.color.r as f32 * res.x + v2.color.r as f32 * res.y + v3.color.r as f32 * res.z;
                    let g = v1.color.g as f32 * res.x + v2.color.g as f32 * res.y + v3.color.g as f32 * res.z;
                    let b = v1.color.b as f32 * res.x + v2.color.b as f32 * res.y + v3.color.b as f32 * res.z;
                    Color::from_rgb(r as u8, g as u8, b as u8)
                } else {
                    shade 
                };
                let color = if Tex::is_textured() {
                    let uv = TexCoord {
                        u: (v1.texcoord.u as f32 * res.x + v2.texcoord.u as f32 * res.y + v3.texcoord.u as f32 * res.z) as u8,
                        v: (v1.texcoord.v as f32 * res.x + v2.texcoord.v as f32 * res.y + v3.texcoord.v as f32 * res.z) as u8,
                    };
                    let texel = self.load_texture_color(params, uv);
                    let color = if Tex::is_raw() {
                        texel.as_color()
                    } else {
                        let color = texel.as_color();
                        let r = (color.r as u16) * (shade.r as u16);
                        let g = (color.g as u16) * (shade.g as u16);
                        let b = (color.b as u16) * (shade.b as u16);
                        Color {
                            r: (r / 0x80).min(0xff) as u8,
                            g: (g / 0x80).min(0xff) as u8,
                            b: (b / 0x80).min(0xff) as u8,
                        }
                    };
                    if Trans::is_transparent() && texel.is_transparent() {
                        let current_color = Color::from_u16(
                            self.vram.load_16(p.x, p.y)
                        );

                    }
                    color
                } else {
                   color 
                };
                self.draw_pixel(p, color);
            }
        }
    }

    /*
    pub fn draw_triangle_block(&mut self, v1: &Vertex, v2: &Vertex, v3: &Vertex) {
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
        if max.x - min.x > 60 || max.y - min.y > 60 {
            for y in (min.y..=max.y).step_by(8) {
                for x in (min.x..=max.x).step_by(8) {
                    let ps = [
                        Point::new(x, y),
                        Point::new(x + 8, y),
                        Point::new(x + 8, y + 8),
                        Point::new(x, y + 8),
                    ];
                    let rs = [
                        Point::barycentric(&points, &ps[0]),
                        Point::barycentric(&points, &ps[1]),
                        Point::barycentric(&points, &ps[2]),
                        Point::barycentric(&points, &ps[3]),
                    ];
                    if rs.iter().all(|v| v.x < 0.0 || v.y < 0.0 || v.z < 0.0) {
                        for y in y..(y + 8) {
                            for x in x..(x + 8) {
                                self.draw_pixel(Point::new(x, y), Color::from_rgb(255, 255, 255));
                            }
                        }
                    } else if !rs.iter().any(|v| v.x < 0.0 || v.y < 0.0 || v.z < 0.0) {
                        continue;
                    } else {
                        for y in y..(y + 8) {
                            for x in x..(x + 8) {
                                let p = Point::new(x, y);
                                let res = Point::barycentric(&points, &p);
                                if res.x < 0.0 || res.y < 0.0 || res.z < 0.0 {
                                    continue;
                                }
                                self.draw_pixel(p, Color::from_rgb(255, 255, 255));
                            }
                        }
                    }
                }
            }
        } else {
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let p = Point {
                        x, y,
                    };
                    let res = Point::barycentric(&points, &p);
                    if res.x < 0.0 || res.y < 0.0 || res.z < 0.0 {
                        continue;
                    }
                    // TODO: Color lerp.
                    // TODO: Texture lerp.
                    self.draw_pixel(p, Color::from_rgb(255, 255, 255));
                }
            }
        }
    }
    */

    pub fn draw_line(&mut self, _start: Point, _end: Point) {
    }
}
