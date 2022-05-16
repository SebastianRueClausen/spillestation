use std::ops::{Add, Sub, Mul};
use std::time::Duration;

/// A duration or span of system run time, independent of actual time.
///
/// 48 bits are used to store CPU cycles, 33 million of which represents one second. This
/// means that it can represent a duration of approx 100 days running at native speed.
///
/// 16 bits are fractional bits for sub-cycle precision. This is to avoid rounding errors when
/// using `SysTime` to represent GPU cycles.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SysTime(u64);

impl SysTime {
    /// A duration of zero time.
    pub const ZERO: Self = Self(0);

    /// An infinite amount of time.
    pub const FOREVER: Self = Self(u64::MAX);

    /// Same as [`from_cpu_cycle`].
    pub const fn new(cycles: u64) -> Self {
        Self::from_cpu_cycles(cycles)
    }

    /// From CPU cycles.
    pub const fn from_cpu_cycles(cycles: u64) -> Self {
        Self(cycles << FRAC_BITS) 
    }
    
    /// From GPU dot cycles running in PAL mode.
    pub const fn from_gpu_pal_cycles(cycles: u64) -> Self {
        Self(cycles * CPU_CYCLES_PER_PAL_CYCLE)
    }

    /// From GPU dot cycles running in NTSC mode.
    pub const fn from_gpu_ntsc_cycles(cycles: u64) -> Self {
        Self(cycles * CPU_CYCLES_PER_NTSC_CYCLE)
    }
    
    /// From [`Duration`] the system is running at native speed.
    pub fn from_duration(duration: Duration) -> Self {
        let cycles = duration.as_nanos() * CPU_CYCLES_PER_NANO as u128;
        Self(cycles as u64) 
    }

    /// Get as CPU cycles without fractional cycles.
    pub const fn as_cpu_cycles(self) -> u64 {
        self.0 >> FRAC_BITS
    }
    
    /// Get as GPU dot cycles running in PAL mode without fractional cycles.
    pub const fn as_gpu_pal_cycles(self) -> u64 {
        ((self.0 as u128 * PAL_CYCLES_PER_CPU_CYCLE) >> FRAC_BITS * 2) as u64
    }
    
    /// Get as GPU dot cycles running in NTSC mode without fractional cycles.
    pub const fn as_gpu_ntsc_cycles(self) -> u64 {
        ((self.0 as u128 * NTSC_CYCLES_PER_CPU_CYCLE) >> FRAC_BITS * 2) as u64
    }
    
    /// Get as [`Duration`] the system is running as native speed.
    pub const fn as_duration(self) -> Duration {
        let nanos = (self.0 as u128 * NANOS_PER_CPU_CYCLE) >> FRAC_BITS * 2;
        Duration::from_nanos(nanos as u64)
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

const FRAC_BITS: u64 = 16;
const SCALING_FACTOR: f64 = (1 << FRAC_BITS) as f64;

const CPU_HZ: f64 = 33_868_800.0;
const PAL_HZ: f64 = 53_203_425.0;
const NTSC_HZ: f64 = 53_693_181.818;
const NANOS_PER_SECOND: f64 = Duration::SECOND.as_nanos() as f64;

const PAL_CYCLES_PER_CPU_CYCLE: u128 = ((PAL_HZ / CPU_HZ) * SCALING_FACTOR) as u128;
const NTSC_CYCLES_PER_CPU_CYCLE: u128 = ((NTSC_HZ / CPU_HZ) * SCALING_FACTOR) as u128;
const NANOS_PER_CPU_CYCLE: u128 = ((NANOS_PER_SECOND / CPU_HZ) * SCALING_FACTOR) as u128;

const CPU_CYCLES_PER_PAL_CYCLE: u64 = (CPU_HZ * SCALING_FACTOR / PAL_HZ) as u64;
const CPU_CYCLES_PER_NTSC_CYCLE: u64 = (CPU_HZ * SCALING_FACTOR / NTSC_HZ) as u64;
const CPU_CYCLES_PER_NANO: u64 = (CPU_HZ * SCALING_FACTOR / NANOS_PER_SECOND) as u64;
