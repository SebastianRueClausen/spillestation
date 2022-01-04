mod cpu;
mod front;
mod gpu;
mod bus;
mod cdrom;
mod util;
mod timer;
mod timing;
mod system;
mod spu;

fn main() {
    front::run();
}
