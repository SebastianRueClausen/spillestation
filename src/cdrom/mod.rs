
mod fifo;

use fifo::Fifo;
use crate::{util::BitExtract, cpu::{IrqState, Irq}, bus::{AddrUnit, BusMap}};

/// Interrupt types used internally by the Playstations CDROM.
#[derive(Clone, Copy)]
enum Interrupt {
    Ack = 0x3,
    Error = 0x5,
}

pub struct CdRom {
    /// The index register. This decides what happens when the CPU writes to and
    /// loads from the CDROM.
    index: u8,
    /// Which ['Interrupt']s are enabled.
    irq_mask: u8,
    /// Which ['Interrupt']s are active.
    irq_flags: u8,
    /// The CDROM may or may not have a command waiting to be executed.
    cmd: Option<u8>,
    /// Sometimes the response or part of the response from commands may take some time to be
    /// available. This is uesd to emulate that. The Option contains the command number.
    cmd_response: Option<u8>,
    /// Responses from commands.
    response_fifo: Fifo,
    /// Arguments to commands.
    arg_fifo: Fifo,
    data_fifo: Fifo,
}

impl CdRom {
    pub fn new() -> Self {
        Self {
            index: 0x0,
            irq_mask: 0x0,
            irq_flags: 0x0,
            cmd: None,
            cmd_response: None,
            response_fifo: Fifo::new(),
            arg_fifo: Fifo::new(),
            data_fifo: Fifo::new(), 
        }
    }

    pub fn store<T: AddrUnit>(&mut self, irq: &mut IrqState, addr: u32, val: u32) {
        match addr {
            0 => self.index = val.extract_bits(0, 1) as u8,
            1 => match self.index {
                0 => {
                    if self.cmd.is_some() {
                        warn!("CDROM beginning command while command is pending")
                    }
                    self.cmd = Some(val as u8);
                    self.exec_cmd(irq);
                },
                _ => todo!(),
            }
            2 => match self.index {
                0 => self.arg_fifo.push(val as u8),
                1 => {
                    let was_active = self.irq_active();
                    self.irq_mask = val as u8 & 0x1f;
                    if !was_active && self.irq_active() {
                        irq.trigger(Irq::CdRom); 
                    }
                }
                2 => todo!(),
                _ => unreachable!(),
            }
            3 => match self.index {
                0 => todo!(),
                1 => {
                    self.irq_flags &= !(val as u8 & 0x1f);
                    if val.extract_bit(6) == 1  {
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


    pub fn exec_cmd(&mut self, irq: &mut IrqState) {
        if let Some(cmd) = self.cmd_response.take() {
            match cmd {
                0x1e => self.cmd_read_toc_response(),
                _ => unreachable!(),
            }
        }
        if let Some(cmd) = self.cmd.take() {
            match cmd {
                0x01 => self.cmd_stat(irq),
                0x19 => self.cmd_test(irq),
                0x1a => self.cmd_get_id(irq),
                0x1e => self.cmd_read_toc(),
                _ => todo!("CDROM Command: {:08x}", cmd),
            }
            self.arg_fifo.clear();
        }
    }

    fn set_interrupt(&mut self, irq: &mut IrqState, int: Interrupt) {
        self.irq_flags = int as u8;
        if self.irq_active() {
            irq.trigger(Irq::CdRom);
        }
    }

    fn irq_active(&self) -> bool {
        (self.irq_flags & self.irq_mask) != 0
    }

    fn drive_stat(&self) -> u8 {
        // This means that the drive cover is open.
        0x10
    }

    fn cmd_stat(&mut self, irq: &mut IrqState) {
        self.response_fifo.push(self.drive_stat());
        self.set_interrupt(irq, Interrupt::Ack);
    }

    fn cmd_test(&mut self, irq: &mut IrqState) {
        match self.arg_fifo.pop() {
            0x20 => {
                // These represent year, month, day and version respectively.
                self.response_fifo.push_slice(&[0x98, 0x06, 0x10, 0xc3]);
                self.set_interrupt(irq, Interrupt::Ack);
            }
            _ => todo!(),
        }
    }

    fn cmd_get_id(&mut self, irq: &mut IrqState) {
        self.response_fifo.push_slice(&[0x11, 0x80]);
        self.set_interrupt(irq, Interrupt::Error);
    }

    fn cmd_read_toc(&mut self) {
        self.response_fifo.push(self.drive_stat());
        self.cmd_response = Some(0x1e);
    }

    fn cmd_read_toc_response(&mut self) {
        self.response_fifo.push(self.drive_stat());
    }
}

impl BusMap for CdRom {
    const BUS_BEGIN: u32 = 0x1f801800;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}
