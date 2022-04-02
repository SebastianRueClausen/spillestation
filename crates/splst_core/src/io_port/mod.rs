#![allow(dead_code)]

pub mod controller;
mod memcard;

use splst_util::{Bit, BitSet};
use crate::bus::BusMap;
use crate::schedule::{Schedule, Event, EventId};
use crate::cpu::Irq;
use crate::SysTime;

use memcard::MemCard;

use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;

pub use controller::{Button, ButtonState, Controllers};

/// Controller and Memory Card I/O ports.
pub struct IoPort {
    state: State,
    /// The device currently active. May be either a controller or memory card.
    active_device: Option<Device>,
    /// Control register.
    ctrl: CtrlReg,
    /// Status register.
    stat: StatReg,
    /// Mode register.
    mode: ModeReg,
    /// # Baudrate
    ///
    /// Baudrate determines the amount of transfers of data between the I/O port and the connected
    /// device per second. The transfer size is usually 8 bits, although it can also be either 5, 6
    /// or 7, which is determined by the mode register.
    ///
    /// 'baud' is the reload value for the baud timer stored in bit 11..31 of 'stat'. The baud timer
    /// is always running and decreases at ~33MHz (CPU clock rate) and it ellapses twice for each
    /// bit.
    ///
    /// The timer reloads when writing to the 'baud' register or when the timer reaches zero. The timer
    /// is set to 'baud' multiplied by the baudrate factor, mode register bits 0..1 (almost always 1),
    /// and then divided by 2.
    baud: u16,
    /// Recieve buffer. This is the data that the I/O port recieves from the peripherals. It's
    /// technically a FIFO queue of bits, but we only emulate sending a whole byte at once. It's
    /// cleared whenever the register is read.
    rx_fifo: Option<u8>,
    /// Transmit buffer. This is the data that get's send to the peripherals. The buffer is filled
    /// by writing to the register and cleared when send to the either a controller or memory card.
    tx_fifo: Option<u8>,
    /// Intermidate buffer to store the content of 'tx_fifo' starting a transfer and actually
    /// sending the data to the devices.
    tx_val: u8,
    memcards: [Option<MemCard>; 2],
    controllers: Rc<RefCell<Controllers>>,
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
        match addr {
            0 => {
                if self.tx_fifo.is_some() {
                    warn!("Write to TX FIFO while full");
                }

                self.tx_fifo = Some(val as u8);

                if self.can_begin_transfer() && !self.state.in_transfer() {
                    self.begin_transfer(schedule);
                }
            }
            8 => self.mode = ModeReg(val as u16),
            10 => {
                self.ctrl = CtrlReg(val as u16);

                if self.ctrl.reset() {
                    trace!("io port control reset");

                    if let State::InTrans { event, .. } = self.state {
                        schedule.unschedule(event);
                        self.state = State::Idle;
                    }

                    self.reset_device_states();

                    self.stat = StatReg(0);
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
                    if let State::InTrans { event, .. } = self.state {
                        schedule.unschedule(event);
                        self.state = State::Idle;
                    }
                } else {
                    if !self.state.in_transfer() && self.can_begin_transfer() {
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
                if let State::InTrans { event, .. } = self.state {
                    schedule.trigger_early(event);
                }

                let val: u32 = self.rx_fifo
                    .take()
                    .unwrap_or(0xff)
                    .into();

                val | (val << 8) | (val << 16) | (val << 24)
            }
            4 => {
                if let State::InTrans { event, .. } = self.state {
                    schedule.trigger_early(event);
                }

                let val: u32 = self.stat_reg().0.into();

                // TODO: Emulate BAUD timer.

                // Set acknownledge input false.
                self.stat.0 = self.stat.0.set_bit(4, false);

                val
            }
            10 => self.ctrl.0.into(),
            14 => self.baud.into(),
            _ => todo!("IoPort load at offset {addr}"),
        }
    }

    pub fn stat_reg(&self) -> StatReg {
        let status = self.stat.0
            .set_bit(0, self.tx_fifo.is_none())
            .set_bit(1, self.rx_fifo.is_none())
            .set_bit(2, {
                self.tx_fifo.is_none() && !self.state.in_transfer()
            });

        StatReg(status)
    }

    pub fn ctrl_reg(&self) -> CtrlReg {
        self.ctrl
    }

    pub fn mode_reg(&self) -> ModeReg {
        self.mode
    }

    pub fn baud(&self) -> u16 {
        self.baud
    }

    pub fn in_transfer(&self) -> bool {
        self.state.in_transfer()
    }

    pub fn waiting_for_ack(&self) -> bool {
        self.state.waiting_for_ack()
    }

    pub fn active_device(&self) -> Option<Device> {
        self.active_device
    }

    fn can_begin_transfer(&self) -> bool {
        let can_begin = self.tx_fifo.is_some()
            && self.ctrl.select()
            && self.ctrl.tx_enabled();
        can_begin
    }
    
    fn reset_device_states(&mut self) {
        self.controllers
            .borrow_mut()
            .iter_mut()
            .for_each(|c| c.reset());
    }

    /// Calculate the transfer time for a single byte.
    fn transfer_interval(&self) -> SysTime {
        let interval = self.baud as u64
            * self.mode.baud_reload_factor()
            * self.mode.char_width();
        SysTime::from_cpu_cycles(interval)
    }

    fn begin_transfer(&mut self, schedule: &mut Schedule) {
        // Set rx_enabled.
        self.ctrl.0.set_bit(2, true);

        self.tx_val = self.tx_fifo
            .take()
            .expect("TX FIFO should be full");

        let event = schedule.schedule_repeat(
            self.transfer_interval(),
            Event::IoPort(Self::transfer)
        );

        self.state = State::InTrans {
            event, waiting_for_ack: false,
        };
    }

    pub fn transfer(&mut self, schedule: &mut Schedule) {
        match self.state {
            State::InTrans { ref mut event, ref mut waiting_for_ack } if !*waiting_for_ack => {
                let index = self.ctrl.io_slot() as usize;

                let ctrl = &mut self.controllers.borrow_mut()[self.ctrl.io_slot()];
                let memcard = &mut self.memcards[index];

                // Set rx_enabled.
                self.ctrl.0.set_bit(2, true);

                let (val, ack) = match self.active_device {
                    None => {
                        // Select a new active device if there is no current active devices.
                        match (ctrl, memcard) {
                            (controller::Port::Unconnected, None) => (0xff, false),
                            (controller::Port::Digital(ctrl), _) => {
                                let (val, ack) = ctrl.transfer(self.tx_val);
                                if ack {
                                    debug!("active controller");
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
                        controller::Port::Digital(ctrl) => {
                            trace!("controller transfer");
                            ctrl.transfer(self.tx_val)
                        },
                        controller::Port::Unconnected => (0xff, false),
                    }
                    Some(Device::MemCard) => match memcard {
                        Some(memcard) => memcard.transfer(self.tx_val),
                        None => (0xff, false),
                    }
                };

                self.rx_fifo = Some(val);

                if ack {
                    let time = match self.active_device {
                        Some(Device::MemCard) => SysTime::new(170),
                        _ => SysTime::new(450),
                    };
                    
                    schedule.unschedule(*event);

                    *event = schedule.schedule(time, Event::IoPort(Self::transfer));
                    *waiting_for_ack = true;
                } else {
                    schedule.unschedule(*event);

                    self.state = State::Idle;
                    self.active_device = None;
                }
            }
            State::Idle => {
                self.ack_input(schedule);

                if self.can_begin_transfer() {
                    self.begin_transfer(schedule);
                }
            }
            State::InTrans { event, .. } => {
                self.ack_input(schedule);

                self.state = State::Idle;
                schedule.unschedule(event);

                if self.can_begin_transfer() {
                    self.begin_transfer(schedule);
                }
            }
        }
    }

    fn ack_input(&mut self, schedule: &mut Schedule) {
        // Set acknowledge input flag.
        self.stat.0 = self.stat.0.set_bit(7, true);

        if self.ctrl.ack_irq_enabled() {
            // Set interrupt flag.
            self.stat.0 = self.stat.0.set_bit(9, true);

            schedule.trigger(Event::Irq(Irq::CtrlAndMemCard));
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
pub struct StatReg(u32);

impl StatReg {
    pub fn tx_ready(self) -> bool {
        self.0.bit(0)
    }

    pub fn rx_fifo_not_empty(self) -> bool {
        self.0.bit(1)
    }

    pub fn tx_done(self) -> bool {
        self.0.bit(2)
    }

    pub fn ack_input_lvl(self) -> bool {
        self.0.bit(7)
    }

    pub fn irq(self) -> bool {
        self.0.bit(9)
    }
}

#[derive(Clone, Copy)]
pub struct CtrlReg(u16);

impl CtrlReg {
    pub fn tx_enabled(self) -> bool {
        self.0.bit(0)
    }

    pub fn select(self) -> bool {
        self.0.bit(1)
    }

    pub fn rx_enabled(self) -> bool {
        self.0.bit(2)
    }

    pub fn ack(self) -> bool {
        self.0.bit(4)
    }

    pub fn reset(self) -> bool {
        self.0.bit(6)
    }

    /// RX Interrupt mode. This tells when it should IRQ, in relation to how many bytes the RX FIFO
    /// contains. It's either 1, 2, 4 or 8.
    pub fn rx_irq_mode(self) -> u32 {
        match self.0.bit_range(8, 9) {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => unreachable!(),
        }
    }

    pub fn tx_irq_enabled(self) -> bool {
        self.0.bit(10)
    }

    pub fn rx_irq_enabled(self) -> bool {
        self.0.bit(11)
    }

    pub fn ack_irq_enabled(self) -> bool {
        self.0.bit(12)
    }

    /// The desired I/O slot to transfer with.
    pub fn io_slot(self) -> IoSlot {
        match self.0.bit(13) {
            false => IoSlot::Slot1,
            true => IoSlot::Slot2,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ModeReg(u16);

impl ModeReg {
    /// The factor that the baud reload value get's multiplied by.
    pub fn baud_reload_factor(self) -> u64 {
        match self.0.bit_range(0, 1) {
            0 | 1 => 1,
            2 => 16,
            3 => 64,
            _ => unreachable!(),
        }
    }

    /// The number of bits in each transfer.
    pub fn char_width(self) -> u64 {
        self.0.bit_range(2, 3) as u64 + 5
    }
}

#[derive(Clone)]
enum State {
    Idle,
    InTrans {
        event: EventId,
        waiting_for_ack: bool,
    }
}

impl State {
    fn in_transfer(&self) -> bool {
        matches!(self, State::InTrans { .. })
    }

    fn waiting_for_ack(&self) -> bool {
        matches!(self, State::InTrans { waiting_for_ack: true, .. })
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Device {
    Controller,
    MemCard,
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            Device::Controller => "controller",
            Device::MemCard => "memory card",
        })
    }
}

impl BusMap for IoPort {
    const BUS_BEGIN: u32 = 0x1f801040;
    const BUS_END: u32 = Self::BUS_BEGIN + 32 - 1;
}
