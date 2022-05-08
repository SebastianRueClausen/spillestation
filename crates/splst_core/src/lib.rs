#![feature(
    int_roundings,
    portable_simd,
    let_else,
    binary_heap_retain,
    option_result_contains,
    duration_constants
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
pub mod timing;
pub mod cpu;

use schedule::Schedule;
use cpu::irq::IrqState;

pub use bus::Bus;
pub use timer::Timers;
pub use gpu::Gpu;
pub use cpu::Cpu;
pub use gpu::Vram;
pub use io_port::{IoPort, IoSlot, Button, ButtonState, Controllers, controller};
pub use bus::bios::Bios;
pub use cdrom::Disc;

use std::time::Duration;
use std::cell::RefCell;
use std::rc::Rc;
use std::ops::{Add, Sub, Mul};

/// An duration of time the system is running, independent of actual time. It's represented
/// as CPU cycles. The system doesn't handle anything syb-cycle, meaning that everything can
/// broken down to CPU cycles.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SysTime(u64);

impl SysTime {
    /// An duration of zero time.
    pub const ZERO: Self = Self(0);

    /// An infinite amount of time.
    pub const FOREVER: Self = Self(u64::MAX);

    /// Same as ['from_cycle'].
    pub const fn new(cycles: u64) -> Self {
        Self::from_cpu_cycles(cycles)
    }

    /// From the a given amount of CPU cycles.
    pub const fn from_cpu_cycles(cycles: u64) -> Self {
        Self(cycles) 
    }

    /// From GPU cycles or dot cycles.
    pub fn from_gpu_cycles(cycles: u64) -> Self {
        Self((cycles as f64 / (11.0 / 7.0)) as u64)
    }

    /// From ['Duration'] the system is running at native speed.
    pub fn from_duration(duration: Duration) -> Self {
        Self((duration.as_nanos() / NANOS_PER_CYCLE) as u64) 
    }

    /// Get as amount of CPU cycles.
    pub const fn as_cpu_cycles(self) -> u64 {
        self.0
    }

    /// Get as amount of GPU cycles.
    pub fn as_gpu_cycles(self) -> u64 {
        (self.0 as f64 * (11.0 / 7.0)) as u64
    }

    /// Get as ['Duration'] the system is running as native speed.
    pub fn as_duration(self) -> Duration {
        Duration::from_nanos(NANOS_PER_CYCLE as u64) * self.0 as u32
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl Add for SysTime {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for SysTime {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl Mul<u64> for SysTime {
    type Output = Self;

    fn mul(self, other: u64) -> Self {
        Self(self.0 * other)
    }
}

/// A total amount of time since startup.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(SysTime);

impl Timestamp {
    /// The timestamp at startup.
    pub const STARTUP: Self = Self(SysTime::ZERO);

    /// A timestamp which will never be reached.
    pub const NEVER: Self = Self(SysTime::FOREVER);

    /// Create from time elapsed since startup
    pub const fn new(time: SysTime) -> Self {
        Self(time)
    }

    pub fn time_since(&self, earlier: &Self) -> SysTime {
        let cycles = self.0
            .as_cpu_cycles()
            .checked_sub(earlier.0.as_cpu_cycles())
            .expect("time earlier than self");
        SysTime::new(cycles)
    }

    pub fn time_since_startup(&self) -> SysTime {
        self.0
    }
}

impl Add<SysTime> for Timestamp {
    type Output = Self;

    fn add(self, other: SysTime) -> Self {
        Self(self.0 + other)
    }
}

pub struct System {
    pub cpu: Box<Cpu>,
}

impl System {
    pub fn new(
        bios: Bios,
        video_output: Rc<RefCell<dyn VideoOutput>>,
        audio_output: Rc<RefCell<dyn AudioOutput>>,
        disc: Rc<RefCell<Disc>>,
        controllers: Rc<RefCell<Controllers>>,
    ) -> Self {
        Self {
            cpu: Cpu::new(bios, video_output, audio_output, disc, controllers),
        }
    }

    /// Run at native speed for a given amount of time.
    pub fn run(&mut self, time: Duration) {
        self.cpu.run(&mut (), SysTime::from_duration(time));
    }

    /// Run at a given speed in debug mode.
    ///
    /// The time remainder is returned, this is for a couple of reasons if running at
    /// very low speeds, then saving the remainder is required to be accurate. It's also
    /// nice to have if the ['System'] exits early.
    ///
    /// Technically it doesn't run the system for 'hz' cycles but 'hz' instructions per second,
    /// meaning it will run faster than native speed even if 'hz' is the same as the original
    /// hardware. 
    pub fn run_debug(
        &mut self,
        hz: u64,
        mut time: Duration,
        dbg: &mut impl Debugger,
    ) -> (Duration, StopReason) {
        let cycle_time = Duration::from_secs(1) / hz as u32;

        while let Some(new) = time.checked_sub(cycle_time) {
            time = new;
            self.cpu.step(dbg);
            if dbg.should_stop() {
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
        dbg: &mut impl Debugger,
    ) -> StopReason {
        for _ in 0..steps {
            self.cpu.step(dbg);
            if dbg.should_stop() {
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

    pub fn irq_state_mut(&mut self) -> &mut IrqState {
        &mut self.cpu.bus.irq_state
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
}

/// The result of running the emulator.
#[derive(PartialEq, Eq)]
pub enum StopReason {
    /// The run session has timed out, meaning that the specified runtime has elapsed.
    Timeout,
    /// The emulator has hit a breakpoint.
    Break,
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
    fn instruction_load(&mut self, _: u32) {}

    fn data_load(&mut self, _: u32) {}

    fn data_store(&mut self, _: u32) {}

    fn should_stop(&mut self) -> bool {
        false
    }
}

pub trait VideoOutput {
    fn send_frame(&mut self, vram_start: (u32, u32), vram_data: &[u8; 1024 * 1024]);
}

impl VideoOutput for () {
    fn send_frame(&mut self, _: (u32, u32), _: &[u8; 1024 * 1024]) {}
}

pub trait AudioOutput {
    fn send_audio(&mut self, samples: [i16; 2]);
}

impl AudioOutput for () {
    fn send_audio(&mut self, _: [i16; 2]) {}
}

/// TODO: This is not very accurate.
const NANOS_PER_CYCLE: u128 = 33;
