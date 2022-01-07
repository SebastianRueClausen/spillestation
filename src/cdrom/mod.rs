
mod fifo;

use fifo::Fifo;
use crate::{util::BitExtract, cpu::{IrqState, Irq}, bus::{AddrUnit, BusMap}};

#[derive(Clone, Copy)]
enum Interrupt {
    Ack = 0x3,
}

pub struct CdRom {
    index: u8,
    irq_mask: u8,
    irq_flags: u8,
    cmd: Option<u8>,
    response_fifo: Fifo,
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
            response_fifo: Fifo::new(),
            arg_fifo: Fifo::new(),
            data_fifo: Fifo::new(), 
        }
    }

    pub fn store<T: AddrUnit>(&mut self, addr: u32, val: u32) {
        println!("store {}", addr);
        match addr {
            0 => self.index = val.extract_bits(0, 1) as u8,
            1 => match self.index {
                0 => self.cmd = Some(val as u8),
                _ => todo!(),
            }
            2 => match self.index {
                0 => self.arg_fifo.push(val as u8),
                1 => self.irq_mask = val as u8 & 0x1f,
                2 => todo!(),
                _ => unreachable!(),
            }
            3 => match self.index {
                0 => todo!(),
                1 => self.irq_flags &= !(val as u8 & 0x1f),
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
                    | (!self.data_fifo.is_empty() as u8) << 6;
                register.into()
            }
            1 => self.response_fifo.pop() as u32,
            2 => self.data_fifo.pop() as u32,
            3 => {
                match self.index {
                    0 => self.irq_mask as u32 | 0xe0,
                    1 => self.irq_flags as u32 | 0xe0,
                    2 => todo!(),
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }


    pub fn exec_cmd(&mut self, irq: &mut IrqState) {
        if let Some(cmd) = self.cmd.take() {
            match cmd {
                0x19 => self.cmd_test(irq),
                _ => todo!(),
            }
        }
    }

    fn set_interrupt(&mut self, irq: &mut IrqState, int: Interrupt) {
        self.irq_flags = int as u8;
        if (self.irq_flags & self.irq_mask) != 0 {
            irq.trigger(Irq::CdRom);
        }
    }

    fn cmd_test(&mut self, irq: &mut IrqState) {
        match self.arg_fifo.pop() {
            // Get version data. This is ofcourse different from console to console.
            0x20 => {
                // These represent year, month, day and version respectively.
                self.response_fifo.push_slice(&[0x95, 0x05, 0x16, 0xc1]);
                self.set_interrupt(irq, Interrupt::Ack);
            }
            _ => todo!(),
        }
        self.arg_fifo.clear();
    }
}

impl BusMap for CdRom {
    const BUS_BEGIN: u32 = 0x1f801800;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}
