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

    pub fn draw_triangle(&mut self, v1: &Vertex, v2: &Vertex, v3: &Vertex) {
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
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                let p = Point {
                    x, y,
                };
                let (x, y, z) = Point::barycentric(&points, &p);
                if x < 0.0 || y < 0.0 || z < 0.0 {
                    continue;
                }
                // TODO: Color lerp.
                // TODO: Texture lerp.
                self.draw_pixel(p, Color::from_rgb(255, 255, 255));
            }
        }
    }
}