use super::primitive::Color;

/// VRAM consists of 512 lines of 2048 bytes each, which equals 1 megabyte.
pub struct Vram {
    pub data: Box<[u8; Self::SIZE]>,
}

impl Vram {
    pub const SIZE: usize = 1024 * 1024;

    pub fn new() -> Self {
        Self {
            data: Box::new([0x0; Self::SIZE]),
        }
    }

    pub fn load_16(&self, x: i32, y: i32) -> u16 {
        let offset = offset_16(x, y);
        let (hi, lo) = (self.data[offset] as u16, self.data[offset + 1] as u16);
        (hi << 8) | lo
    }

    #[allow(dead_code)]
    pub fn load_24(&self, x: i32, y: i32) -> u32 {
        let offset = offset_24(x, y);
        let (hi, mid, lo) = (
            self.data[offset] as u32,
            self.data[offset + 1] as u32,
            self.data[offset + 2] as u32,
        );
        (hi << 16) | (mid << 8) | lo
    }

    pub fn store_16(&mut self, x: i32, y: i32, val: u16) {
        let offset = offset_16(x, y);
        self.data[offset] = val as u8;
        self.data[offset + 1] = (val >> 8) as u8;
    }
    
    pub fn raw_data(&self) -> &[u8; Self::SIZE] {
        &self.data
    }

    pub fn clear(&mut self) {
        self.data = Box::new([0x0; Self::SIZE]);
    }
    
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut img = Vec::with_capacity(1024 * 512 * 4);
        for y in 0..512 {
            for x in 0..1024 {
                let color = Color::from_u16(self.load_16(x, y));
                img.push(color.r << 3);
                img.push(color.g << 3);
                img.push(color.b << 3);
                img.push(255);
            }
        }
        img
    }
}

fn offset_16(x: i32, y: i32) -> usize {
    (x * 2 + y * 2048) as usize & (Vram::SIZE - 1)
}

fn offset_24(x: i32, y: i32) -> usize {
    (x * 3 + y * 2048) as usize & (Vram::SIZE - 1)
}
