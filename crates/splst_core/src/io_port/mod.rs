#![allow(dead_code)]

pub mod controller;
mod memcard;

use splst_util::{Bit, BitSet};
use crate::bus::BusMap;
use crate::schedule::{Schedule, Event};
use crate::cpu::Irq;
use crate::Cycle;

use memcard::MemCard;

use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;

pub use controller::{Button, ButtonState, Controllers};

/// Controller and Memory Card I/O ports.
pub struct IoPort {
    state: State,
    active_device: Option<Device>,

    ctrl: CtrlReg,
    stat: StatReg,
    mode: ModeReg,
    baud: u16,

    rx_fifo: Option<u8>,
    tx_fifo: Option<u8>,
    tx_val: u8,

    memcards: [Option<MemCard>; 2],
    pub(super) controllers: Rc<RefCell<Controllers>>,
}

impl IoPort {
    pub fn new(controllers: Rc<RefCell<Controllers>>) -> Self {
        Self {
            state: State::Idle,
            active_device: None,

            ctrl: CtrlReg(0),
            stat: StatReg(0),
            mode: ModeReg(0),
            baud: 0,

            rx_fifo: None,
            tx_fifo: None,
            tx_val: 0x0,

            memcards: [None, None],
            controllers,
        }
    }

    pub fn store(&mut self, schedule: &mut Schedule, addr: u32, val: u32) {
        trace!("IO Port store");

        match addr {
            0 => {
                if self.tx_fifo.is_some() {
                    warn!("Write to TX FIFO while full");
                }

                self.tx_fifo = Some(val as u8);

                if self.can_begin_transfer() && !self.is_transmitting() {
                    self.begin_transfer(schedule);
                }
            }
            8 => self.mode = ModeReg(val as u16),
            10 => {
                self.ctrl = CtrlReg(val as u16);

                if self.ctrl.reset() {
                    trace!("IoPort control reset");

                    if self.is_transmitting() {
                        self.end_transfer(schedule);
                    }

                    self.reset_device_states();

                    self.baud = 0;
                    self.mode = ModeReg(0);
                    self.ctrl = CtrlReg(0);

                    self.tx_fifo = None;
                    self.rx_fifo = None;
                }

                if self.ctrl.ack() {
                    self.stat.0 = self.stat.0.set_bit(0, false);
                }

                if !self.ctrl.select() {
                    self.reset_device_states();
                    self.active_device = None;
                }

                if !self.ctrl.select() || !self.ctrl.tx_enabled() {
                    if self.is_transmitting() {
                        self.end_transfer(schedule);
                    }
                } else {
                    if !self.is_transmitting() && self.can_begin_transfer() {
                        self.begin_transfer(schedule);
                    }
                }
            },
            14 => self.baud = val as u16,
            _ => todo!("{}", addr),
        }
    }

    pub fn load(&mut self, schedule: &mut Schedule, addr: u32) -> u32 {
        match addr {
            0 => {
                if self.is_transmitting() {
                    schedule.unschedule(Event::IoPortTransfer);
                    self.transfer(schedule);
                }

                let val: u32 = self.rx_fifo
                    .take()
                    .unwrap_or(0xff)
                    .into();

                val | (val << 8) | (val << 16) | (val << 24)
            }
            4 => {
                self.stat.0 = self.stat.0
                    .set_bit(0, self.tx_fifo.is_none())
                    .set_bit(1, self.rx_fifo.is_none())
                    .set_bit(2, {
                        self.tx_fifo.is_none() && self.state != State::InTrans
                    });

                let val: u32 = self.stat.0.into();

                // Set acknownledge input false.
                self.stat.0 = self.stat.0.set_bit(4, false);

                val
            }
            10 => self.ctrl.0.into(),
            14 => self.baud.into(),
            _ => todo!("IoPort load at offset {addr}"),
        }
    }

    fn can_begin_transfer(&self) -> bool {
        self.tx_fifo.is_some()
            && self.ctrl.select()
            && self.ctrl.tx_enabled()
    }
    
    fn is_transmitting(&self) -> bool {
        self.state != State::Idle
    }

    fn reset_device_states(&mut self) {
        self.controllers
            .borrow_mut()
            .iter_mut()
            .for_each(|c| c.reset());
    }

    fn begin_transfer(&mut self, schedule: &mut Schedule) {
        // Set rx_enabled.
        self.ctrl.0.set_bit(2, true);

        self.tx_val = self.tx_fifo
            .take()
            .expect("TX FIFO should be full");

        self.state = State::InTrans;

        schedule.schedule_in(self.baud as Cycle * 8, Event::IoPortTransfer);
    }

