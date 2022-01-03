use super::BusMap;

pub struct RamSize {
    reg: u32, 
}

impl RamSize {
    pub fn new() -> Self {
        Self { reg: 0 }
    }

    pub fn store(&mut self, val: u32) {
        self.reg = val;
    }

    pub fn load(&mut self) -> u32 {
        self.reg
    }
}

impl BusMap for RamSize {
    const BUS_BEGIN: u32 = 0x1f801060;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}


