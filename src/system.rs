use crate::{cpu::Cpu, memory::bios::Bios, timing};
use std::time::Duration;

/// The whole system is on ['Cpu']. This struct is to control and interact with the system
/// from the frontend.
pub struct System {
    pub cpu: Box<Cpu>,
}

impl System {
    pub fn new(bios: Bios) -> Self {
        Self {
            cpu: Cpu::new(bios),
        }
    }

    /// Take a single CPU step, which can be multiple cycles.
    fn cpu_step(&mut self) {
        self.cpu.fetch_and_exec();
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
    pub fn run_debug(&mut self, hz: u64, mut time: Duration) -> Duration {
        let cycle_time = Duration::from_secs(1) / hz as u32;
        while let Some(new) = time.checked_sub(cycle_time) {
            time = new;
            self.cpu_step();
        }
        time
    }

    /// Take a given number of steps.
    pub fn step_debug(&mut self, steps: u64) {
        (0..steps).for_each(|_| self.cpu_step());  
    }
}

/// How many CPU cycles between each CDROM run.
const CDROM_FREQ: u64 = 2_u64.pow(21);

/// How many CPU cycles between each timer run.
const TIMER_FREQ: u64 = 2_u64.pow(14);
