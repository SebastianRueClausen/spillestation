use super::raw::RawMem;
use super::{MemUnit, BusMap};

pub struct ScratchPad(RawMem<{Self::SIZE}>);

impl ScratchPad {
    const SIZE: usize = 1024;

    pub fn new() -> Self {
        Self(RawMem::new())
    }

    #[inline]
    pub fn load<T: MemUnit>(&mut self, offset: u32) -> u32 {
        self.0.load::<T>(offset)
    }

    #[inline]
    pub fn store<T: MemUnit>(&mut self, offset: u32, val: u32) {
        self.0.store::<T>(offset, val)
    }
}

impl BusMap for ScratchPad {
    const BUS_BEGIN: u32 = 0x1f80_0000;
    const BUS_END: u32 = Self::BUS_BEGIN + 1024 - 1;
}
