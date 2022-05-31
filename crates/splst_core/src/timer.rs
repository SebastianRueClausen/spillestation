use splst_util::{Bit, BitSet};

use crate::cpu::Irq;
use crate::bus::BusMap;
use crate::schedule::{Schedule, Event, EventId};
use crate::{SysTime, Timestamp};
use crate::bus::{self, AddrUnit};
use crate::{dump, dump::Dumper};

use std::fmt;

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

    fn from_index(val: u32) -> TimerId {
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
            TimerId::Tmr0 => "timer 0",
            TimerId::Tmr1 => "timer 1",
            TimerId::Tmr2 => "timer 2",
        })
    }
}

/// All the possible sync mode for all the timers. The kind of sync modes vary from counter to
/// counter.
///
/// ## The naming follows the convention
///
/// - `Pause` - The timer is paused during V/H Blank.
/// - `Reset` - The timer resets to 0 when entering V/H Blank.
/// - `ResetAndRun` - Reset the counter to 0 when entering V/H Blank and pause when not in H/V Blank.
/// - `Wait` - Wait until V/H Blank and switch to `FreeRun`.
///
#[derive(PartialEq, Eq)]
pub enum SyncMode {
    HblankPause,
    HblankReset,
    HblankResetAndRun,
    HblankWait,
    VblankPause,
    VblankReset,
    VblankResetAndRun,
    VblankWait,
    Stop,
    FreeRun,
}

impl fmt::Display for SyncMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
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
        };
        f.write_str(name)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClockSource {
    SystemClock,
    // Since the dot clock runs at different speed depending in video mode, i'm not quite sure which
    // one this is. TODO: Test this.
    DotClock,
    Hblank,
    SystemClockDiv8,
}

impl ClockSource {
    fn time_to_ticks(self, time: SysTime) -> u64 {
        match self {
            ClockSource::SystemClock => time.as_cpu_cycles(),
            ClockSource::SystemClockDiv8 => time.as_cpu_cycles() / 8,
            ClockSource::DotClock => time.as_gpu_ntsc_cycles(),
            ClockSource::Hblank => 0,
        }
    }

    fn ticks_to_time(self, ticks: u64) -> SysTime {
        match self {
            ClockSource::SystemClock => SysTime::new(ticks),
            ClockSource::SystemClockDiv8 => SysTime::new(ticks * 8),
            ClockSource::DotClock => SysTime::from_gpu_ntsc_cycles(ticks),
            ClockSource::Hblank => SysTime::ZERO,
        }
    }
}

impl fmt::Display for ClockSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            ClockSource::SystemClock => "system clock",
            ClockSource::DotClock => "dot clock",
            ClockSource::Hblank => "hblank",
            ClockSource::SystemClockDiv8 => "system clock / 8",
        })
    }
}

/// The mode register.
#[derive(Clone, Copy)]
pub struct Mode(u16);

impl Mode {
    /// If the is false, then the timer effectively runs in [`SyncMode::FreeRun`] sync mode.
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
    /// dependent on [`irq_on_target`] and [`irq_on_overflow`]. If it's false, it won't stop the
    /// timer after first interrupt, but will just avoid triggering again.
    pub fn irq_repeat(self) -> bool {
        self.0.bit(6)
    }

    /// If this is true, the timer toggles [`master_irq_flag`] after each interrupt. Otherwise, it
    /// will be set all the time except a few cycles after an interrupt.
    pub fn irq_toggle_mode(self) -> bool {
        self.0.bit(7)
    }

