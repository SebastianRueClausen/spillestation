#![allow(dead_code)]

pub mod memcard;
pub mod pad;

use crate::bus::BusMap;
use crate::bus::{self, AddrUnit};
use crate::cpu::Irq;
use crate::schedule::{Event, EventId, Schedule};
use crate::SysTime;
use crate::{dump, dump::Dumper};
use splst_util::{Bit, BitSet};

use memcard::MemCards;
use pad::GamePads;

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// Gamepads and Memory Card I/O ports.
pub struct IoPort {
    state: State,

    /// The device currently active. May be either a controller or memory card.
    active_device: Option<Device>,

    /// Control register.
    control: ControlReg,

    /// Status register.
    status: StatusReg,

    /// Mode register.
    mode: ModeReg,

    /// # Baudrate
    ///
    /// Baudrate determines the amount of transfers of data between the I/O port and the connected
    /// device per second. The transfer size is usually 8 bits, although it can also be either 5, 6
    /// or 7, which is determined by the mode register.
    ///
    /// `baud` is the reload value for the baud timer stored in bit 11..31 of 'stat'. The baud timer
    /// is always running and decreases at ~33MHz (CPU clock rate) and it ellapses twice for each
    /// bit.
    ///
    /// The timer reloads when writing to the `baud` register or when the timer reaches zero. The timer
    /// is set to `baud` multiplied by the baudrate factor, mode register bits 0..1 (almost always 1),
    /// and then divided by 2.
    baud: u16,

    /// Recieve buffer. This is the data that the I/O port recieves from the peripherals. It's
    /// technically a FIFO queue of bits, but we only emulate sending a whole byte at once. It's
    /// cleared whenever the register is read.
    rx_fifo: Option<u8>,

    /// Transmit buffer. This is the data that get's send to the peripherals. The buffer is filled
    /// by writing to the register and cleared when send to the either a controller or memory card.
    tx_fifo: Option<u8>,

    /// Intermidate buffer to store the content of `tx_fifo` starting a transfer and actually
    /// sending the data to the devices.
    tx_val: u8,

    memcards: Rc<RefCell<MemCards>>,
    pads: Rc<RefCell<GamePads>>,
}

impl IoPort {
    pub(crate) fn new(pads: Rc<RefCell<GamePads>>, memcards: Rc<RefCell<MemCards>>) -> Self {
        Self {
            state: State::Idle,
            active_device: None,

            control: ControlReg(0),
            status: StatusReg(0),
            mode: ModeReg(0),
            baud: 0,

            rx_fifo: None,
            tx_fifo: None,
            tx_val: 0x0,

            memcards,
            pads,
        }
    }

    pub(crate) fn store<T: AddrUnit>(&mut self, schedule: &mut Schedule, addr: u32, val: T) {
        match addr {
            0 => {
                if self.tx_fifo.is_some() {
                    warn!("write to TX FIFO while full");
                }

                self.tx_fifo = Some(val.as_u8());

                if self.can_begin_transfer() && !self.state.is_transmitting() {
                    self.begin_transfer(schedule);
                }
            }
            8 => self.mode = ModeReg(val.as_u16()),
            10 => {
                self.control = ControlReg(val.as_u16());

                if self.control.reset() {
                    trace!("io port control reset");

                    self.stop_transfer(schedule);
                    self.reset_device_states();
                    self.active_device = None;

                    self.status = StatusReg(0);
                    self.mode = ModeReg(0);
                    self.control = ControlReg(0);

                    self.tx_fifo = None;
                    self.rx_fifo = None;
                }

                if self.control.ack() {
                    self.status.0 = self.status.0.set_bit(0, false);
                }

                if !self.control.select() {
                    self.reset_device_states();
                    self.active_device = None;
                }

                if !self.control.select() || !self.control.tx_enabled() {
                    self.stop_transfer(schedule);
                } else {
                    if !self.state.is_transmitting() && self.can_begin_transfer() {
                        self.begin_transfer(schedule);
                    }
                }
            }
            14 => self.baud = val.as_u16(),
            _ => todo!("I/O port store to {addr}"),
        }
    }

    pub(crate) fn load<T: AddrUnit>(&mut self, schedule: &mut Schedule, addr: u32) -> T {
        let val: u32 = match addr {
            0 => {
                self.do_transfer_early(schedule);

                let val: u32 = self.rx_fifo.take().unwrap_or(0xff).into();

                val | (val << 8) | (val << 16) | (val << 24)
            }
            4 => {
                self.do_transfer_early(schedule);

                let val: u32 = self.status_reg().0.into();

                // TODO: Emulate BAUD timer.

                // Set acknownledge input false.
                self.status.0 = self.status.0.set_bit(7, false);

                val
            }
            10 => self.control.0.into(),
            14 => self.baud.into(),
            _ => todo!("io-port load at offset {addr}"),
        };

        T::from_u32(val)
    }

