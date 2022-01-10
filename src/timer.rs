use crate::{util::BitExtract, cpu::{IrqState, Irq}, timing, bus::BusMap};
use std::fmt;

/// The Playstation has three different timers, which all have different uses.
#[derive(Clone, Copy)]
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
#[derive(Clone, Copy)]
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
pub struct Mode(u32);

impl Mode {
    /// The reset value of the ['Mode'] register.
    fn new() -> Self {
        Mode(0)
    }

    /// If the is false, then the timer effectively runs in free run sync mode.
    pub fn sync_enabled(self) -> bool {
        self.0.extract_bit(0) == 1
    }

    pub fn sync_mode(self, timer: TimerId) -> SyncMode {
        match (timer, self.0.extract_bits(1, 2)) {
            (TimerId::Tmr0, 0) => SyncMode::HblankPause,
            (TimerId::Tmr0, 1) => SyncMode::HblankReset,
            (TimerId::Tmr0, 2) => SyncMode::HblankResetAndRun,
            (TimerId::Tmr0, 3) => SyncMode::HblankWait,
            // This is the same as tmr0, just with Vblank instead of Hblank.
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
        self.0.extract_bit(3) == 1
    }

    /// If this is true, the timer triggers an interrupt when the target is reached.
    pub fn irq_on_target(self) -> bool {
        self.0.extract_bit(4) == 1
    }

    /// If this is true, the timer triggers an interrupt on overflow.
    pub fn irq_on_overflow(self) -> bool {
        self.0.extract_bit(5) == 1
    }

    /// If this is true, the timer triggers an interrupt each time it hits the target or overflows,
    /// dependent on ['irq_on_target'] and ['irq_on_overflow']. If it's false, it won't stop the
    /// timer after first interrupt, but will just avoid triggering again.
    pub fn irq_repeat(self) -> bool {
        self.0.extract_bit(6) == 1
    }

    /// If this is true, the timer toggles ['master_irq_flag'] after each interrupt. Otherwise, it
    /// will be set all the time except a few cycles after an interrupt.
    pub fn irq_toggle_mode(self) -> bool {
        self.0.extract_bit(7) == 1
    }

    /// The source of the timers clock.
    pub fn clock_source(self, timer: TimerId) -> ClockSource {
        match (timer, self.0.extract_bits(8, 9)) {
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
        self.0.extract_bit(10) == 1
    }

    /// If the target has been reached. It gets reset after reading the register.
    pub fn target_reached(self) -> bool {
        self.0.extract_bit(11) == 1
    }

    /// If overflow has been reached. It gets reset after reading the register.
    pub fn overflow_reached(self) -> bool {
        self.0.extract_bit(12) == 1
    }

    fn store(&mut self, value: u32) {
        // Bit 10..12 are readonly.
        self.0 |= value & 0x3ff;
        // In toggle mode, the irq master flag is always set after each store. When not in toggle
        // mode, it will more or less always be on.
        self.set_master_irq_flag(true);
    }

    fn set_master_irq_flag(&mut self, value: bool) {
        self.0 |= (value as u32) << 10;
    }

    fn set_target_reached(&mut self) {
        self.0 |= 1 << 11; 
    }

    fn set_overflow_reached(&mut self) {
        self.0 |= 1 << 12; 
    }
}

pub struct Timer {
    pub id: TimerId,
    /// The mode register.
    pub mode: Mode,
    /// The counter increasing after each clock tick.
    pub counter: u16,
    /// A target which can be set. The timer can be configured to 
    pub target: u16,
    /// This is to track if it has interrupted since last write to ['mode'], since in oneshot
    /// mode it only does it once.
    has_triggered: bool,
}

impl Timer {
    fn new(id: TimerId) -> Self {
        Self {
            id,
            mode: Mode::new(),
            counter: 0,
            target: 0,
            has_triggered: false,
        }
    }

    fn load(&mut self, offset: u32) -> u32 {
        match offset.extract_bits(0, 3) {
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
        match offset.extract_bits(0, 3) {
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

    fn trigger_irq(&mut self, irq: &mut IrqState) {
        if self.mode.irq_repeat() || !self.has_triggered {
            self.has_triggered = true;
            if self.mode.master_irq_flag() {
                irq.trigger(self.id.irq_kind());
            }
            if self.mode.irq_toggle_mode() {
                // In toggle mode, the irq master flag is toggled each IRQ.
                self.mode.set_master_irq_flag(!self.mode.master_irq_flag());   
            }
        }
    }

    fn target_reached(&mut self, irq: &mut IrqState) {
        self.mode.set_target_reached();
        if self.mode.reset_on_target() {
            self.counter = 0;
        }
        if self.mode.irq_on_target() {
            self.trigger_irq(irq);
        }
    }

    fn add_to_counter(&mut self, irq: &mut IrqState, add: u16) {
        match self.counter.overflowing_add(add) {
            (value, false) => {
                self.counter = value;
                if value >= self.target {
                    self.target_reached(irq);
                }
            }
            (value, true) => {
                let prev_counter = self.counter;
                self.counter = value;
                // If the target is between the counter before the overflow and the counter has
                // overflown, then the clock must have overflown.
                if self.target > prev_counter {
                    self.target_reached(irq);
                }
                if self.mode.irq_on_overflow() {
                    self.trigger_irq(irq);
                }
                self.mode.set_overflow_reached(); 
            }
        }
    }

    fn run(&mut self, irq: &mut IrqState, hblanks: u64, cycles: u64) {
        // Translate CPU cycles to whatever the current clocksource. This is not very accurate.
        let mut ticks = match self.mode.clock_source(self.id) {
            ClockSource::SystemClock => cycles,
            ClockSource::SystemClockDiv8 => cycles / 8,
            ClockSource::Hblank => hblanks,
            ClockSource::DotClock => timing::cpu_to_gpu_cycles(cycles),
        };
        // If ticks is more than 0xffff, it has to be added in several steps.
        while let (value, false) = ticks.overflowing_sub(0xffff) {
            warn!("More than a single overflow in a timer run");
            self.add_to_counter(irq, 0xffff);
            ticks = value;
        };
        self.add_to_counter(irq, ticks as u16);
    }
}

pub struct Timers {
    pub timers: [Timer; 3],
    last_run: u64,
    hblanks: u64,
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [
                Timer::new(TimerId::Tmr0),
                Timer::new(TimerId::Tmr1),
                Timer::new(TimerId::Tmr2),
            ],
            last_run: 0,
            hblanks: 0,
        }
    }

    pub fn load(&mut self, irq: &mut IrqState, cycle: u64, offset: u32) -> u32 {
        self.run(irq, cycle);
        self.timers[offset.extract_bits(4, 5) as usize].load(offset)
    }

    pub fn store(&mut self, irq: &mut IrqState, cycle: u64, offset: u32, value: u32) {
        self.run(irq, cycle);
        self.timers[offset.extract_bits(4, 5) as usize].store(offset, value);
    }

    pub fn run(&mut self, irq: &mut IrqState, cycle: u64) {
        for timer in self.timers.iter_mut() {
            timer.run(irq, self.hblanks, cycle - self.last_run);
        }
        self.last_run = cycle;
        self.hblanks = 0;
    }

    pub fn hblank(&mut self, count: u64) {
        self.hblanks += count;
    }
}

impl BusMap for Timers {
    const BUS_BEGIN: u32 = 0x1f801100;
    const BUS_END: u32 = Self::BUS_BEGIN + 48 - 1;
}
