use super::AddrUnit;

pub struct RawMem<const SIZE: usize> {
    data: Box<[u8; SIZE]>,
}

impl<const SIZE: usize> RawMem<SIZE> {
    pub fn new() -> Self {
        Self {
            data: Box::new([0xff; SIZE]),
        }
    }

    #[inline]
    pub fn load<T: AddrUnit>(&mut self, offset: u32) -> u32 {
        let offset = offset as usize;
        (0..T::WIDTH).fold(0, |value, byte| {
            value | (self.data[offset + byte] as u32) << (8 * byte)
        })
    }

    #[inline]
    pub fn store<T: AddrUnit>(&mut self, offset: u32, val: u32) {
        let offset = offset as usize;
        for byte in 0..T::WIDTH {
            self.data[offset + byte] = (val >> (8 * byte)) as u8;
        }
    }
}
