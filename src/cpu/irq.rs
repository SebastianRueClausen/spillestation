
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Irq {
    VBlank = 0,
    Gpu = 1,
    CdRom = 2,
    Dma = 3,
    Tmr0 = 4,
    Tmr1 = 5,
    Tmr2 = 6,
    CtrlAndMemCard = 7,
    Sio = 8,
    Spu = 9,
}

pub struct IrqState {
    pub status: u32,
    pub mask: u32,
}

impl IrqState {
    pub fn new() -> Self {
        Self {
            status: 0,
            mask: 0,
        }
    }

    /// Trigger an interrupt.
    pub fn trigger(&mut self, irq: Irq) {
        self.status |= 1 << irq as u32;
    }

    pub fn active(&self) -> bool {
        self.status & self.mask != 0
    }

    pub fn store(&mut self, offset: u32, value: u32) {
        match offset {
            0 => self.status &= value,
            4 => self.mask = value,
            _ => unreachable!("Invalid store at offset {}", offset),
        }
    }

    pub fn load(&mut self, offset: u32) -> u32 {
        match offset {
            0 => self.status,
            4 => self.mask,
            _ => unreachable!("Invalid load at offset {}", offset),
        }
    }
}
