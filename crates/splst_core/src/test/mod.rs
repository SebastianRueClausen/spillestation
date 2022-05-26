mod cpu;
mod dma;

use crate::bus::bios::Bios;
use crate::Cpu;

use std::cell::RefCell;
use std::rc::Rc;

pub fn run_code(input: &str) -> Box<Cpu> {
    let base = 0x1fc00000;
    let (code, main) = match splst_asm::assemble(input, base) {
        Ok(res) => res,
        Err(error) => panic!("{error}"),
    };

    let mut cpu = Cpu::new(
        Bios::from_code(base, &code),
        Rc::new(RefCell::new(())),
        Rc::new(RefCell::new(())),
        Default::default(),
        Default::default(),
    );

    cpu.pc = main;
    cpu.next_pc = main + 4;

    cpu
}
