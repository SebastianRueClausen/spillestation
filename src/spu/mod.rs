use crate::bus::BusMap;

pub struct Spu;

impl Spu {
    pub fn new() -> Self {
        Self
    }

    pub fn store(&mut self, _addr: u32, _val: u32) {
        
    }

    pub fn load(&mut self, _addr: u32) -> u32 {
        0x0
    }
}

impl BusMap for Spu {
    const BUS_BEGIN: u32 = 0x1f801c00;
    const BUS_END: u32 = Self::BUS_BEGIN + 640 - 1;
}
