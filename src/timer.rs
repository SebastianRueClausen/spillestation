use crate::util::{Bit, BitSet};
use crate::cpu::Irq;
use crate::timing;
use crate::bus::{Schedule, Event, BusMap};
use crate::system::Cycle;

use std::fmt;

/// The Playstation has three different timers, which all have different uses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimerId {
    Tmr0,
    Tmr1,
    Tmr2,
}

impl TimerId {
    fn irq_kind(self) -> Irq {
        match self {
            TimerId::Tmr0 => Irq::Tmr0,
            TimerId::Tmr1 => Irq::Tmr1,
            TimerId::Tmr2 => Irq::Tmr2,
        }
    }

    fn from_value(val: u32) -> TimerId {
        match val {
            0 => TimerId::Tmr0,
            1 => TimerId::Tmr1,
            2 => TimerId::Tmr2,
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for TimerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            TimerId::Tmr0 => "Tmr0",
            TimerId::Tmr1 => "Tmr1",
            TimerId::Tmr2 => "Tmr2",
        })
    }
}

/// All the possible sync mode for all the timers. The kind of sync modes vary from counter to
/// counter.
#[derive(PartialEq, Eq)]
pub enum SyncMode {
    /// Pause the counter during Hblank.
    HblankPause,
    /// Reset the counter to 0 when entering Hblank.
    HblankReset,
    /// Reset the counter to 0 when entering Hblank, and pause when not in Hblank.
    HblankResetAndRun,
    /// Wait until Hblank occours and switch to free run.
    HblankWait,
    /// Pause the counter during Vblank.
    VblankPause,
    /// Reset the counter to 0 when entering Vblank.
    VblankReset,
    /// Reset the counter to 0 when entering Vblank, and pause when not in Vblank.
    VblankResetAndRun,
    /// Wait until Vblank occours and switch to free run.
    VblankWait,
    /// Stop at current value.
    Stop,
    /// Run until stopped.
    FreeRun,
}

impl fmt::Display for SyncMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            SyncMode::HblankPause => "Hblank pause",
            SyncMode::HblankReset => "Hblank reset",
            SyncMode::HblankResetAndRun => "Hblank reset and run",
            SyncMode::HblankWait => "Hblank wait",
            SyncMode::VblankPause => "Vblank pause",
            SyncMode::VblankReset => "Vblank reset",
            SyncMode::VblankResetAndRun => "Vblank reset and run",
            SyncMode::VblankWait => "Vblank wait",
            SyncMode::Stop => "stop",
            SyncMode::FreeRun => "free run",
        })
    }
}

/// The timers can take several differnt inputs, which effects the speed of the timers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClockSource {
    /// The CPU's clock speed, approx 33MHz.
    SystemClock,
    /// The GPU's clock speed, appox 53Mhz.
    DotClock,
    /// Counted each GPU Hblank.
    Hblank,
    /// The CPU's clock speed divided by 8.
    SystemClockDiv8,
}

impl ClockSource {
    fn cycles_to_ticks(self, cycles: Cycle) -> u64 {
        match self {
            ClockSource::SystemClock => cycles,
            ClockSource::SystemClockDiv8 => cycles / 8,
            ClockSource::DotClock => timing::cpu_to_gpu_cycles(cycles),
            ClockSource::Hblank => 0,
        }
    }

    fn ticks_to_cycles(self, ticks: u64) -> Cycle {
        match self {
            ClockSource::SystemClock => ticks,
            ClockSource::SystemClockDiv8 => ticks * 8,
            ClockSource::DotClock => timing::gpu_to_cpu_cycles(ticks),
            ClockSource::Hblank => 0,
        }
    }
}

impl fmt::Display for ClockSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            ClockSource::SystemClock => "system clock",
            ClockSource::DotClock => "dot clock",
            ClockSource::Hblank => "Hblank",
            ClockSource::SystemClockDiv8 => "system clock / 8",
        })
    }
}

