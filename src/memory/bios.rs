use super::AddrUnit;

/// Bios always takes up 512 kilobytes
pub const BIOS_SIZE: usize = 1024 * 512;

pub struct Bios {
    data: Box<[u8; BIOS_SIZE]>,
}

impl Bios {
    pub fn new(bytes: &[u8; BIOS_SIZE]) -> Self {
        Self {
            data: Box::new(*bytes)
        }
    }

    /// Load a memory from bios.
    pub fn load<T: AddrUnit>(&self, offset: u32) -> u32 {
        let mut value: u32 = 0;
        for i in 0..T::width() {
            value |= (self.data[offset as usize + i] as u32) << (8 * i);      
        }
        value 
    }
}
