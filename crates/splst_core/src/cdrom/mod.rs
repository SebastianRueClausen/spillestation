
mod fifo;
mod decoder;

use splst_util::Bit;
use splst_cdimg::CdImage;
use crate::cpu::Irq;
use crate::bus::{AddrUnit, BusMap};
use crate::schedule::{Schedule, Event};

use fifo::Fifo;

use std::fmt;

pub struct CdRom {
    cd: Option<CdImage>,
    state: DriveState,
    /// The index register. This decides what happens when the CPU writes to and
    /// loads from the CDROM.
    index: u8,
    /// Which ['Interrupt']s are enabled.
    irq_mask: u8,
    /// Which ['Interrupt']s are active.
    irq_flags: u8,
    /// The CDROM may or may not have a command waiting to be executed.
    cmd: Option<u8>,
    /// Responses from commands.
    response_fifo: Fifo,
    /// Arguments to commands.
    arg_fifo: Fifo,
    data_fifo: Fifo,
}

impl CdRom {
    pub fn new(cd: Option<CdImage>) -> Self {
        Self {
            cd,
            state: DriveState::Idle,
            index: 0x0,
            irq_mask: 0x0,
            irq_flags: 0x0,
            cmd: None,
            response_fifo: Fifo::new(),
            arg_fifo: Fifo::new(),
            data_fifo: Fifo::new(), 
        }
    }

    pub fn store<T: AddrUnit>(&mut self, schedule: &mut Schedule, addr: u32, val: u32) {
        match addr {
            0 => self.index = val.bit_range(0, 1) as u8,
            1 => match self.index {
                0 => {
                    if self.cmd.is_some() {
                        warn!("CDROM beginning command while command is pending");
                    }
                    self.cmd = Some(val as u8);
                    self.exec_cmd(schedule);
                },
                _ => todo!(),
            }
            2 => match self.index {
                0 => self.arg_fifo.push(val as u8),
                1 => {
                    let was_active = self.irq_active();
                    self.irq_mask = val.bit_range(0, 4) as u8;
                    if !was_active && self.irq_active() {
                        schedule.schedule_now(Event::IrqTrigger(Irq::CdRom));
                    }
                }
                2 => todo!(),
                _ => unreachable!(),
            }
            3 => match self.index {
                0 => todo!(),
                1 => {
                    self.irq_flags &= !(val.bit_range(0, 4) as u8);
                    if val.bit(6) {
                        self.arg_fifo.clear();
                    }
                }
                2 => todo!(),
                _ => unreachable!(),
            }
            _ => unreachable!(),
        }
    }

    pub fn load<T: AddrUnit>(&mut self, addr: u32) -> u32 {
        match addr {
            // Status register.
            0 => {
                let register = self.index
                    | (self.arg_fifo.is_empty() as u8) << 3
                    | (!self.arg_fifo.is_full() as u8) << 4
                    | (!self.response_fifo.is_empty() as u8) << 5
                    | (!self.data_fifo.is_empty() as u8) << 6
                    | (self.cmd.is_some() as u8) << 7;
                register.into()
            }
            1 => self.response_fifo.try_pop().unwrap_or(0x0).into(),
            2 => self.data_fifo.pop().into(),
            3 => match self.index {
                0 => self.irq_mask as u32 | !0x1f,
                1 => self.irq_flags as u32 | !0x1f,
                2 => todo!(),
                _ => unreachable!(),
            }
            _ => unreachable!(),
        }
    }

    fn finish_cmd(&mut self, schedule: &mut Schedule, irq: Interrupt) {
        self.response_fifo.push(self.drive_stat());
        self.set_interrupt(schedule, irq);
    }

    pub fn run(&mut self, schedule: &mut Schedule) {
        self.exec_cmd(schedule);
        schedule.schedule_in(10_000, Event::RunCdRom);
    }

    pub fn reponse(&mut self, cmd: CdRomCmd) {
        debug!("CDROM reponse for command: {}", cmd);
        match cmd.0 {
            // Init.
            0x0a => {
                self.response_fifo.push(self.drive_stat());
            }
            // Read Table of Content.
            0x1e => {
                self.response_fifo.push(self.drive_stat());
            }
            _ => todo!(),
        }
    }

    fn exec_cmd(&mut self, schedule: &mut Schedule) {
        if self.irq_flags != 0 {
            return;
        }

        if let Some(cmd) = self.cmd.take() {
            self.response_fifo.clear();
            debug!("CDROM command {}", CdRomCmd(cmd));
            match cmd {
                // Status.
                0x01 => {
                    self.finish_cmd(schedule, Interrupt::Ack);
                }
                // Init.
                0x0a => {
                    self.finish_cmd(schedule, Interrupt::Ack);
                    self.state = DriveState::Paused;
                    schedule.schedule_in(900_000, Event::CdRomResponse(CdRomCmd(0x0a)));
                }
                // Test command. It's behavior depent on the first argument.
                0x19 => match self.arg_fifo.pop() {
                    0x20 => {
                        // These represent year, month, day and version respectively.
                        self.response_fifo.push_slice(&[0x98, 0x06, 0x10, 0xc3]);
                        self.set_interrupt(schedule, Interrupt::Ack);
                    }
                    _ => todo!(),
                }
                // Get ID.
                0x1a => {
                    self.state = DriveState::Reading;
                    self.finish_cmd(schedule, Interrupt::Ack);
                    self.response_fifo.push_slice(&[0x11, 0x80]);
                }
                // Read Table of Content.
                0x1e => {
                    debug!("Read table of content");
                    self.response_fifo.push(self.drive_stat());
                    // This might not take as long without a disc.
                    schedule.schedule_in(30_000_000, Event::CdRomResponse(CdRomCmd(0x1e)));
                }
                _ => todo!("CDROM Command: {:08x}", cmd),
            }

            self.arg_fifo.clear();
        }
    }

    fn set_interrupt(&mut self, schedule: &mut Schedule, int: Interrupt) {
        self.irq_flags = int as u8;
        if self.irq_active() {
            schedule.schedule_now(Event::IrqTrigger(Irq::CdRom));
        }
    }

    fn irq_active(&self) -> bool {
        (self.irq_flags & self.irq_mask) != 0
    }

    fn drive_stat(&self) -> u8 {
        if self.cd.is_none() {
            // This means that the drive cover is open.
            0x10
        } else {
            match self.state {
                DriveState::Idle => 0,
                DriveState::Paused => (1 << 1),
                DriveState::Reading => (1 << 1) | (1 << 5),
            }
        }
    }
}

#[derive(PartialEq, Eq)]
pub struct CdRomCmd(u8);

impl fmt::Display for CdRomCmd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            0x1 => write!(f, "status"),
            0xa => write!(f, "init"),
            0x19 => write!(f, "test"),
            0x1a => write!(f, "get_id"),
            0x1e => write!(f, "read_toc"),
            _ => unreachable!(),
        }
    }
}
/// Interrupt types used internally by the CDROM.
#[derive(Clone, Copy)]
enum Interrupt {
    Ack = 0x3,
    // Error = 0x5,
}

#[derive(Clone, Copy)]
enum DriveState {
    Idle,
    Paused,
    Reading,
}

impl BusMap for CdRom {
    const BUS_BEGIN: u32 = 0x1f801800;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}