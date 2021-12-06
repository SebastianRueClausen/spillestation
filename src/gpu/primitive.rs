use crate::util::bits::BitExtract;

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

    pub fn from_u32(value: u32) -> Self {
        Self {
            x: value.extract_bits(0, 10) as i32,
            y: value.extract_bits(16, 26) as i32,
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

    pub fn from_u16(value: u16) -> Self {
        Self {
            r: ((value << 3) & 0xf8) as u8,
            g: ((value >> 2) & 0xf8) as u8,
            b: ((value >> 7) & 0xf8) as u8,
        }
    }

    pub fn from_u32(value: u32) -> Self {
        Self {
            r: value.extract_bits(0, 7) as u8,
            g: value.extract_bits(8, 15) as u8,
            b: value.extract_bits(16, 23) as u8,
        }
    }

    pub fn as_u16(self) -> u16 {
        let r = (self.r & 0xf8) as u16;
        let g = (self.g & 0xf8) as u16;
        let b = (self.b & 0xf8) as u16;
        (r >> 3) | (g << 2) | (b << 7)
    }
}

pub struct TextureParams {
    pub palette_page_x: i32,
    pub palette_page_y: i32,
    pub texture_page_x: i32,
    pub texture_page_y: i32,
    pub texture_page_colors: i32,
}

impl Default for TextureParams {
    fn default() -> Self {
        Self {
            palette_page_x: 0,
            palette_page_y: 0,
            texture_page_x: 0,
            texture_page_y: 0,
            texture_page_colors: 0,
        }
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
            color: Color::from_rgb(255, 0, 0),
            texcoord: TexCoord::new(0, 0),
        }
    }
}

