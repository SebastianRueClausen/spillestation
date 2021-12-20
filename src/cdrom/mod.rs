
mod fifo;

use fifo::Fifo;
use crate::util::bits::BitExtract;

pub struct CdRom {
    index: u8,
    irq_mask: u8,
    irq_flags: u8,
    command: Option<u8>,
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
            command: None,
            response_fifo: Fifo::new(),
            arg_fifo: Fifo::new(),
            data_fifo: Fifo::new(), 
        }
    }

    pub fn store(&mut self, offset: u32, value: u32) {
        match offset {
            0 => self.index = value.extract_bits(0, 1) as u8,
            1 => match self.index {
                0 => self.command = Some(value as u8),
                _ => {},
            }
            2 => match self.index {
                0 => self.arg_fifo.push(value as u8),
                _ => {},
            }
            3 => {}
            _ => unreachable!(),
        }
    }

    pub fn load(&mut self, offset: u32) -> u32 {
        match offset {
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
            3 => match self.index {
                0 => self.irq_mask as u32 | 0xe0,
                1 => self.irq_flags as u32 | 0xe0,
                _ => todo!(),
            }
            _ => unreachable!(),
        }
    }

    pub fn exec_cmd(&mut self) {
        if let Some(cmd) = self.command.take() {
            println!("Ran Cmd {:08x}", cmd);
            match cmd {
                0x19 => self.cmd_test(),
                _ => todo!(),
            }
        }
    }

    fn cmd_test(&mut self) {
        match self.arg_fifo.pop() {
            // Get version data. This is ofcourse different from console to console.
            0x20 => {
                // These represent year, month, day and version respectively.
                self.response_fifo.push_slice(&[0x99, 0x2, 0x01, 0xc3]);
            }
            _ => todo!(),
        }
    }
}
