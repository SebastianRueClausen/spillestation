mod cpu;
mod gpu;
mod memory;
mod util;
mod front;

fn main() {
    front::run();
    /*
    let mut cpu = cpu::Cpu::new();
    loop {
        cpu.fetch_and_exec();
    }
    */
}