    /// The source of the timers clock.
    pub fn clock_source(self, timer: TimerId) -> ClockSource {
        match (timer, self.0.bit_range(8, 9)) {
            (TimerId::Tmr0, 0 | 2) => ClockSource::SystemClock,
            (TimerId::Tmr1, 0 | 2) => ClockSource::SystemClock,
            (TimerId::Tmr2, 0 | 1) => ClockSource::SystemClock,
            (TimerId::Tmr0, 1 | 3) => ClockSource::DotClock,
            (TimerId::Tmr1, 1 | 3) => ClockSource::Hblank,
            (TimerId::Tmr2, 2 | 3) => ClockSource::SystemClockDiv8,
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
        // Bit 10..12 are readonly.
        self.0 |= val & 0x3ff;

        // In toggle mode, the irq master flag is always set after each store. When not in toggle
        // mode, it will more or less always be on.
        self.set_master_irq_flag(true);

        if self.sync_enabled() {
            warn!("Sync Enabled");
        }
    }

    fn load(&mut self) -> u16 {
        let val = self.0;

        // Overflow/target reached flags get's reset after each read.
        self.set_target_reached(false);
        self.set_overflow_reached(false);

        val
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
    /// This is to track if it has interrupted since last write to `mode`, since in oneshot
    /// mode it only does it once.
    has_triggered: bool,
    /// The [`EventId`] for any update events, and the timestamp.
    next_update: Option<EventId>,
}

impl Timer {
    fn new(id: TimerId) -> Self {
        Self {
            id,
            mode: Mode(0),
            counter: 0,
            target: 0,
            has_triggered: false,
            next_update: None,
        }
    }

    /// Load memory fromy timer. This ignores everything but the 4 lsb of the offset, so it won't
    /// verify that the offset actually points into this timer.
    fn load(&mut self, offset: u32) -> u16 {
        match offset.bit_range(0, 3) {
            0 => {
                trace!("timer {} counter read", self.id);
                self.counter
            }
            4 => {
                trace!("timer {} mode read", self.id);
                self.mode.load() 
            }
            8 => self.target,
            _ => unreachable!(),
        }
    }

    /// Same as `load` but without side effects.
    fn peek(&self, offset: u32) -> u16 {
        match offset.bit_range(0, 3) {
            0 => self.counter,
            4 => self.mode.0,
            8 => self.target,
            _ => unreachable!(),
        }
    }

    fn store(&mut self, offset: u32, val: u16) {
        match offset.bit_range(0, 3) {
            0 => {
                self.has_triggered = false;
                self.counter = val;
            }
            4 => {
                self.counter = 0;
                self.has_triggered = false;

                self.mode.store(val);

                trace!("Timer {} mode set", self.id);

                if self.mode.sync_enabled() {
                    warn!("Sync enabled for timer {}", self.id);
                }
            }
            8 => self.target = val,
            _ => unreachable!(),
        }
    }

    fn trigger_irq(&mut self, schedule: &mut Schedule) {
        if self.mode.irq_repeat() || !self.has_triggered {
            self.has_triggered = true;

            if self.mode.master_irq_flag() {
                schedule.trigger(Event::Irq(self.id.irq_kind()));
            }

            // In toggle mode, the irq master flag is toggled each IRQ. Otherwise it's always
            // set besides a few cycles after the IRQ has been triggered.
            if self.mode.irq_toggle_mode() {
                self.mode.set_master_irq_flag(!self.mode.master_irq_flag());   
            } else {
                self.mode.set_master_irq_flag(false);
                schedule.schedule(
                    SysTime::new(20),
                    Event::Timer(self.id, Timers::enable_irq_master_flag)
                );
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

    /// Choose the amount of time until this timer should run again.
    fn predict_next_irq(&self) -> Option<SysTime> {
        if !self.mode.irq_on_overflow() && !self.mode.irq_on_target() {
            return None;
        }

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
        Some(self.clock_source().ticks_to_time(ticks_left.into()))
    }

    fn schedule_next_run(&mut self, schedule: &mut Schedule) {
        self.next_update = self.predict_next_irq().map(|time| {
            if let Some(id) = self.next_update {
                schedule.unschedule(id); 
            }
            schedule.schedule(time, Event::Timer(self.id, Timers::run_timer))
        });
    }

    fn run(&mut self, schedule: &mut Schedule, mut ticks: u64) {
        while let Some(val) = ticks.checked_sub(u16::MAX.into()) {
            self.add_to_counter(schedule, u16::MAX);
            ticks = val;
        };
        self.add_to_counter(schedule, ticks as u16);
    }

    fn sync_mode(&self) -> SyncMode {
        self.mode.sync_mode(self.id)
    }

    fn clock_source(&self) -> ClockSource {
        self.mode.clock_source(self.id)
    }

    pub fn dump(&self, d: &mut impl Dumper) {
        dump!(d, "counter", "{}", self.counter);
        dump!(d, "target", "{}", self.counter);

        let mode = self.mode;

        dump!(d, "sync enabled", "{}", mode.sync_enabled());
        dump!(d, "sync mode", "{}", self.sync_mode());
        dump!(d, "reset on target", "{}", mode.reset_on_target());
        dump!(d, "irq on target", "{}", mode.irq_on_target());
        dump!(d, "irq on overflow", "{}", mode.irq_on_overflow());
        dump!(d, "irq repeat", "{}", mode.irq_repeat());
        dump!(d, "irq toggle mode", "{}", mode.irq_toggle_mode());
        dump!(d, "clock source", "{}", self.clock_source());
        dump!(d, "master irq flag", "{}", mode.master_irq_flag());
        dump!(d, "target reached", "{}", mode.target_reached());
        dump!(d, "overflow reached", "{}", mode.overflow_reached());

    }
}

/// The 3 timers of the Playstation.
///
/// All the timers can run simultaneously. Each timer can be configured to take different sources,
/// have different targets and what to do when reaching the target such as triggering an interrupt.
pub struct Timers {
    pub timers: [(Timer, Timestamp); 3],
}

impl Timers {
    pub(crate) fn new() -> Self {
        Self {
            timers: [
                (Timer::new(TimerId::Tmr0), Timestamp::STARTUP),
                (Timer::new(TimerId::Tmr1), Timestamp::STARTUP),
                (Timer::new(TimerId::Tmr2), Timestamp::STARTUP),
            ],
        }
    }

    /// Load memory from either timer 1, 2 or 3 depending on the offset.
    pub(crate) fn load<T: AddrUnit>(&mut self, schedule: &mut Schedule, offset: u32) -> T {
        let id = TimerId::from_index(offset.bit_range(4, 5));

        self.update_timer(schedule, id);

        let (tmr, _) = &mut self.timers[id as usize];

        // TODO: Check what happens when you read an unaligned byte for instance.
        let val = tmr.load(offset);

        tmr.schedule_next_run(schedule);

        T::from_u32(u32::from(val))
    }

    /// Same as `load` but without side effects.
    pub(crate) fn peek<T: AddrUnit>(&self, offset: u32) -> T {
        let id = TimerId::from_index(offset.bit_range(4, 5));
        let (tmr, _) = &self.timers[id as usize];

        let val = tmr.peek(bus::align_as::<u32>(offset));

        T::from_u32_aligned(u32::from(val), offset)
    }

    pub(crate) fn store<T: AddrUnit>(&mut self, schedule: &mut Schedule, offset: u32, val: T) {
        let id = TimerId::from_index(offset.bit_range(4, 5));

        self.update_timer(schedule, id);

        let (tmr, _) = &mut self.timers[id as usize];

        tmr.store(offset, val.as_u16());
        tmr.schedule_next_run(schedule);
    }

    /// Update the timer. If the clock source is derivable from clock cycles ie. not Hblanks, then
    /// the timer gets run.
    fn update_timer(&mut self, schedule: &mut Schedule, id: TimerId) {
        let (tmr, last_update) = &mut self.timers[id as usize];
        let time = schedule.now().time_since(last_update);

        *last_update = schedule.now();

        if tmr.clock_source() != ClockSource::Hblank {
            tmr.run(schedule, tmr.clock_source().time_to_ticks(time));
        }
    }

    pub fn timer(&self, id: TimerId) -> &Timer {
        &self.timers[id as usize].0
    }

    /// Update the timer and schedule the next run if required.
    pub(crate) fn run_timer(&mut self, schedule: &mut Schedule, id: TimerId) {
        self.update_timer(schedule, id);

        let (tmr, _) = &mut self.timers[id as usize];

        tmr.schedule_next_run(schedule);
    }

    pub(crate) fn enable_irq_master_flag(&mut self, _: &mut Schedule, id: TimerId) {
        let (tmr, _) = &mut self.timers[id as usize];
        tmr.mode.set_master_irq_flag(true);
    }

    pub(crate) fn hblank(&mut self, schedule: &mut Schedule, count: u64) {
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
