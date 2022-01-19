#![feature(let_else, binary_heap_retain)]

#[macro_use]
extern crate log;

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
mod io_port;
mod asm;

use front::Frontend;

use log::LevelFilter;
use std::io::Write;

fn main() {
    env_logger::Builder::new()
        .format(|f, record| {
            writeln!(f, "{}: {}", record.level(), record.args())
        })
        .filter_module("wgpu", LevelFilter::Error)
        .filter_module("winit", LevelFilter::Error)
        .filter_module("naga", LevelFilter::Error)
        .filter(None, LevelFilter::Debug)
        .init();

    Frontend::new().run();
}