    fn end_transfer(&mut self, schedule: &mut Schedule) {
        self.state = State::Idle;

        // Could maybe ignore it and just to nothing if the state is idle, but it may be possible
        // to change the state in the mean time, which would mess up timings.
        schedule.unschedule(Event::IoPortTransfer);
    }

    pub fn transfer(&mut self, schedule: &mut Schedule) {
        match self.state {
            State::InTrans => self.make_transfer(schedule),
            State::WaitForAck | State::Idle => {
                // Set acknowledge input flag.
                self.stat.0 = self.stat.0.set_bit(7, true);

                if self.ctrl.ack_irq_enabled() {
                    // Set interrupt flag.
                    self.stat.0 = self.stat.0.set_bit(9, true);
                    schedule.schedule_now(Event::IrqTrigger(Irq::CtrlAndMemCard));
                }

                self.end_transfer(schedule);

                if self.can_begin_transfer() {
                    self.begin_transfer(schedule);
                }
            }
        }
    }

    fn make_transfer(&mut self, schedule: &mut Schedule) {
        let index = self.ctrl.io_slot() as usize;

        let ctrl = &mut self.controllers.borrow_mut()[self.ctrl.io_slot()];
        let memcard = &mut self.memcards[index];

        // Set rx_enabled.
        self.ctrl.0.set_bit(2, true);

        let (val, ack) = match self.active_device {
            None => {
                match (ctrl, memcard) {
                    (controller::Port::Unconnected, None) => (0xff, false),
                    (controller::Port::Digital(ctrl), _) => {
                        let (val, ack) = ctrl.transfer(self.tx_val);
                        if ack {
                            self.active_device = Some(Device::Controller);
                        }
                        (val, ack)
                    }
                    (controller::Port::Unconnected, Some(memcard)) => {
                        let (val, ack) = memcard.transfer(self.tx_val);
                        if ack {
                            self.active_device = Some(Device::MemCard);
                        }
                        (val, ack)
                    }
                }
            }
            Some(Device::Controller) => match ctrl {
                controller::Port::Digital(ctrl) => ctrl.transfer(self.tx_val),
                controller::Port::Unconnected => (0xff, false),
            }
            Some(Device::MemCard) => match memcard {
                Some(memcard) => memcard.transfer(self.tx_val),
                None => (0xff, false),
            }
        };

        self.rx_fifo = Some(val);

        match ack {
            false => {
                self.state = State::Idle;
                self.active_device = None;

                schedule.unschedule(Event::IoPortTransfer);
            }
            true => {
                let cycles = match self.active_device {
                    Some(Device::MemCard) => 170,
                    _ => 450,
                };

                schedule.schedule_in(cycles, Event::IoPortTransfer);
                self.state = State::WaitForAck
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum IoSlot {
    Slot1,
    Slot2,
}

impl fmt::Display for IoSlot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "slot {}", *self as usize + 1)
    }
}

impl Default for IoSlot {
    fn default() -> Self {
        IoSlot::Slot1
    }
}

#[derive(Clone, Copy)]
struct StatReg(u32);

impl StatReg {
    fn tx_ready(self) -> bool {
        self.0.bit(0)
    }

    fn rx_fifo_empty(self) -> bool {
        self.0.bit(1)
    }

    fn tx_done(self) -> bool {
        self.0.bit(2)
    }

    fn ack_input_lvl(self) -> bool {
        self.0.bit(7)
    }

    fn irq(self) -> bool {
        self.0.bit(9)
    }

    fn baud_tmr(self) -> u32 {
        self.0.bit_range(11, 31)
    }
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

    fn ack(self) -> bool {
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

    fn io_slot(self) -> IoSlot {
        match self.0.bit(13) {
            false => IoSlot::Slot1,
            true => IoSlot::Slot2,
        }
    }
}

#[derive(Clone, Copy)]
struct ModeReg(u16);

impl ModeReg {
    fn reload_factor(self) -> u32 {
        match self.0.bit_range(0, 1) {
            0 | 1 => 1,
            2 => 16,
            3 => 64,
            _ => unreachable!(),
        }
    }

    fn char_len(self) -> u32 {
        5 + self.0.bit_range(2, 3) as u32
    }

    fn parity_enabled(self) -> bool {
        self.0.bit(4)
    }

    fn parity_odd(self) -> bool {
        self.0.bit(5)
    }
}


#[derive(PartialEq, Eq)]
enum State {
    Idle,
    InTrans,
    WaitForAck,
}

#[derive(PartialEq, Eq)]
enum Device {
    Controller,
    MemCard,
}

impl BusMap for IoPort {
    const BUS_BEGIN: u32 = 0x1f801040;
    const BUS_END: u32 = Self::BUS_BEGIN + 32 - 1;
}
