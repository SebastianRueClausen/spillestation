#![feature(
    int_roundings,
    portable_simd,
    let_else,
    binary_heap_retain,
    option_result_contains,
    duration_constants,
    array_zip,
)]

#[macro_use]
extern crate log;

#[cfg(test)]
mod test;

mod cdrom;
mod fifo;

pub mod spu;
pub mod io_port;
pub mod schedule;
pub mod timer;
pub mod bus;
pub mod gpu;
pub mod cpu;
pub mod time;
pub mod debug;
pub mod dump;

use splst_util::Exe;
use io_port::pad;
use schedule::Schedule;
use cpu::irq::IrqState;

pub use cdrom::CdRom;
pub use time::{SysTime, Timestamp};
pub use bus::{dma::Dma, Bus};
pub use timer::Timers;
pub use gpu::Gpu;
pub use cpu::Cpu;
pub use gpu::Vram;
pub use bus::bios::Bios;
pub use cdrom::Disc;
pub use io_port::IoPort;

use std::time::Duration;
use std::cell::RefCell;
use std::rc::Rc;

pub struct System {
    pub cpu: Box<Cpu>,
}

impl System {
    pub fn new(
        bios: Bios,
        video_output: Rc<RefCell<dyn VideoOutput>>,
        audio_output: Rc<RefCell<dyn AudioOutput>>,
        disc: Rc<RefCell<Disc>>,
        controllers: Rc<RefCell<pad::Controllers>>,
    ) -> Self {
        Self { cpu: Cpu::new(bios, video_output, audio_output, disc, controllers) }
    }

    /// Load executable file into RAM and path the BIOS to run `exe`.
    pub fn load_exe(&mut self, exe: &Exe) {
        debug!("loading exe file");

        // `exe` should be validated, so there is no need to check the stores here.
      
        // Copy text data into RAM.
        let text_base = bus::regioned_addr(exe.text_base);
        for (i, byte) in (0..exe.text_size).zip(exe.text.iter()) {
            self.cpu.bus.store(text_base + i, *byte);
        }

        // Fill bss section with 0.
        let bss_base = bus::regioned_addr(exe.bss_base);
        for i in 0..exe.bss_size {
            self.cpu.bus.store(bss_base + i, 0_u8);
        }

        self.cpu.bus.bios.patch_for_exe(exe);
    }

    /// Run at native speed for a given amount of time.
    pub fn run(&mut self, time: Duration) {
        self.cpu.run(&mut (), SysTime::from_duration(time));
    }

    /// Run at a given speed in debug mode.
    ///
    /// Technically it doesn't run the system for `hz` cycles but `hz` instructions per second,
    /// meaning it will run faster than native speed even if `hz` is the same as the original hardware. 
    pub fn run_debug(
        &mut self,
        hz: u64,
        mut time: Duration,
        dbg: &mut impl debug::Debugger,
    ) -> (Duration, StopReason) {
        let cycle_time = Duration::from_secs(1) / hz as u32;

        while let Some(new) = time.checked_sub(cycle_time) {
            time = new;
            self.cpu.step(dbg);
            if dbg.should_break() {
                return (time, StopReason::Break);
            }
        }

        (time, StopReason::Timeout)
    }

    /// Run for a given number of instructions in debug mode.
    ///
    /// It will run instructions as fast as possible with no regard for the real speed of the
    /// system.
    pub fn step_debug(
        &mut self,
        steps: u64,
        dbg: &mut impl debug::Debugger,
    ) -> StopReason {
        for _ in 0..steps {
            self.cpu.step(dbg);
            if dbg.should_break() {
                return StopReason::Break;
            }
        }
        StopReason::Timeout
    }

    pub fn bios(&self) -> &Bios {
        &self.cpu.bus.bios
    }

    pub fn bus(&self) -> &Bus {
        &self.cpu.bus
    }
    
    pub fn dma(&self) -> &Dma {
        &self.cpu.bus.dma
    }

    pub fn irq_state(&self) -> &IrqState {
        &self.cpu.bus.irq_state
    }

    pub fn gpu(&self) -> &Gpu {
        &self.cpu.bus.gpu
    }

    pub fn schedule(&self) -> &Schedule {
        &self.cpu.bus.schedule
    }

    pub fn timers(&self) -> &Timers {
        &self.cpu.bus.timers
    }

    pub fn io_port(&self) -> &IoPort {
        &self.cpu.bus.io_port
    }

    pub fn cdrom(&self) -> &CdRom {
        &self.cpu.bus.cdrom
    }
}

/// The result of running the emulator.
#[derive(Debug, PartialEq, Eq)]
pub enum StopReason {
    /// The run session has timed out, meaning that the specified runtime has elapsed.
    Timeout,
    /// The emulator has hit a breakpoint.
    Break,
}

pub trait VideoOutput {
    fn send_frame(&mut self, vram_start: (u32, u32), vram_data: &[u16; 512 * 1024]);
}

impl VideoOutput for () {
    fn send_frame(&mut self, _: (u32, u32), _: &[u16; 512 * 1024]) {}
}

pub trait AudioOutput {
    fn send_audio(&mut self, samples: [i16; 2]);
}

impl AudioOutput for () {
    fn send_audio(&mut self, _: [i16; 2]) {}
}
