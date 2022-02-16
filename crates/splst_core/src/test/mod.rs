mod cpu;
mod dma;

use crate::bus::bios::Bios;
use crate::Cpu;

fn run_cpu(cpu: &mut Cpu) {
    loop {
        let ins = cpu.curr_ins();

        // Stop if the current instruction is break.
        if ins.op() == 0x0 && ins.special() == 0xd {
            break;
        }

        cpu.step(&mut ());
    }
}

pub fn run_code(input: &str) -> Box<Cpu> {
    let base = 0x1fc00000;
    let (code, main) = match splst_asm::assemble(&[input], base) {
        Ok(res) => res,
        Err(error) => panic!("{error}"),
    };

    let bios = Bios::from_code(base, &code);
    let mut cpu = Cpu::new(bios, None);

    cpu.pc = main;
    cpu.next_pc = main + 4;

    run_cpu(&mut cpu);
    cpu
}

