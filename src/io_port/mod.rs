#![allow(dead_code)]

mod dualshock;

use crate::util::Bit;
use crate::bus::BusMap;
use crate::cpu::{Irq, IrqState};

#[derive(Clone, Copy)]
enum SlotNum {
    Joy1,
    Joy2,
}

#[derive(Clone, Copy)]
struct CtrlReg(u16);

impl CtrlReg {
    fn tx_enabled(self) -> bool {
        self.0.bit(0)
    }

    fn select(self) -> bool {
        self.0.bit(1)
    }

    fn rx_enabled(self) -> bool {
        self.0.bit(2)
    }

    fn acknowledge(self) -> bool {
        self.0.bit(4)
    }

    fn reset(self) -> bool {
        self.0.bit(6)
    }

    /// RX Interrupt mode. This tells when it should IRQ, in relation to how many bytes the RX FIFO
    /// contains. It's either 1, 2, 4 or 8.
    fn rx_irq_mode(self) -> u32 {
        match self.0.bit_range(8, 9) {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => unreachable!(),
        }
    }

    fn tx_irq_enabled(self) -> bool {
        self.0.bit(10)
    }

    fn rx_irq_enabled(self) -> bool {
        self.0.bit(11)
    }

    fn ack_irq_enabled(self) -> bool {
        self.0.bit(12)
    }

    fn desired_slot_num(self) -> SlotNum {
        match self.0.bit(13) {
            false => SlotNum::Joy1,
            true => SlotNum::Joy2,
        }
    }
}

/// Controller and memory card I/O ports.
pub struct IoPort {
    mode: u8,
    baud: u16,
    response: u16,
    is_idle: bool,
    tx_pending: Option<u8>,
    ctrl: CtrlReg,
}

impl IoPort {
    pub fn new() -> Self {
        Self {
            mode: 0,
            baud: 0,
            response: 0xff,
            is_idle: true,
            tx_pending: None,
            ctrl: CtrlReg(0),
        }
    }

    pub fn store(&mut self, irq: &mut IrqState, addr: u32, val: u32) {
        trace!("IO Port store");
        match addr {
            0 => todo!(),
            8 => self.mode = val as u8,
            10 => {
                self.ctrl = CtrlReg(val as u16);
                if self.ctrl.reset() {
                    trace!("IoPort control reset");
                    self.baud = 0;
                    self.mode = 0;
                    self.ctrl = CtrlReg(0);
                }
                if !self.ctrl.acknowledge() && self.ctrl.ack_irq_enabled() {
                   irq.trigger(Irq::CtrlAndMemCard); 
                }
            },
            14 => self.baud = val as u16,
            _ => todo!("{}", addr),
        }
    }

    pub fn load(&mut self, addr: u32) -> u32 {
        match addr {
            0 => self.response as u32,
            _ => todo!("IoPort load at offset {}", addr),
        }
    }

    fn _maybe_transfer_byte(&mut self) {
        let Some(_val) = self.tx_pending.take() else {
            return;
        };
        if !self.ctrl.tx_enabled() || !self.is_idle {
            return;
        }
        if !self.ctrl.select() {
            warn!("Select off for IO Port");
        }
        let _tx_start = self.baud - 40;
        let _tx_amount = (self.baud - 11) * 11;
         
    }
}

impl BusMap for IoPort {
    const BUS_BEGIN: u32 = 0x1f801040;
    const BUS_END: u32 = Self::BUS_BEGIN + 32 - 1;
}
