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

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Texel(u16);

impl Texel {
    pub fn new(value: u16) -> Self {
        Self(value)
    }

    pub fn as_color(self) -> Color {
        Color::from_u16(self.0)
    }

    pub fn is_transparent(self) -> bool {
        (self.0 as u32).extract_bit(15) == 1
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

    pub fn shade_blend(self, other: Self) -> Self {
        let r = (self.r as u16) * (other.r as u16);
        let g = (self.g as u16) * (other.g as u16);
        let b = (self.b as u16) * (other.b as u16);
        Self {
            r: (r / 0x80).min(0xff) as u8,
            g: (g / 0x80).min(0xff) as u8,
            b: (b / 0x80).min(0xff) as u8,
        }
    }

    pub fn avg_blend(self, other: Self) -> Self {
        Self {
            r: (self.r / 2).saturating_add(other.r / 2),
            g: (self.g / 2).saturating_add(other.g / 2),
            b: (self.b / 2).saturating_add(other.b / 2),
        }
    }

    pub fn add_blend(self, other: Self) -> Self {
        Self {
            r: other.r.saturating_add(self.r),
            g: other.g.saturating_add(self.g),
            b: other.b.saturating_add(self.b),
        }
    }

    pub fn sub_blend(self, other: Self) -> Self {
        Self {
            r: other.r.saturating_sub(self.r),
            g: other.g.saturating_sub(self.g),
            b: other.b.saturating_sub(self.b),
        }
    }

    pub fn add_div_blend(self, other: Self) -> Self {
        Self {
            r: (other.r as i32 + ((self.r / 4) as i32)).clamp(0, 255) as u8,
            g: (other.g as i32 + ((self.g / 4) as i32)).clamp(0, 255) as u8,
            b: (other.b as i32 + ((self.b / 4) as i32)).clamp(0, 255) as u8,
        }
    }
}

pub struct TextureParams {
    pub clut_x: i32,
    pub clut_y: i32,
    pub texture_x: i32,
    pub texture_y: i32,
    pub texture_colors: i32,
}

impl Default for TextureParams {
    fn default() -> Self {
        Self {
            clut_x: 0,
            clut_y: 0,
            texture_x: 0,
            texture_y: 0,
            texture_colors: 0,
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

