use crate::util::bits::BitExtract;
use ultraviolet::vec::Vec3;

/// The PSX uses 2D coordinates for everything.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x, y,
        }
    }

    pub fn from_cmd(cmd: u32) -> Self {
        Self::new(cmd.extract_bits(0, 10) as i32, cmd.extract_bits(16, 26) as i32)
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
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct TexCoord {
    pub u: u8,
    pub v: u8,
}

impl TexCoord {
    fn new(u: u8, v: u8) -> Self {
        Self {
            u, v,
        }
    }
}

/// Depth of the color can be either 16 or 24 bits.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r, g, b,
        }
    }

    pub fn as_u16(self) -> u16 {
        let r = (self.r & 0xf8) as u16;
        let g = (self.g & 0xf8) as u16;
        let b = (self.b & 0xf8) as u16;
        (r >> 3) | (g << 2) | (b << 7)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Vertex {
    pub point: Point,
    pub color: Color,
    pub texcoord: TexCoord,
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            point: Point::new(0, 0),
            color: Color::from_rgb(0, 0, 0),
            texcoord: TexCoord::new(0, 0),
        }
    }
}

