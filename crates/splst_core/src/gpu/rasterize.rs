use super::primitive::{Color, Point, TexCoord, Texel};
use super::{Gpu, TexelDepth};
use super::gp0::draw_mode;

use std::simd::{f32x4, i32x4, i32x8, f32x8};

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
                    true if masked => self.status.blend_mode().blend(color, bg),
                    true => color,
                    false => self.status.blend_mode().blend(color, bg),
                }
            }
        };
        self.vram.store_16(x, y, color.as_u16());
    }

    /// Load a texel at a given texture coordinate.
    fn load_texel(
        &self,
        coord: TexCoord,
        tex_param_cache: TexParamCache
    ) -> Texel {
        let u = (coord.u & tex_param_cache.tex_win_u) as i32;
        let v = (coord.v & tex_param_cache.tex_win_v) as i32;
       
        match self.status.texture_depth() {
            TexelDepth::B4 => {
                let val = self.vram.load_16(
                    self.status.tex_page_x() + u / 4,
                    self.status.tex_page_y() + v,
                );

                let offset = (val >> ((u & 3) * 4)) as i32 & 0xf;

                self.clut_cache.get(offset)
            }
            TexelDepth::B8 => {
                let val = self.vram.load_16(
                    self.status.tex_page_x() + u / 2,
                    self.status.tex_page_y() + v,
                );

                let offset = (val >> ((u & 1) * 8)) as i32 & 0xff;

                self.clut_cache.get(offset)
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
    fn triangle_draw_time<Shade, Tex, Trans>(&self, mut pixels: u64) -> u64
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

        cycles + (pixels as f64 * pixel_cost) as u64
    }

    /// Timings from mednafen.
    fn _triangle_draw_time_mdnf<Shade, Tex, Trans>(&self, pixels: u64) -> u64
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
    fn rect_draw_time<Tex, Trans>(&self, mut pixels: u64) -> u64
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

        cycles + (pixels as f64 * pixel_cost) as u64
    }

    /// Amount of GPU cycles to draw a line.
    fn line_draw_time<Shade, Trans>(&self, mut pixels: u64) -> u64
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

        cycles + (pixels as f64 * pixel_cost) as u64
    }

    /// FIXME: This may draw outside the draw area.
    pub fn draw_triangle<Shade, Tex, Trans>(
        &mut self,
        flat_shade: Color,
        clut: Point,
        mut points: [Point; 3],
        mut colors: [Color; 3],
        mut coords: [TexCoord; 3],
    ) -> u64
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
        
        if let TexelDepth::B4 | TexelDepth::B8 = self.status.texture_depth() {
            self.clut_cache.maybe_fetch(clut, self.status.texture_depth(), &self.vram);
        }

        // The determinant of a 3x3 matrix of where arranged as:
        //
        //     a.x | b.x | c.x
        //     ----+-----+----
        //     a.y | b.y | c.y
        //     ----+-----+----
        //      1  |  1  |  1
        //
        // This gives a number which represents where the point 'c' in relation to the line from
        // 'a' and 'b'. If 'c' is to the left if that line, the result will be positive and
        // it will be negative on the right.
        fn edge_function(a: Point, b: Point, c: Point) -> i32 {
            (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
        }
        
        // Assure the triangle is wound counter-clockwise, i.e. the vertex 'c' lies to the left of
        // the edge ab. If that isn't the case, vertex 'b' and 'c' must be swapped, which makes it
        // counter-clockwise.
        //
        //  # Example
        //
        //       a            a
        //      / \    ->    / \
        //     b - c        c - d
        //
        if edge_function(points[0], points[1], points[2]) < 0 {
            points.swap(1, 2);
            colors.swap(1, 2);
            coords.swap(1, 2);
        }

        // Check if an edge is the top most edge, and that the edge is 'left' meaning that the end
        // point is lower than the start point.
        //
        //  # Example
        //
        //       a
        //      / \
        //     c - b
        //
        // Here the the edge ab is top left.
        // 
        fn is_top_left(a: Point, b: Point) -> bool {
            let delta = Point {
                x: b.x - a.x,
                y: a.y - b.y,
            };
            // b must be below a or if they are on the same line, or a must be to the right of b.
            delta.y < 0 || (delta.y == 0 && delta.x < 0)
        }

        // Bias for each edge. This is to avoid drawing pixels twice if triangles are next to each-
        // other. Only pixels on the top left edge (explained above) gets drawn. This is also done
        // by real rasterizer.
        let bias = [
            if is_top_left(points[1], points[2]) { 0 } else { -1 },
            if is_top_left(points[2], points[0]) { 0 } else { -1 },
            if is_top_left(points[0], points[1]) { 0 } else { -1 },
        ];
                
        // The double area of the triangle.
        let area = edge_function(points[0], points[1], points[2]) as f32;

        // Find the bounding box of the triangle.
        let bb_max = Point {
            x: i32::max(points[0].x, i32::max(points[1].x, points[2].x)),
            y: i32::max(points[0].y, i32::max(points[1].y, points[2].y)),
        };

        let bb_min = Point {
            x: i32::min(points[0].x, i32::min(points[1].x, points[2].x)),
            y: i32::min(points[0].y, i32::min(points[1].y, points[2].y)),
        };

        // Clip bounding box against screen bounds.
        let max = Point {
            // Since check four pixels at a time, this must be a multiple of 4.
            x: i32::min(bb_max.x, self.da_x_max).next_multiple_of(4),
            y: i32::min(bb_max.y, self.da_y_max),
        };

        let min = Point {
            x: i32::max(bb_min.x, self.da_x_min),
            y: i32::max(bb_min.y, self.da_y_min),
        };
        
        // This is to keep track of how many pixels gets drawn to calculate timing.
        let mut pixels_drawn: u64 = 0;
        
        #[derive(Default)]
        struct Delta {
            x: f32,
            y: f32,
        }

        // The barycentric coordinates delta along the x and y axes.
        let dx = [
            points[1].y - points[2].y,
            points[2].y - points[0].y,
            points[0].y - points[1].y,
        ];

        let dy = [
            points[2].x - points[1].x,
            points[0].x - points[2].x,
            points[1].x - points[0].x,
        ];
        
        let u_delta;
        let v_delta;
        
        if Tex::IS_TEXTURED {
            let delta: f32x4 = coords.iter()
                .zip(dx.iter())
                .zip(dy.iter())
                .fold(i32x4::default(), |delta, ((uv, dx), dy)| {
                    let uv = i32x4::from([
                        uv.u.into(),
                        uv.u.into(),
                        uv.v.into(),
                        uv.v.into()
                    ]);
                    delta + uv * i32x4::from([*dx, *dy, *dx, *dy])
                })
                .cast();

            let delta = delta / f32x4::from([area; 4]);
            
            u_delta = Delta {
                x: delta[0],
                y: delta[1],
            };

            v_delta = Delta {
                x: delta[2],
                y: delta[3],
            };
        } else {
            u_delta = Delta::default();  
            v_delta = Delta::default();  
        };
        
        let r_delta;
        let g_delta;
        let b_delta;
        
        if Shade::IS_SHADED {
            let delta: f32x8 = colors
                .iter()
                .zip(dx.iter())
                .zip(dy.iter())
                .fold(i32x8::default(), |delta, ((color, dx), dy)| {
                    let rgb = i32x8::from([
                        color.r.into(),
                        color.r.into(),
                        color.g.into(),
                        color.g.into(),
                        color.b.into(),
                        color.b.into(), 
                        0,
                        0
                    ]);
                    delta + rgb * i32x8::from([*dx, *dy, *dx, *dy, *dx, *dy, 0, 0])
                })
                .cast();

            let delta = delta / f32x8::from([area; 8]);

            r_delta = Delta {
                x: delta[0],
                y: delta[1],
            };

            g_delta = Delta {
                x: delta[2],
                y: delta[3],
            };

            b_delta = Delta {
                x: delta[4],
                y: delta[5],
            };
        } else {
            r_delta = Delta::default();
            g_delta = Delta::default();
            b_delta = Delta::default();
        };

        let attr_base;
        let fbias;
        
        if Tex::IS_TEXTURED || Shade::IS_SHADED {
            attr_base = [
                (points[1].x * points[2].y - points[2].x * points[1].y) as f32,
                (points[2].x * points[0].y - points[0].x * points[2].y) as f32,
                (points[0].x * points[1].y - points[1].x * points[0].y) as f32,
            ];
            fbias = [bias[0] as f32, bias[1] as f32, bias[2] as f32];
        } else {
            attr_base = [0.0; 3];
            fbias = [0.0; 3];
        }
        
        let (mut u_row, mut v_row) = if Tex::IS_TEXTURED {
            let u = attr_base[0] * coords[0].u as f32 - fbias[0]
                + attr_base[1] * coords[1].u as f32 - fbias[1]
                + attr_base[2] * coords[2].u as f32 - fbias[2];
            let v = attr_base[0] * coords[0].v as f32 - fbias[0]
                + attr_base[1] * coords[1].v as f32 - fbias[1]
                + attr_base[2] * coords[2].v as f32 - fbias[2];
            ((u / area) + 0.5, (v / area) + 0.5)
        } else {
            (0.0, 0.0)
        };

        let (mut r_row, mut g_row, mut b_row) = if Shade::IS_SHADED {
            let r = attr_base[0] * colors[0].r as f32 - fbias[0]
                + attr_base[1] * colors[1].r as f32 - fbias[1]
                + attr_base[2] * colors[2].r as f32 - fbias[2];
            let g = attr_base[0] * colors[0].g as f32 - fbias[0]
                + attr_base[1] * colors[1].g as f32 - fbias[1]
                + attr_base[2] * colors[2].g as f32 - fbias[2];
            let b = attr_base[0] * colors[0].b as f32 - fbias[0]
                + attr_base[1] * colors[1].b as f32 - fbias[1]
                + attr_base[2] * colors[2].b as f32 - fbias[2];
            ((r / area) + 0.5, (g / area) + 0.5, (b / area) + 0.5)
        } else {
            (0.0, 0.0, 0.0)
        };
        
        struct Edges {
            // The barycentric coordinates at the start of each line. They change every time the 
            // rasterizer goes down a line.
            y_bary: [i32x4; 3],
            // The current barycentric coordinates. They change for each column step.
            x_bary: [i32x4; 3],
            // The delta to the barycentric coordinates for each step along the x-axis.
            x_delta: [i32x4; 3],
            // The delta to the barycentric coordinates for each step along the y-axis.
            y_delta: [i32x4; 3],
        }
        
        impl Edges {
            fn new(v0: Point, v1: Point, v2: Point, origin: Point, bias: &[i32; 3]) -> Self {
                fn setup(v0: Point, v1: Point) -> [i32; 3] {
                    [v0.y - v1.y, v1.x - v0.x, v0.x * v1.y - v0.y * v1.x]
                }

                let s0 = setup(v1, v2);
                let s1 = setup(v2, v0);
                let s2 = setup(v0, v1);
                
                let x_delta = [
                    i32x4::from([s0[0] * 4; 4]),
                    i32x4::from([s1[0] * 4; 4]),
                    i32x4::from([s2[0] * 4; 4]),
                ];
                
                let y_delta = [
                    i32x4::from([s0[1]; 4]),
                    i32x4::from([s1[1]; 4]),
                    i32x4::from([s2[1]; 4]),
                ];
                
                let x = i32x4::from([origin.x; 4]) + i32x4::from([0, 1, 2, 3]);
                let y = i32x4::from([origin.y; 4]);
                
                let y_bary = [
                      i32x4::from([s0[0]; 4]) * x
                        + i32x4::from([s0[1]; 4]) * y
                        + i32x4::from([s0[2]; 4])
                        + i32x4::from([bias[0]; 4]),
                      i32x4::from([s1[0]; 4]) * x
                        + i32x4::from([s1[1]; 4]) * y
                        + i32x4::from([s1[2]; 4])
                        + i32x4::from([bias[1]; 4]),
                      i32x4::from([s2[0]; 4]) * x
                        + i32x4::from([s2[1]; 4]) * y
                        + i32x4::from([s2[2]; 4])
                        + i32x4::from([bias[2]; 4]),
                ];
                
                Self {
                    x_bary: y_bary.clone(),
                    y_bary,
                    x_delta,
                    y_delta,
                }
            }
            
            fn x_step(&mut self) {
                self.x_bary[0] += self.x_delta[0];
                self.x_bary[1] += self.x_delta[1];
                self.x_bary[2] += self.x_delta[2];
            }
            
            fn y_step(&mut self) {
                self.x_bary = self.y_bary.clone();
                self.y_bary[0] += self.y_delta[0];
                self.y_bary[1] += self.y_delta[1];
                self.y_bary[2] += self.y_delta[2];
            }
        }
        
        let mut edges = Edges::new(points[0], points[1], points[2], min, &bias);

        if Tex::IS_TEXTURED {
            u_row += u_delta.y * min.y as f32;
            u_row += u_delta.x * min.x as f32;
            v_row += v_delta.y * min.y as f32;
            v_row += v_delta.x * min.x as f32;
        }

        if Shade::IS_SHADED {
            r_row += r_delta.y * min.y as f32;
            r_row += r_delta.x * min.x as f32;
            g_row += g_delta.y * min.y as f32;
            g_row += g_delta.x * min.x as f32;
            b_row += b_delta.y * min.y as f32;
            b_row += b_delta.x * min.x as f32;
        }
        
        // Loop through all points in the bounding box, and draw the pixel if it's inside the
        // triangle.
        for y in min.y..=max.y {
            edges.y_step();
            
            let mut u = u_row;
            let mut v = v_row;
            
            let mut r = r_row;
            let mut g = g_row;
            let mut b = b_row;
            
            if Tex::IS_TEXTURED {
                u_row += u_delta.y;
                v_row += v_delta.y;
            }
            
            if Shade::IS_SHADED {
                r_row += r_delta.y;
                g_row += g_delta.y;
                b_row += b_delta.y;
            }

            for x in (min.x..=max.x).step_by(4) {
                // All three barycentric coordinates must all be positive for the point
                // to be inside the triangle. To check that we only have to check the
                // sign bit of all the weights.
                let mask = (edges.x_bary[0] | edges.x_bary[1] | edges.x_bary[2]).is_negative();

                edges.x_step();

                // If all the pixels are outside the triangle.
                if mask.all() {
                    if Tex::IS_TEXTURED {
                        u += u_delta.x * 4.0;
                        v += v_delta.x * 4.0;
                    }
            
                    if Shade::IS_SHADED {
                        r += r_delta.x * 4.0;
                        g += g_delta.x * 4.0;
                        b += b_delta.x * 4.0;
                    }

                    continue;
                }
                
                // Some of the pixels are in the triangle.
                for (i, ignore) in mask.to_array().iter().enumerate() {
                    if *ignore {
                        if Shade::IS_SHADED {
                            r += r_delta.x;
                            g += g_delta.x;
                            b += b_delta.x;
                        }
                        if Tex::IS_TEXTURED {
                            u += u_delta.x;
                            v += v_delta.x;
                        }

                        continue;
                    }
                    
                    let x = x + i as i32;
                    
                    let shade = if Shade::IS_SHADED {
                        let color = Color::from_rgb(r as u8, g as u8, b as u8);
                    
                        r += r_delta.x;
                        g += g_delta.x;
                        b += b_delta.x;

                        color
                    } else {
                        flat_shade
                    };

                    let (color, masked) = if Tex::IS_TEXTURED {
                        let uv = TexCoord {
                            u: u as u8,
                            v: v as u8,
                        };

                        u += u_delta.x;
                        v += v_delta.x;

                        // let texel = self.load_texel(uv, tex_param_cache);
                        let texel = self.load_texel(uv, tex_param_cache);

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
    pub fn draw_line<Shade, Trans>(
        &mut self,
        mut points: [Point; 2],
        colors: [Color; 2],
        flat_shade: Color
    ) -> u64
    where
        Shade: draw_mode::Shading,
        Trans: draw_mode::Transparency,
    {
        points[0] = self.clamp_to_da(points[0]);
        points[1] = self.clamp_to_da(points[1]);

        let dx = points[1].x - points[0].x;
        let dy = points[1].y - points[0].y;

        let abs_dx = dx.abs();
        let abs_dy = dy.abs();

        let longest = abs_dx.max(abs_dy) as u8;

        // Color delta values.
        // FIXME: Pretty sure this is wrong.
        let (dr, dg, db) = match Shade::IS_SHADED {
            false => (0, 0, 0),
            true => (
                colors[1].r.abs_diff(colors[0].r) / longest,
                colors[1].g.abs_diff(colors[0].g) / longest,
                colors[1].b.abs_diff(colors[0].b) / longest,
            ),
        };

        let Point { mut x, mut y } = points[0];
        let Color { mut r, mut g, mut b } = colors[0];

        // Lines are always dithered.
        self.draw_pixel::<Trans, draw_mode::UnTextured>(
            x, y, flat_shade.dither(x, y), false
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
                    false => flat_shade,
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
                    false => flat_shade,
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
    ) -> u64
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
        
        if let TexelDepth::B4 | TexelDepth::B8 = self.status.texture_depth() {
            self.clut_cache.maybe_fetch(clut, self.status.texture_depth(), &self.vram);
        }

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
                    let texel = self.load_texel(tc, tex_param_cache);

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

/// Cache for static info used to render each textured pixel.
#[derive(Clone, Copy)]
struct TexParamCache {
    /// !(tex_win_w * 8) | ((tex_win_x & tex_win_w) * 8).
    tex_win_u: u8,
    /// !(tex_win_h * 8) | ((tex_win_y & tex_win_h) * 8).
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
