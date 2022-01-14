use crate::cpu::Cpu;
use crate::bus::bios::Bios;
use crate::timing;

use std::time::Duration;

/// Used to represent an absolute CPU cycle number. This will never overflow, unless the emulator runs
/// for 17,725 years.
pub type Cycle = u64;

#[derive(PartialEq, Eq)]
pub enum DebugStop {
    Breakpoint(u32),
    Time,
}

/// Breakpoints which can be supplied to ['System::debug_step'] and ['System::debug_run'].
pub struct Breaks<'a> {
    pub code: &'a [u32],
    pub store: &'a [u32],
    pub load: &'a [u32],
}

/// The whole system is on ['Cpu']. This struct is to control and interact with the system
/// from the frontend.
pub struct System {
    pub cpu: Box<Cpu>,
}

impl System {
    pub fn new(bios: Bios) -> Self {
        Self { cpu: Cpu::new(bios) }
    }

    /// Take a single CPU step, which can be multiple cycles.
    fn cpu_step(&mut self) {
        self.cpu.step();
    }

    /// Run at full speed.
    pub fn run(&mut self, mut time: Duration) {
        let cycle_time = Duration::from_secs(1) / timing::CPU_HZ as u32;
        while let Some(new) = time.checked_sub(cycle_time) {
            time = new;
            self.cpu_step();
        }
    }


    /// Run at a given speed in debug mode. Returns the remainder.
    pub fn run_debug(&mut self, hz: u64, mut time: Duration, breaks: Breaks) -> (Duration, DebugStop) {
        let cycle_time = Duration::from_secs(1) / hz as u32;
        while let Some(new) = time.checked_sub(cycle_time) {
            time = new;
            self.cpu_step();
            if breaks.code.contains(&self.cpu.next_pc) {
                return (time, DebugStop::Breakpoint(self.cpu.next_pc));
            }
        }
        (time, DebugStop::Time)
    }

    /// Run for a given number of cycles in debug mode.
    pub fn step_debug(&mut self, steps: u64, breaks: Breaks) -> DebugStop {
        for _ in 0..steps {
            self.cpu_step();
            if breaks.code.contains(&self.cpu.next_pc) {
                return DebugStop::Breakpoint(self.cpu.next_pc);
            }
        }
        DebugStop::Time
    }
}