    /// The same as `load` but without side effects.
    pub(crate) fn peek<T: AddrUnit>(&self, addr: u32) -> T {
        let val: u32 = match bus::align_as::<u32>(addr) {
            0 => {
                let val: u32 = self.rx_fifo.unwrap_or(0xff).into();
                val | (val << 8) | (val << 16) | (val << 24)
            }
            4 => self.status_reg().0.into(),
            10 => self.control.0.into(),
            14 => self.baud.into(),
            _ => todo!("io-port load at offset {addr}"),
        };

        T::from_u32_aligned(val, addr)
    }

    pub fn status_reg(&self) -> StatusReg {
        let status = self
            .status
            .0
            .set_bit(0, self.tx_fifo.is_none())
            .set_bit(1, self.rx_fifo.is_some())
            .set_bit(2, {
                self.tx_fifo.is_none() && !self.state.is_transmitting()
            });

        StatusReg(status)
    }

    pub fn control_reg(&self) -> ControlReg {
        self.control
    }

    pub fn mode_reg(&self) -> ModeReg {
        self.mode
    }

    fn can_begin_transfer(&self) -> bool {
        self.tx_fifo.is_some() && self.control.select() && self.control.tx_enabled()
    }

    fn reset_device_states(&mut self) {
        self.pads.borrow_mut().reset_transfer_state();
        self.memcards.borrow_mut().reset_transfer_state();
    }

    /// Calculate the transfer time for a single byte.
    fn transfer_interval(&self) -> SysTime {
        let interval = self.baud as u64
            * self.mode.baud_reload_factor() as u64
            * self.mode.char_width() as u64;
        SysTime::from_cpu_cycles(interval)
    }

    fn begin_transfer(&mut self, schedule: &mut Schedule) {
        // Set rx_enabled.
        self.control.0.set_bit(2, true);

        self.tx_val = self.tx_fifo.take().expect("TX FIFO should be full");

        let event = schedule.schedule(
            self.transfer_interval(),
            Event::IoPort(Self::transfer),
        );

        self.state = State::InTrans(event);
    }

    fn stop_transfer(&mut self, schedule: &mut Schedule) {
        if let State::InTrans(event) | State::WaitingForAck(event) = self.state {
            schedule.unschedule(event);
        }

        self.state = State::Idle;
    }

    fn do_transfer_early(&mut self, schedule: &mut Schedule) {
        match self.state {
            State::InTrans(event) => {
                schedule.unschedule(event);
                self.transfer(schedule);
            }
            State::WaitingForAck(event) => {
                schedule.unschedule(event);
                self.ack_input(schedule);
            }
            State::Idle => (),
        }
    }

    fn transfer(&mut self, schedule: &mut Schedule) {
        let slot = self.control.io_slot();

        let (val, ack) = {
            let mut pad = self.pads.borrow_mut();
            let mut memcard = self.memcards.borrow_mut();

            // Set `rx_enabled`.
            self.control.0.set_bit(2, true);

            match self.active_device {
                None => {
                    // Select a new active device if there is no current active devices.
                    match (pad.get_mut(slot), memcard.get_mut(slot)) {
                        (None, None) => (0xff, false),
                        (Some(pad), Some(memcard)) => {
                            let (val, ack) = pad.transfer(self.tx_val);

                            if !ack {
                                if let (val, true) = memcard.transfer(self.tx_val) {
                                    self.active_device = Some(Device::MemCard);
                                    (val, true)
                                } else {
                                    (val, false)
                                }
                            } else {
                                self.active_device = Some(Device::Pad);
                                (val, true)
                            }
                        }
                        (Some(pad), None) => {
                            let (val, ack) = pad.transfer(self.tx_val);
                            if ack {
                                self.active_device = Some(Device::Pad);
                            }
                            (val, ack)
                        }
                        (None, Some(memcard)) => {
                            let (val, ack) = memcard.transfer(self.tx_val);
                            if ack {
                                self.active_device = Some(Device::MemCard);
                            }
                            (val, ack)
                        }
                    }
                }
                Some(Device::Pad) => match pad.get_mut(slot) {
                    Some(pad) => pad.transfer(self.tx_val),
                    None => (0xff, false),
                },
                Some(Device::MemCard) => match memcard.get_mut(slot) {
                    Some(memcard) => memcard.transfer(self.tx_val),
                    None => (0xff, false),
                },
            }
        };

        self.rx_fifo = Some(val);

        if ack {
            let time = if let Some(Device::MemCard) = self.active_device {
                SysTime::new(300)
            } else {
                SysTime::new(500)
            };

            self.state =
                State::WaitingForAck(schedule.schedule(time, Event::IoPort(Self::ack_input)));
        } else {
            self.state = State::Idle;
            self.active_device = None;
        }
    }

