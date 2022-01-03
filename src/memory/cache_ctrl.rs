use super::BusMap;
use crate::util::bits::BitExtract;

pub struct CacheCtrl {
    reg: u32,
}

impl CacheCtrl {
    pub fn new() -> Self {
        Self { reg: 0 }
    }

    pub fn store(&mut self, val: u32) {
        self.reg = val 
    }

    pub fn load(&self) -> u32 {
        self.reg
    }

    pub fn icache_enabled(&self) -> bool {
        self.reg.extract_bit(11) == 1
    }
}

impl BusMap for CacheCtrl {
    const BUS_BEGIN: u32 = 0xfffe0130;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}
