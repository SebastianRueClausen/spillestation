#![feature(let_else, binary_heap_retain, option_result_contains, duration_constants)]

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

use splst_render::Renderer;
use schedule::Schedule;
use cpu::irq::IrqState;

pub use bus::Bus;
pub use timer::Timers;
pub use gpu::Gpu;
pub use cpu::Cpu;
pub use gpu::Vram;
pub use io_port::{IoSlot, Button, ButtonState, Controllers, controller};
pub use bus::bios::Bios;
pub use cdrom::Disc;

use std::time::Duration;
use std::cell::RefCell;
use std::rc::Rc;
use std::ops::{Add, Sub, Mul};

/// An duration of time the system is running, independent of actual time. It's represented
/// as CPU cycles. The system doesn't handle anything syb-cycle, meaning that everything can
/// broken down to CPU cycles. The cycle number is stored as 'u64', which will never
/// overflow, unless the emulator runs for 17.725 years.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SysTime(u64);

impl SysTime {
    pub const ZERO: Self = Self(0);

    /// Represents an infinite amount of time.
    pub const FOREVER: Self = Self(u64::MAX);

    /// Same as 'from_cycle'.
    pub fn new(cycles: u64) -> Self {
        Self::from_cpu_cycles(cycles)
    }

    /// From the a given amount of CPU cycles.
    pub fn from_cpu_cycles(cycles: u64) -> Self {
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
    pub fn as_cpu_cycles(self) -> u64 {
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

impl Mul for SysTime {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self(self.0 * other.0)
    }
}

pub struct System {
    pub cpu: Box<Cpu>,
    cycle_duration: Duration,
}

impl System {
    pub fn new(
        bios: Bios,
        renderer: Rc<RefCell<Renderer>>,
        disc: Rc<RefCell<Disc>>,
        controllers: Rc<RefCell<Controllers>>,
    ) -> Self {
        Self {
            cpu: Cpu::new(bios, renderer, disc, controllers),
            cycle_duration: Duration::from_secs(1) / timing::CPU_HZ as u32
        }
    }

    /// Run at native speed for a given amount of time.
    pub fn run(&mut self, time: Duration) {
        let cycles = SysTime::new((time.as_nanos() / NANOS_PER_CYCLE) as u64);
        let end = self.cpu.bus.schedule.since_startup() + cycles;

        while self.cpu.bus.schedule.since_startup() <= end {
            self.cpu.step(&mut ());
        }
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

        (time, StopReason::Time)
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
        StopReason::Time
    }

    pub fn bios(&self) -> &Bios {
        &self.cpu.bus.bios
    }

    pub fn bus_mut(&mut self) -> &mut Bus {
        &mut self.cpu.bus
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
}

#[derive(PartialEq, Eq)]
pub enum StopReason {
    Time,
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
    fn instruction_load(&mut self, _: u32) { }

    fn data_load(&mut self, _: u32) { }

    fn data_store(&mut self, _: u32) { }

    fn should_stop(&mut self) -> bool {
        false
    }
}

const NANOS_PER_CYCLE: u128 = 30;
