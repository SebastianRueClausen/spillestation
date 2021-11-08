
use super::primitive::Point;

const VRAM_SIZE: usize = 1024 * 1024;

/// VRAM consists of 512 lines of 2048 bytes each, which equals 1 megabyte.
pub struct Vram {
    data: Box<[u8; VRAM_SIZE]>,
}

impl Vram {
    pub fn new() -> Self {
        Self {
            data: Box::new([0x0; VRAM_SIZE]),
        }
    }

    pub fn load_16(&self, point: Point) -> u16 {
        let offset = offset(&point);
        let (hi, lo) = (self.data[offset] as u16, self.data[offset + 1] as u16); 
        (hi << 8) | lo
    }

    pub fn load_24(&self, point: Point) -> u32 {
        let offset = offset(&point);
        let (hi, mid, lo) = (
            self.data[offset + 0] as u32,
            self.data[offset + 1] as u32,
            self.data[offset + 2] as u32,
        );
        (hi << 16) | (mid << 8) | lo
    }

    pub fn store_16(&mut self, point: Point, value: u16) {
        let offset = offset(&point);
        println!("{:?}", offset);
        self.data[offset + 0] = (value >> 0) as u8;
        self.data[offset + 1] = (value >> 8) as u8;
    }
}

fn offset(point: &Point) -> usize {
    let x = point.x & (1024 - 1);
    let y = point.y & (512 - 1);
    (y * 1024 + x) as usize
}
