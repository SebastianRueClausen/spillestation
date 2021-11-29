use super::Gpu;
use super::primitive::{
    Point,
    Color,
    Vertex,
};

impl Gpu {
    fn draw_pixel(&mut self, point: Point, color: Color) {
        self.vram.store_16(point, color.as_u16());
    }

    pub fn draw_triangle_scalar(&mut self, v1: &Vertex, v2: &Vertex, v3: &Vertex) {
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

    pub fn draw_triangle(&mut self, v1: &Vertex, v2: &Vertex, v3: &Vertex) {
        self.draw_triangle_scalar(v1, v2, v3);
    }

    pub fn draw_line(&mut self, _start: Point, _end: Point) {
    }
}
