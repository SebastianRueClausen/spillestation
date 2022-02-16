use crate::Bit;

use std::ops::Add;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bcd(u8);

impl Bcd {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1);

    pub fn raw(self) -> u8 {
        self.0
    }

    pub fn from_binary(val: u8) -> Option<Self> {
        if val > 0x99 {
            None
        } else {
            let val = ((val / 10) << 4) + val % 10;
            Some(Self(val))
        }
    }

    pub fn from_bcd(val: u8) -> Option<Self> {
        if val <= 0x99 && val.bit_range(0, 3) <= 0x9 {
            Some(Self(val))
        } else {
            None
        }
    }

    pub fn as_binary(self) -> u8 {
        (self.0 >> 4) * 10 + self.0 % 16
    }
}

impl Add for Bcd {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let binary = self.as_binary() + other.as_binary(); 
        Bcd::from_binary(binary).expect("Overflow")
    }
}

impl fmt::Display for Bcd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:02x}", self.0)
    }
}
