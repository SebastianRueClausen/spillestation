use super::primitive::Color;

/// VRAM consists of 512 lines of 2048 bytes each, which equals 1 megabyte.
#[derive(Clone, Copy)]
pub struct Vram {
    pub data: [u16; Vram::SIZE],
}

impl Vram {
    pub const SIZE: usize = 1024 * 512;

    pub fn new() -> Self {
        Self { data: [0x0; Vram::SIZE] }
    }
    
    pub fn load_16(&self, x: i32, y: i32) -> u16 {
        self.data[offset_16(x, y)]
    }

    pub fn store_16(&mut self, x: i32, y: i32, val: u16) {
        self.data[offset_16(x, y)] = val;
    }
    
    pub fn raw_data(&self) -> &[u16; 1024 * 512] {
        &self.data
    }

    pub fn clear(&mut self) {
        self.data = [0x0; Self::SIZE];
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
    (x + y * 1024) as usize & (Vram::SIZE - 1)
}
