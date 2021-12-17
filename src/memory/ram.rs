use super::AddrUnit;
use crate::util::bits::BitExtract;

/// 2 Megabytes.
const RAM_SIZE: usize = 2 * 1024 * 1024;

pub struct Ram {
    data: Box<[u8; RAM_SIZE]>,
}

impl Ram {
    pub fn new() -> Self {
        Self {
            data: Box::new([0x44; RAM_SIZE]),
        }
    }

    pub fn load<T: AddrUnit>(&self, offset: u32) -> u32 {
        // Make sure RAM is mirrorred four time.
        let offset = offset.extract_bits(0, 20) as usize;
        let mut value: u32 = 0;
        for i in 0..T::width() {
            value |= (self.data[offset + i] as u32) << (8 * i);
        }
        value
    }

    pub fn store<T: AddrUnit>(&mut self, offset: u32, value: u32) {
        let offset = offset.extract_bits(0, 20) as usize;
        for i in 0..T::width() {
            self.data[offset + i] = (value >> (8 * i)) as u8;
        }
    }
}