/// The mode register.
#[derive(Clone, Copy)]
pub struct Mode(u16);

impl Mode {
    /// If the is false, then the timer effectively runs in ['SyncMode::FreeRun'] sync mode.
    pub fn sync_enabled(self) -> bool {
        self.0.bit(0)
    }

    pub fn sync_mode(self, timer: TimerId) -> SyncMode {
        match (timer, self.0.bit_range(1, 2)) {
            (TimerId::Tmr0, 0) => SyncMode::HblankPause,
            (TimerId::Tmr0, 1) => SyncMode::HblankReset,
            (TimerId::Tmr0, 2) => SyncMode::HblankResetAndRun,
            (TimerId::Tmr0, 3) => SyncMode::HblankWait,
            (TimerId::Tmr1, 0) => SyncMode::VblankPause,
            (TimerId::Tmr1, 1) => SyncMode::VblankReset,
            (TimerId::Tmr1, 2) => SyncMode::VblankResetAndRun,
            (TimerId::Tmr1, 3) => SyncMode::VblankWait,
            (TimerId::Tmr2, 0 | 3) => SyncMode::Stop,
            (TimerId::Tmr2, 1 | 2) => SyncMode::FreeRun,
            _ => unreachable!(),
        }
    }

    /// If this is true, the counter should reset whenever target is reached, otherwise the counter
    /// should reset when the counter overflows.
    pub fn reset_on_target(self) -> bool {
        self.0.bit(3)
    }

    /// If this is true, the timer triggers an interrupt when the target is reached.
    pub fn irq_on_target(self) -> bool {
        self.0.bit(4)
    }

    /// If this is true, the timer triggers an interrupt on overflow.
    pub fn irq_on_overflow(self) -> bool {
        self.0.bit(5)
    }

    /// If this is true, the timer triggers an interrupt each time it hits the target or overflows,
    /// dependent on ['irq_on_target'] and ['irq_on_overflow']. If it's false, it won't stop the
    /// timer after first interrupt, but will just avoid triggering again.
    pub fn irq_repeat(self) -> bool {
        self.0.bit(6)
    }

    /// If this is true, the timer toggles ['master_irq_flag'] after each interrupt. Otherwise, it
    /// will be set all the time except a few cycles after an interrupt.
    pub fn irq_toggle_mode(self) -> bool {
        self.0.bit(7)
    }

    /// The source of the timers clock.
    pub fn clock_source(self, timer: TimerId) -> ClockSource {
        match (timer, self.0.bit_range(8, 9)) {
            // All the timers can run at system clock speed.
            (TimerId::Tmr0 | TimerId::Tmr1, 0 | 2) | (TimerId::Tmr2, 2 | 3) => {
                ClockSource::SystemClock
            }
            (TimerId::Tmr0, 1 | 3) => ClockSource::DotClock,
            (TimerId::Tmr1, 1 | 3) => ClockSource::Hblank,
            (TimerId::Tmr2, 0 | 1) => ClockSource::SystemClockDiv8,
            _ => unreachable!(),
        }
    }

    /// This is updated whenever ['Mode'] is written to. 
    pub fn master_irq_flag(self) -> bool {
        self.0.bit(10)
    }

    /// If the target has been reached. It gets reset after reading the register.
    pub fn target_reached(self) -> bool {
        self.0.bit(11)
    }

    /// If overflow has been reached. It gets reset after reading the register.
    pub fn overflow_reached(self) -> bool {
        self.0.bit(12)
    }

    fn store(&mut self, val: u16) {
        // In toggle mode, the irq master flag is always set after each store. When not in toggle
        // mode, it will more or less always be on.
        self.set_master_irq_flag(true);
        // Bit 10..12 are readonly.
        self.0 |= val & 0x3ff;
    }

    fn load(&mut self) -> u32 {
        let val = self.0;
        // Overflow/target reached flags get's reset after each read.
        self.set_target_reached(false);
        self.set_overflow_reached(false);
        val as u32
    }

