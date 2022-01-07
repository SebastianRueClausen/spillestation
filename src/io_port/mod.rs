#![allow(dead_code)]

mod dualshock;

use crate::util::BitExtract;
use crate::bus::BusMap;

#[derive(Clone, Copy)]
enum SlotNum {
    Joy1,
    Joy2,
}

#[derive(Clone, Copy)]
struct CtrlReg(u16);

impl CtrlReg {
    fn new() -> Self {
        Self(0)
    }

    fn tx_enabled(self) -> bool {
        self.0.extract_bit(0) == 1
    }

    fn rx_enabled(self) -> bool {
        self.0.extract_bit(2) == 1
    }

    fn acknowledge(self) -> bool {
        self.0.extract_bit(4) == 1
    }

    fn reset(self) -> bool {
        self.0.extract_bit(6) == 1
    }

    /// RX Interrupt mode. This tells when it should IRQ, in relation to how many bytes the RX FIFO
    /// contains. It's either 1, 2, 4 or 8.
    fn rx_irq_mode(self) -> u32 {
        match self.0.extract_bits(8, 9) {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => unreachable!(),
        }
    }

    fn tx_irq_enabled(self) -> bool {
        self.0.extract_bit(10) == 1
    }

    fn rx_irq_enabled(self) -> bool {
        self.0.extract_bit(11) == 1
    }

    fn ack_irq_enabled(self) -> bool {
        self.0.extract_bit(12) == 1
    }

    fn desired_slot_num(self) -> SlotNum {
        match self.0.extract_bit(13) {
            0 => SlotNum::Joy1,
            1 => SlotNum::Joy2,
            _ => unreachable!(),
        }
    }
}

/// Controller and memory card I/O ports.
pub struct IoPort {
    mode: u8,
    /// Baudrate reload value.
    baud: u16,
    ctrl: CtrlReg,
}

impl IoPort {
    pub fn new() -> Self {
        Self {
            mode: 0,
            baud: 0,
            ctrl: CtrlReg::new(),
        }
    }

    pub fn store(&mut self, addr: u32, val: u32) {
        match addr {
            0 => todo!(),
            8 => self.mode = val as u8,
            10 => {
                self.ctrl = CtrlReg(val as u16);
            },
            14 => self.baud = val as u16,
            _ => todo!("{}", addr),
        }
    }

    pub fn load(&mut self, _addr: u32) -> u32 {
        todo!();
    }
}

impl BusMap for IoPort {
    const BUS_BEGIN: u32 = 0x1f801040;
    const BUS_END: u32 = Self::BUS_BEGIN + 32 - 1;
}
