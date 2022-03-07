#![feature(let_else, binary_heap_retain, option_result_contains)]

#[macro_use]
extern crate log;

#[cfg(test)]
mod test;

mod cdrom;
mod spu;

pub mod io_port;
pub mod schedule;
pub mod timer;
pub mod bus;
pub mod gpu;
pub mod timing;
pub mod cpu;

pub use cpu::Cpu;
pub use gpu::Vram;
pub use io_port::{IoSlot, Button, ButtonState, ControllerPort, Controllers};
pub use bus::bios::Bios;
pub use cdrom::Disc;

use std::time::Duration;

/// Used to represent an absolute CPU cycle number. This will never overflow, unless the emulator runs
/// for 17,725 years.
pub type Cycle = u64;

pub struct DrawInfo {
    pub vram_x_start: u32,
    pub vram_y_start: u32,
}

#[derive(PartialEq, Eq)]
pub enum StopReason {
    Time,
    Break,
}

pub trait VidOut {
    fn new_frame(&mut self, draw_info: &DrawInfo, vram: &Vram);
}

pub trait Debugger {
    /// Called when loading an instruction.
    fn instruction_load(&mut self, addr: u32);
    /// Callec when loading data. 
    fn data_load(&mut self, addr: u32);
    /// Called when storing data.
    fn data_store(&mut self, addr: u32);
    /// Called after every cycle. The ['System'] will stop if it returns true.
    fn should_stop(&mut self) -> bool;
}

// Implement debugger for unit type to easily use no debugger.
impl Debugger for () {
    fn instruction_load(&mut self, _: u32) { }

    fn data_load(&mut self, _: u32) { }

    fn data_store(&mut self, _: u32) { }

    fn should_stop(&mut self) -> bool {
        false
    }
}

/// The whole system is on ['Cpu']. This struct is to control and interact with the system
/// from the frontend.
pub struct System {
    pub cpu: Box<Cpu>,
    /// The frame number of the last frame drawn.
    last_frame: u64,
}

impl System {
    pub fn new(
        bios: Bios,
        disc: Disc,
        controllers: Controllers,
    ) -> Self {
        Self { cpu: Cpu::new(bios, disc, controllers), last_frame: 0 }
    }

    pub fn bios(&self) -> &Bios {
        &self.cpu.bus.bios()
    }

    fn maybe_draw_frame(&mut self, out: &mut impl VidOut) {
        let gpu = self.cpu.bus().gpu();

        if self.last_frame < gpu.frame_count() {
            self.last_frame = gpu.frame_count();
            out.new_frame(&gpu.draw_info(), gpu.vram());
        }
    }

    /// Run at full speed for a given amount of time.
    pub fn run(
        &mut self,
        time: Duration,
        out: &mut impl VidOut,
    ) {
        // Since 'Duration' can't be constant for now, it has to be
        // calculated each run even though the number is constant.
        let cycle_time = Duration::from_secs(1) / timing::CPU_HZ as u32;

        let cycles = time.as_nanos() / cycle_time.as_nanos();
        let end = self.cpu.bus().schedule.cycle() + cycles as u64;

        while self.cpu.bus.schedule.cycle() <= end {
            for _ in 0..16 { 
                self.cpu.step(&mut ());
            }
            self.maybe_draw_frame(out);
        }
    }

    /// Run at a given speed in debug mode. The time remainder is returned. This is required since
    /// for a couple of reasons. If running at very low speeds, then saving the remainder is
    /// required to be accurate. It's also nice to have if the ['System'] exits early.
    pub fn run_debug(
        &mut self,
        hz: u64,
        mut time: Duration,
        out: &mut impl VidOut,
        dbg: &mut impl Debugger,
    ) -> (Duration, StopReason) {
        let cycle_time = Duration::from_secs(1) / hz as u32;

        while let Some(new) = time.checked_sub(cycle_time) {
            time = new;

            self.cpu.step(dbg);
            self.maybe_draw_frame(out);

            if dbg.should_stop() {
                return (time, StopReason::Break);
            }
        }
        (time, StopReason::Time)
    }

    /// Run for a given number of cycles in debug mode.
    pub fn step_debug(
        &mut self,
        steps: u64,
        out: &mut impl VidOut,
        dbg: &mut impl Debugger,
    ) -> StopReason {
        for _ in 0..steps {
            self.cpu.step(dbg);
            self.maybe_draw_frame(out);

            if dbg.should_stop() {
                return StopReason::Break;
            }
        }
        StopReason::Time
    }
}

