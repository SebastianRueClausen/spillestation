use splst_util::Bit;

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Bcd(u8);

impl Bcd {
    pub const ZERO: Self = Self(0);

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

impl fmt::Display for Bcd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:02x}", self.0)
    }
}
