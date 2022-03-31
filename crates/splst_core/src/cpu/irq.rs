use splst_util::Bit;
use crate::bus::BusMap;
use crate::schedule::{Event, Schedule};

use std::fmt;

/// The different kind of interrupts. The value is the nth bit that represents the interrupts in
/// the status and mask register.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Irq {
    /// Triggered every time the CPU enters Vblank.
    VBlank = 0,
    /// Rarely used. Can be requested via the GP0(0x1f) GPU command.
    Gpu = 1,
    /// Usually triggered by the CDROM controller when it's done executing a command.
    CdRom = 2,
    /// Can be triggered by the DMA when it's done executing a transfer.
    Dma = 3,
    /// Timer 0.
    Tmr0 = 4,
    /// Timer 1.
    Tmr1 = 5,
    /// Timer 2.
    Tmr2 = 6,
    /// Triggered when the IO Ports have recieved a byte.
    CtrlAndMemCard = 7,
    /// Serial port.
    Sio = 8,
    /// Sound processing unit.
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

/// Interrupt registers. There keep track of which 
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

    /// Trigger interrupt. This doesn't make the system do anything on it's own, since the CPU
    /// doesn't check for new active interrupts unless it's forced to. If the type of interrupt
    /// isn't masked, meaning it's not enabled, it will still be set as active in the
    /// 'status' register.
    pub fn trigger(&mut self, irq: Irq) {
        self.status |= 1 << irq as u32;
        if irq as u32 & self.mask != 0 {
            trace!("Triggered irq of type {}", irq);
        }
    }

    /// Check if there are any active interrupts. Even if this is true, the CPU may not acknowledge
    /// them, since interrupts can be disabled by the COP0.
    pub fn active(&self) -> bool {
        self.status & self.mask != 0
    }

    /// Check if a specific type of interrupt has been triggered. It doesn't account for if the
    /// interrupt is masked.
    pub fn is_triggered(&self, irq: Irq) -> bool {
        self.status.bit(irq as usize)
    }

    /// Check if a specific type of interrupt is masked, meaning if it's enabled.
    pub fn is_masked(&self, irq: Irq) -> bool {
        self.mask.bit(irq as usize)
    }

    /// Store to interrupt registers.
    ///
    /// Writing to the status register will bitwise and 'val' with the current value of the
    /// register. Writing to the mask register will set the register.
    pub fn store(&mut self, schedule: &mut Schedule, offset: u32, val: u32) {
        schedule.trigger(Event::IrqCheck);
        match offset {
            0 => self.status &= val,
            4 => self.mask = val,
            _ => unreachable!("Invalid store at offset {offset}"),
        }
    }

    /// Load from interrupt registers.
    pub fn load(&self, offset: u32) -> u32 {
        match offset {
            0 => self.status,
            4 => self.mask,
            _ => unreachable!("Invalid load at offset {offset}"),
        }
    }
}

impl BusMap for IrqState {
    const BUS_BEGIN: u32 = 0x1f801070;
    const BUS_END: u32 = Self::BUS_BEGIN + 8 - 1;
}
