mod cpu;
mod gpu;
mod memory;
mod util;
mod front;

use memory::*;
use cpu::*;
use front::*;

fn main() {
    let bios = bios::Bios::new(include_bytes!("SCPH1001.BIN"));
    let ram = ram::Ram::new();
    let dma = dma::Dma::new();
    let gpu = gpu::Gpu::new();
    let bus = Bus::new(bios, ram, dma, gpu);
    let mut cpu = Cpu::new(bus);
    Frontend::new().run();
    loop {
        cpu.fetch_and_exec();
    }
}
