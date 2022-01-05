use crate::{cpu::Cpu, bus::bios::Bios, timing};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Debug,
    Emulation,
}

pub struct Debugger {
    pub breakpoints: Vec<u32>,
    pub changed_addr: Option<u32>,
}

impl Debugger {
    fn new() -> Self {
        Self {
            breakpoints: vec![],
            changed_addr: None,
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum DebugStop {
    Breakpoint(u32),
    Time,
}

/// The whole system is on ['Cpu']. This struct is to control and interact with the system
/// from the frontend.
pub struct System {
    pub cpu: Box<Cpu>,
    pub dbg: Debugger,
}

impl System {
    pub fn new(bios: Bios) -> Self {
        Self {
            cpu: Cpu::new(bios),
            dbg: Debugger::new(),
        }
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
    }

    /// Run at full speed.
    pub fn run(&mut self, mut time: Duration) {
        let cycle_time = Duration::from_secs(1) / timing::CPU_HZ as u32;
        while let Some(new) = time.checked_sub(cycle_time) {
            time = new;
            self.cpu_step();
        }
    }


    /// Run at a given speed. Returns the remainder.
    pub fn run_debug(&mut self, hz: u64, mut time: Duration) -> (Duration, DebugStop) {
        let cycle_time = Duration::from_secs(1) / hz as u32;
        while let Some(new) = time.checked_sub(cycle_time) {
            time = new;
            self.cpu_step();
            if self.dbg.breakpoints.contains(&self.cpu.next_pc) {
                return (time, DebugStop::Breakpoint(self.cpu.next_pc));
            }
        }
        (time, DebugStop::Time)
    }

    /// Take a given number of steps.
    pub fn step_debug(&mut self, steps: u64) -> DebugStop {
        for _ in 0..steps {
            self.cpu_step();
            if self.dbg.breakpoints.contains(&self.cpu.next_pc) {
                return DebugStop::Breakpoint(self.cpu.next_pc);
            }
        }
        DebugStop::Time
    }
}

/// How many CPU cycles between each CDROM run.
const CDROM_FREQ: u64 = 2_u64.pow(21);

/// How many CPU cycles between each timer run.
const TIMER_FREQ: u64 = 2_u64.pow(14);