    fn set_master_irq_flag(&mut self, val: bool) {
        self.0 = self.0.set_bit(10, val);
    }

    fn set_target_reached(&mut self, val: bool) {
        self.0 = self.0.set_bit(11, val);
    }

    fn set_overflow_reached(&mut self, val: bool) {
        self.0 = self.0.set_bit(12, val);
    }
}

pub struct Timer {
    pub id: TimerId,
    pub mode: Mode,
    pub counter: u16,
    pub target: u16,
    /// This is to track if it has interrupted since last write to 'mode', since in oneshot
    /// mode it only does it once.
    has_triggered: bool,
}

impl Timer {
    fn new(id: TimerId) -> Self {
        Self {
            id,
            mode: Mode(0),
            counter: 0,
            target: 0,
            has_triggered: false,
        }
    }

    fn load(&mut self, offset: u32) -> u32 {
        match offset.bit_range(0, 3) {
            0 => {
                trace!("Timer {} counter read", self.id);
                self.counter.into()
            }
            4 => {
                trace!("Timer {} mode read", self.id);
                self.mode.load()
            }
            8 => self.target.into(),
            _ => unreachable!(),
        }
    }

    fn store(&mut self, offset: u32, value: u32) {
        match offset.bit_range(0, 3) {
            0 => {
                self.has_triggered = false;
                self.counter = value as u16;
            }
            4 => {
                self.counter = 0;
                self.has_triggered = false;
                self.mode.store(value as u16);

                trace!("Timer {} mode set", self.id);
                if self.mode.sync_enabled() {
                    warn!("Sync enabled for timer {}", self.id);
                }
            }
            8 => self.target = value as u16,
            _ => unreachable!(),
        }
    }

    fn trigger_irq(&mut self, schedule: &mut Schedule) {
        if self.mode.irq_repeat() || !self.has_triggered {
            self.has_triggered = true;
            if self.mode.master_irq_flag() {
                schedule.schedule_now(Event::IrqTrigger(self.id.irq_kind()));
            }
            if self.mode.irq_toggle_mode() {
                // In toggle mode, the irq master flag is toggled each IRQ.
                self.mode.set_master_irq_flag(!self.mode.master_irq_flag());   
            } else {
                // Nocash says that the master IRQ flag is always on when not in toggle mode except
                // for a few cycles when the IRQ is triggered. This is to emulate that.
                self.mode.set_master_irq_flag(false);
                schedule.schedule_in(20, Event::TimerIrqEnable(self.id));
            }
        }
    }

    fn target_reached(&mut self, schedule: &mut Schedule) {
        self.mode.set_target_reached(true);
        if self.mode.reset_on_target() {
            self.counter = 0;
        }
        if self.mode.irq_on_target() {
            self.trigger_irq(schedule);
        }
    }

    fn add_to_counter(&mut self, schedule: &mut Schedule, add: u16) {
        match self.counter.overflowing_add(add) {
            (value, false) => {
                self.counter = value;
                if value >= self.target {
                    self.target_reached(schedule);
                }
            }
            (value, true) => {
                let prev_counter = self.counter;
                self.counter = value;
                // If the target is between the counter before the overflow and the counter has
                // overflown, then the clock must have overflown.
                if self.target > prev_counter {
                    self.target_reached(schedule);
                }
                if self.mode.irq_on_overflow() {
                    self.trigger_irq(schedule);
                }
                self.mode.set_overflow_reached(true);
            }
        }
    }

