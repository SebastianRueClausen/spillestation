use super::{AddrUnit, BusMap};
use crate::util::BitExtract;

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

    pub fn load<T: AddrUnit>(&mut self, offset: u32) -> u32 {
        // Make sure RAM is mirrorred four time.
        let offset = offset.extract_bits(0, 20) as usize;
        (0..T::WIDTH).fold(0, |value, byte| {
            value | (self.data[offset + byte] as u32) << (8 * byte)
        })
    }

    pub fn store<T: AddrUnit>(&mut self, offset: u32, value: u32) {
        let offset = offset.extract_bits(0, 20) as usize;
        for i in 0..T::WIDTH {
            self.data[offset + i] = (value >> (8 * i)) as u8;
        }
    }
}

impl BusMap for Ram {
    const BUS_BEGIN: u32 = 0x0;
    const BUS_END: u32 = RAM_SIZE as u32 - 1;

}