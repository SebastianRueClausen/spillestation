use crate::bcd::Bcd;

use std::ops::Sub;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Msf {
    pub min: Bcd,
    pub sec: Bcd,
    pub frame: Bcd,
}

impl Msf {
    pub const ZERO: Self = Self {
        min: Bcd::ZERO,
        sec: Bcd::ZERO,
        frame: Bcd::ZERO,
    };

    pub fn from_binary(min: u8, sec: u8, frame: u8) -> Option<Self> {
        let msf = Self {
            min: Bcd::from_binary(min)?,
            sec: Bcd::from_binary(sec)?,
            frame: Bcd::from_binary(frame)?,
        };
        Some(msf)
    }

    pub fn from_bcd(min: Bcd, sec: Bcd, frame: Bcd) -> Self {
        Self { min, sec, frame }
    }

    pub fn next_sector(&self) -> Option<Msf> {
        let msf = *self;

        if msf.frame.raw() < 0x74 {
            return Some(Msf {
                min: msf.min,
                sec: msf.sec,
                frame: msf.frame + Bcd::ONE,
            });
        }

        if msf.sec.raw() < 0x59 {
            return Some(Msf {
                min: msf.min,
                sec: msf.sec + Bcd::ONE,
                frame: Bcd::ZERO,
            });
        }

        if msf.min.raw() < 0x99 {
            return Some(Msf {
                min: msf.min + Bcd::ONE,
                sec: Bcd::ZERO,
                frame: Bcd::ZERO
            });
        }

        None
    }

    pub fn from_sector(sector: usize) -> Option<Self> {
        let min = sector / (60 * 75);
        let sector = sector % (60 * 75);
        Self::from_binary(
            min as u8,
            (sector / 75) as u8,
            (sector % 75) as u8,
        )
    }

    pub fn sector(&self) -> usize {
        let m = self.min.as_binary() as usize;
        let s = self.sec.as_binary() as usize;
        let f = self.frame.as_binary() as usize;
        (60 * 75 * m) + (75 * s) + f
    }

    pub fn checked_add(&self, other: Self) -> Option<Self> {
        Self::from_sector(self.sector() + other.sector())
    }
}

impl Sub<Msf> for Msf {
    type Output = Self;
    
    fn sub(self, other: Self) -> Self::Output {
        Self::from_sector(self.sector() - other.sector()).unwrap()
    }
}

impl fmt::Display for Msf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}.{}", self.min, self.sec, self.frame)        
    }
}
