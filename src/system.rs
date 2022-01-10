use crate::{cpu::Cpu, bus::bios::Bios, timing};
use std::time::Duration;

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
        let cycle = self.cpu.bus().cycle_count;
        if cycle % CDROM_FREQ == 0 {
            self.cpu.bus_mut().run_cdrom();
        }
        if cycle % TIMER_FREQ == 0 {
            self.cpu.bus_mut().run_timers();
        }
        if cycle % GPU_FREQ == 0 {
            self.cpu.bus_mut().run_gpu();
        }
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
    pub fn run_debug<'a>(
        &mut self,
        hz: u64,
        mut time: Duration,
        breaks: Breaks<'a>,
    ) -> (Duration, DebugStop) {
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
    pub fn step_debug<'a>(&mut self, steps: u64, breaks: Breaks<'a>) -> DebugStop {
        for _ in 0..steps {
            self.cpu_step();
            if breaks.code.contains(&self.cpu.next_pc) {
                return DebugStop::Breakpoint(self.cpu.next_pc);
            }
        }
        DebugStop::Time
    }
}

const CDROM_FREQ: u64 = 2_u64.pow(12);
const TIMER_FREQ: u64 = 2_u64.pow(11);
const GPU_FREQ: u64 = 2_u64.pow(12);