    /// Choose the amount of cycles until this timer should run again.
    fn predict_next_irq(&self) -> Option<Cycle> {
        if !self.mode.irq_on_overflow() && !self.mode.irq_on_target() {
            return None;
        }
        /*
        if !self.mode.master_irq_flag() {
            return None;
        }
        */
        if !self.mode.irq_repeat() && self.has_triggered {
            return None;
        }
        if self.mode.clock_source(self.id) == ClockSource::Hblank {
            return None;
        }
        if self.mode.sync_enabled() && self.mode.sync_mode(self.id) == SyncMode::Stop {
            return None;
        }
        // Find some kind of target to aim at. It would be possible to calculate the exact cycle
        // and account for overflow and such, but that would likely be more costly than just
        // predicting the next overflow or target and run at that.
        let target = if self.mode.irq_on_target() {
            if self.counter >= self.target {
                u16::MAX 
            } else {
                self.target
            }
        } else {
            u16::MAX
        };
        let ticks_left = target - self.counter;
        Some(self.clock_source().ticks_to_cycles(ticks_left.into()))
    }

    fn run(&mut self, schedule: &mut Schedule, mut ticks: u64) {
        while let (value, false) = ticks.overflowing_sub(u16::MAX.into()) {
            self.add_to_counter(schedule, u16::MAX);
            ticks = value;
        };
        self.add_to_counter(schedule, ticks as u16);
    }

    fn clock_source(&self) -> ClockSource {
        self.mode.clock_source(self.id)
    }
}

pub struct Timers {
    pub timers: [(Timer, Cycle); 3],
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [
                (Timer::new(TimerId::Tmr0), 0),
                (Timer::new(TimerId::Tmr1), 0),
                (Timer::new(TimerId::Tmr2), 0),
            ],
        }
    }

    pub fn load(&mut self, schedule: &mut Schedule, offset: u32) -> u32 {
        let id = TimerId::from_value(offset.bit_range(4, 5));
        self.update_timer(schedule, id);
        let (tmr, _) = &mut self.timers[id as usize];
        let val = tmr.load(offset);
        if let Some(cycles) = tmr.predict_next_irq() {
            schedule.schedule_in(cycles, Event::RunTimer(id));
        }
        val
    }

    pub fn store(&mut self, schedule: &mut Schedule, offset: u32, val: u32) {
        let id = TimerId::from_value(offset.bit_range(4, 5));
        self.update_timer(schedule, id);
        let (tmr, _) = &mut self.timers[id as usize];
        tmr.store(offset, val);
        if let Some(cycles) = tmr.predict_next_irq() {
            schedule.unschedule(Event::RunTimer(id));
            schedule.schedule_in(cycles, Event::RunTimer(id));
        }
    }

    /// Update the timer. If the clock source is derivable from clock cycles ie. not Hblanks, then
    /// the timer gets run.
    fn update_timer(&mut self, schedule: &mut Schedule, id: TimerId) {
        let (tmr, last_update) = &mut self.timers[id as usize];
        let cycles = schedule.cycle() - *last_update;
        *last_update = schedule.cycle();
        if tmr.clock_source() != ClockSource::Hblank {
            tmr.run(schedule, tmr.clock_source().cycles_to_ticks(cycles));
        }
    }

    pub fn timer(&self, id: TimerId) -> &Timer {
        &self.timers[id as usize].0
    }

    pub fn run_timer(&mut self, schedule: &mut Schedule, id: TimerId) {
        self.update_timer(schedule, id);
        let (tmr, _) = &mut self.timers[id as usize];
        if let Some(cycles) = tmr.predict_next_irq() {
            schedule.unschedule(Event::RunTimer(id));
            schedule.schedule_in(cycles, Event::RunTimer(id));
        }
    }

    pub fn enable_irq_master_flag(&mut self, id: TimerId) {
        let (tmr, _) = &mut self.timers[id as usize];
        tmr.mode.set_master_irq_flag(true);
    }

    pub fn hblank(&mut self, schedule: &mut Schedule, count: u64) {
        let (tmr1, _) = &mut self.timers[1];
        if let ClockSource::Hblank = tmr1.clock_source() {
            tmr1.run(schedule, count);
        }
    }
}

impl BusMap for Timers {
    const BUS_BEGIN: u32 = 0x1f801100;
    const BUS_END: u32 = Self::BUS_BEGIN + 48 - 1;
}
