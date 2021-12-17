pub const VRAM_SIZE: usize = 1024 * 1024;

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

    pub fn store_16(&mut self, x: i32, y: i32, value: u16) {
        let offset = offset_16(x, y);
        self.data[offset] = value as u8;
        self.data[offset + 1] = (value >> 8) as u8;
    }

    pub fn raw_data(&self) -> &[u8] {
        &self.data[..]
    }
}

fn offset_16(x: i32, y: i32) -> usize {
    (x * 2 + y * 2048) as usize & (VRAM_SIZE - 1)
}

fn offset_24(x: i32, y: i32) -> usize {
    (x * 3 + y * 2048) as usize & (VRAM_SIZE - 1)
}
