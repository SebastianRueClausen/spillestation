use crate::cpu::Cpu;
use crate::bus::bios::Bios;
use crate::timing;
use crate::render::Renderer;

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
    /// The frame number of the last frame drawn.
    last_frame: u64,
}

impl System {
    pub fn new(bios: Bios) -> Self {
        Self {
            cpu: Cpu::new(bios),
            last_frame: 0,
        }
    }

    fn maybe_draw_frame(&mut self, renderer: &Renderer) {
        let gpu = self.cpu.bus().gpu();
        if self.last_frame < gpu.frame_count() {
            self.last_frame = gpu.frame_count();
            renderer.send_frame(
                gpu.draw_info(),
                gpu.vram(),
            );
        }
    }

    /// Run at full speed for a given amount of time.
    pub fn run(&mut self, time: Duration, renderer: &Renderer) {
        let cycle_time = Duration::from_secs(1) / timing::CPU_HZ as u32;
        let cycles = time.as_nanos() / cycle_time.as_nanos();

        let outer = cycles / 16;

        for _ in 0..outer {
            for _ in 0..16 { 
                self.cpu.step();
            }
            self.maybe_draw_frame(renderer);
        }
    }

    /// Run at a given speed in debug mode. Returns the remainder.
    pub fn run_debug(
        &mut self,
        hz: u64,
        mut time: Duration,
        renderer: &Renderer,
        breaks: Breaks
    ) -> (Duration, DebugStop) {
        let cycle_time = Duration::from_secs(1) / hz as u32;
        while let Some(new) = time.checked_sub(cycle_time) {
            time = new;
            self.cpu.step();
            self.maybe_draw_frame(renderer);
            if breaks.code.contains(&self.cpu.next_pc) {
                return (time, DebugStop::Breakpoint(self.cpu.next_pc));
            }
        }
        (time, DebugStop::Time)
    }

    /// Run for a given number of cycles in debug mode.
    pub fn step_debug(&mut self, steps: u64, renderer: &Renderer, breaks: Breaks) -> DebugStop {
        for _ in 0..steps {
            self.cpu.step();
            self.maybe_draw_frame(renderer);
            if breaks.code.contains(&self.cpu.next_pc) {
                return DebugStop::Breakpoint(self.cpu.next_pc);
            }
        }
        DebugStop::Time
    }
}
