use splst_util::Bit;
use super::{AddrUnit, BusMap};
use super::raw::RawMem;

pub struct Ram(RawMem<{Self::SIZE}>);

impl Ram {
    const SIZE: usize = 2 * 1024 * 1024;

    pub fn new() -> Self {
        Self(RawMem::new())
    }

    #[inline]
    pub fn load<T: AddrUnit>(&self, offset: u32) -> T {
        let offset = offset.bit_range(0, 20);
        self.0.load(offset)
    }

    #[inline]
    pub fn store<T: AddrUnit>(&mut self, offset: u32, val: T) {
        let offset = offset.bit_range(0, 20);
        self.0.store(offset, val)
    }
}

impl BusMap for Ram {
    const BUS_BEGIN: u32 = 0x0;
    const BUS_END: u32 = Self::SIZE as u32 - 1;
}
