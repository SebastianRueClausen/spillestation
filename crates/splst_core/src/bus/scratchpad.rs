use super::raw::RawMem;
use super::{AddrUnit, BusMap};

pub struct ScratchPad(RawMem<{Self::SIZE}>);

impl ScratchPad {
    const SIZE: usize = 1024;

    pub fn new() -> Self {
        Self(RawMem::new())
    }

    #[inline]
    pub fn load<T: AddrUnit>(&mut self, offset: u32) -> T {
        self.0.load(offset)
    }

    #[inline]
    pub fn store<T: AddrUnit>(&mut self, offset: u32, val: T) {
        self.0.store(offset, val)
    }
}

impl BusMap for ScratchPad {
    const BUS_BEGIN: u32 = 0x1f80_0000;
    const BUS_END: u32 = Self::BUS_BEGIN + 1024 - 1;
}