    /// Acknowledge input.
    fn ack_input(&mut self, schedule: &mut Schedule) {
        debug!("ack");

        // Set acknowledge input flag.
        self.status.0 = self.status.0.set_bit(7, true);

        if self.control.ack_irq_enabled() {
            // Set interrupt flag.
            self.status.0 = self.status.0.set_bit(9, true);
            schedule.trigger(Event::Irq(Irq::CtrlAndMemCard));
        }

        self.state = State::Idle;

        if self.can_begin_transfer() {
            self.begin_transfer(schedule);
        }
    }

    pub fn dump(&self, d: &mut impl Dumper) {
        let transfer = if self.state.waiting_for_ack() {
            "waiting for acknowledgement"
        } else {
            if self.state.is_transmitting() {
                "active"
            } else {
                "inactive"
            }
        };

        let active = self
            .active_device
            .map(|dev| dev.to_string())
            .unwrap_or_else(|| "none".to_string());

        dump!(d, "baud rate", "{}", self.baud);
        dump!(d, "active device", "{active}");
        dump!(d, "transfer", "{transfer}");
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
pub struct StatusReg(u32);

impl StatusReg {
    fn tx_ready(self) -> bool {
        self.0.bit(0)
    }

    fn rx_fifo_not_empty(self) -> bool {
        self.0.bit(1)
    }

    fn tx_done(self) -> bool {
        self.0.bit(2)
    }

    fn ack_input_low(self) -> bool {
        self.0.bit(7)
    }

    fn irq(self) -> bool {
        self.0.bit(9)
    }

    pub fn dump(&self, d: &mut impl Dumper) {
        dump!(d, "tx ready", "{}", self.tx_ready());
        dump!(d, "rx fifo not empty", "{}", self.rx_fifo_not_empty());
        dump!(d, "rx done", "{}", self.tx_done());
        dump!(d, "acknowledge input low", "{}", self.ack_input_low());
        dump!(d, "irq", "{}", self.irq());
    }
}

#[derive(Clone, Copy)]
pub struct ControlReg(u16);

impl ControlReg {
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

    /// The desired I/O slot to transfer with.
    fn io_slot(self) -> IoSlot {
        match self.0.bit(13) {
            false => IoSlot::Slot1,
            true => IoSlot::Slot2,
        }
    }

    pub fn dump(&self, d: &mut impl Dumper) {
        dump!(d, "tx enabled", "{}", self.tx_enabled());
        dump!(d, "select", "{}", self.select());
        dump!(d, "rx enabled", "{}", self.rx_enabled());
        dump!(d, "acknowledge", "{}", self.ack());
        dump!(d, "reset", "{}", self.reset());
        dump!(d, "rx irq mode", "{}", self.rx_irq_mode());
        dump!(d, "rx irq enabled", "{}", self.rx_irq_enabled());
        dump!(d, "acknowledge irq enabled", "{}", self.ack_irq_enabled());
        dump!(d, "slot", "{}", self.io_slot());
    }
}

#[derive(Clone, Copy)]
pub struct ModeReg(u16);

impl ModeReg {
    /// The factor that the baud reload value get's multiplied by.
    fn baud_reload_factor(self) -> u16 {
        match self.0.bit_range(0, 1) {
            0 | 1 => 1,
            2 => 16,
            3 => 64,
            _ => unreachable!(),
        }
    }

    /// The number of bits in each transfer.
    fn char_width(self) -> u16 {
        self.0.bit_range(2, 3) as u16 + 5
    }

    pub fn dump(&self, d: &mut impl Dumper) {
        dump!(d, "baud reload factor", "{}", self.baud_reload_factor());
        dump!(d, "character width", "{}", self.char_width());
    }
}

#[derive(Clone)]
enum State {
    Idle,
    InTrans(EventId),
    WaitingForAck(EventId),
}

impl State {
    fn is_transmitting(&self) -> bool {
        matches!(self, State::InTrans(_) | State::WaitingForAck(_))
    }

    fn waiting_for_ack(&self) -> bool {
        matches!(self, State::WaitingForAck(_))
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Device {
    Pad,
    MemCard,
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            Device::Pad => "game pad",
            Device::MemCard => "memory card",
        })
    }
}

impl BusMap for IoPort {
    const BUS_BEGIN: u32 = 0x1f801040;
    const BUS_END: u32 = Self::BUS_BEGIN + 32 - 1;
}
