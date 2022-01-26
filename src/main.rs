use splst_front::Frontend;

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
