mod cpu;
mod memory;

use memory::*;
use cpu::*;

fn main() {
    let bios = bios::Bios::new(include_bytes!("SCPH1001.BIN"));
    let ram = ram::Ram::new();
    let mut cpu = Cpu::new(Bus::new(bios, ram));
    loop {
        cpu.fetch_and_exec();
    }
}
