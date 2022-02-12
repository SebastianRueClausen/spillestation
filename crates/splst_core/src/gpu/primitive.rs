use splst_util::Bit;

/// A point on the screen or in VRAM.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn from_cmd(val: u32) -> Self {
        fn sign_extend(val: u32) -> i32 {
            ((val << 21) as i32) >> 21
        }

        Self {
            x: sign_extend(val), 
            y: sign_extend(val >> 16),
        }
    }

    pub fn with_offset(self, x: i32, y: i32) -> Self {
        Self::new(self.x + x, self.y + y)
    }
}

/// Texture coordinate.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct TexCoord {
    pub u: u8,
    pub v: u8,
}

impl TexCoord {
    fn new(u: u8, v: u8) -> Self {
        Self { u, v }
    }
}

/// Texture color.
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
        self.0.bit(15)
    }

    pub fn is_invisible(self) -> bool {
        self.0 == 0
    }
}

const DITHER_LUT: [[i32; 4]; 4] = [
    [-4, 0, -3, 1],
    [2, -2, 3, -1],
    [-3, 1, -4, 0],
    [3, -1, 2, -2]
];

/// Depth of the color can be either 16 or 24 bits.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn from_u16(val: u16) -> Self {
        Self {
            r: (val.bit_range(0, 4) << 3) as u8,
            g: (val.bit_range(5, 9) << 3) as u8,
            b: (val.bit_range(10, 14) << 3) as u8,
        }
    }

    pub fn from_cmd(cmd: u32) -> Self {
        Self {
            r: cmd.bit_range(0, 7) as u8,
            g: cmd.bit_range(8, 15) as u8,
            b: cmd.bit_range(16, 23) as u8,
        }
    }

    pub fn as_u16(self) -> u16 {
        let r = (self.r & 0xf8) as u16;
        let g = (self.g & 0xf8) as u16;
        let b = (self.b & 0xf8) as u16;
        (r >> 3) | (g << 2) | (b << 7)
    }

    /// The shading used when blending with the shading. Basically multiplying the two colors
    /// together and dividing by 128.
    pub fn shade_blend(self, other: Self) -> Self {
        let r = (self.r as u16) * (other.r as u16);
        let g = (self.g as u16) * (other.g as u16);
        let b = (self.b as u16) * (other.b as u16);
        Self {
            r: (r / 128).min(0xff) as u8,
            g: (g / 128).min(0xff) as u8,
            b: (b / 128).min(0xff) as u8,
        }
    }

    /// Average blending. Finds the average between the two colors.
    pub fn avg_blend(self, other: Self) -> Self {
        Self {
            r: (self.r / 2).saturating_add(other.r / 2),
            g: (self.g / 2).saturating_add(other.g / 2),
            b: (self.b / 2).saturating_add(other.b / 2),
        }
    }

    /// Add blending. Adds the colors together.
    pub fn add_blend(self, other: Self) -> Self {
        Self {
            r: other.r.saturating_add(self.r),
            g: other.g.saturating_add(self.g),
            b: other.b.saturating_add(self.b),
        }
    }

    /// Subtract blending. Subtracts the other color from self.
    pub fn sub_blend(self, other: Self) -> Self {
        Self {
            r: other.r.saturating_sub(self.r),
            g: other.g.saturating_sub(self.g),
            b: other.b.saturating_sub(self.b),
        }
    }

    /// Add and divide by 4 blending. Divide self by 4 and add with other.
    pub fn add_div_blend(self, other: Self) -> Self {
        Self {
            r: (other.r as i32 + ((self.r / 4) as i32)).clamp(0, 255) as u8,
            g: (other.g as i32 + ((self.g / 4) as i32)).clamp(0, 255) as u8,
            b: (other.b as i32 + ((self.b / 4) as i32)).clamp(0, 255) as u8,
        }
    }

    pub fn dither(self, p: Point) -> Self {
        let (x, y) = (p.x.bit_range(0, 1), p.y.bit_range(0, 1));
        let d = DITHER_LUT[y as usize][x as usize];
        Self {
            r: ((self.r as i32) + d).clamp(0, 255) as u8,
            g: ((self.g as i32) + d).clamp(0, 255) as u8,
            b: ((self.b as i32) + d).clamp(0, 255) as u8,
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
