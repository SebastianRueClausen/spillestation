mod cpu;
mod gpu;
mod memory;
mod bits;

use memory::*;
use cpu::*;

fn main() {
    let bios = bios::Bios::new(include_bytes!("SCPH1001.BIN"));
    let ram = ram::Ram::new();
    let dma = dma::Dma::new();
    let gpu = gpu::Gpu::new();
    let bus = Bus::new(bios, ram, dma, gpu);
    let mut cpu = Cpu::new(bus);
    loop {
        cpu.fetch_and_exec();
    }
}
