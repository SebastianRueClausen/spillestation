use std::fmt;
use crate::bus::BusMap;

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

impl fmt::Display for Irq {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            Irq::VBlank => "Vblank",
            Irq::Gpu => "GPU",
            Irq::CdRom => "CDROM",
            Irq::Dma => "DMA",
            Irq::Tmr0 => "TMR0",
            Irq::Tmr1 => "TMR1",
            Irq::Tmr2 => "TMR2",
            Irq::CtrlAndMemCard => "controller and memory card",
            Irq::Sio => "SIO",
            Irq::Spu => "Spu",
        })
    }
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

    pub fn is_triggered(&self, irq: Irq) -> bool {
        self.status & (1 << irq as u32) == 1
    }

    pub fn is_masked(&self, irq: Irq) -> bool {
        self.mask & (1 << irq as u32) == 1
    }

    pub fn store(&mut self, offset: u32, val: u32) {
        match offset {
            0 => self.status &= val,
            4 => self.mask = val,
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

impl BusMap for IrqState {
    const BUS_BEGIN: u32 = 0x1f801070;
    const BUS_END: u32 = Self::BUS_BEGIN + 8 - 1;
}
