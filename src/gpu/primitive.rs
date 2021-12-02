use ultraviolet::int::IVec2;
use crate::util::bits::BitExtract;

pub trait FromCmd {
    fn from_cmd(cmd: u32) -> Self;
}

impl FromCmd for IVec2 {
    fn from_cmd(cmd: u32) -> Self {
        Self::new(cmd.extract_bits(0, 10) as i32, cmd.extract_bits(16, 26) as i32)
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

impl FromCmd for Color {
    fn from_cmd(cmd: u32) -> Self {
        Self {
            r: (cmd & 0xff) as u8,
            g: ((cmd >> 8) & 0xff) as u8,
            b: ((cmd >> 16) & 0xff) as u8,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Vertex {
    pub point: IVec2,
    pub color: Color,
    pub texcoord: TexCoord,
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            point: IVec2::new(0, 0),
            color: Color::from_rgb(255, 0, 0),
            texcoord: TexCoord::new(0, 0),
        }
    }
}

