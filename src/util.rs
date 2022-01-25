
macro_rules! impl_bit {
    ($t:ident) => {
        impl Bit for $t {
            fn bit(self, n: usize) -> bool {
                (self >> n) & 1 == 1
            }

            fn bit_range(self, ls: usize, ms: usize) -> Self {
                let mask = ((1 << (ms - ls + 1)) - 1) << ls;
                (self & mask) >> ls
            }
        }
    }
}

macro_rules! impl_bit_set {
    ($t:ident) => {
        impl BitSet for $t {
            fn set_bit(self, bit: usize, val: bool) -> Self {
                (self & !(1 << bit)) | ((val as Self) << bit)
            }

            fn set_bit_range(self, ls: usize, ms: usize, val: Self) -> Self {
                let mask = (1 << (ms - ls + 1)) - 1;
                (self & !(mask << ls)) | ((val & mask) << ls)
            }
        }
    }
}

impl_bit!(u32);
impl_bit!(i32);
impl_bit!(u16);
impl_bit!(u8);

impl_bit_set!(u32);
impl_bit_set!(i32);
impl_bit_set!(u16);

/// Trait to extract a value between two given bit positions.
pub trait Bit {
    /// Extract a single bit.
    #[must_use]
    fn bit(self, n: usize) -> bool;

    /// Extract a range of bits. Both are inclusive.
    #[must_use]
    fn bit_range(self, ls: usize, ms: usize) -> Self;
}

pub trait BitSet {
    fn set_bit(self, bit: usize, val: bool) -> Self;

    fn set_bit_range(self, ls: usize, ms: usize, val: Self) -> Self;
}

#[test]
fn test_set_bit_range() {
    let a = 0_u32.set_bit_range(3, 4, 0b11);
    assert_eq!(0b11000, a);

    let a = 0_u32.set_bit_range(0, 10, u32::MAX);
    assert_eq!(0b11111111111, a);

    let a = 0_u32.set_bit_range(0, 10, 0b0101010101);
    assert_eq!(0b0101010101, a);

    let a = 0_u32.set_bit_range(1, 2, 0b11);
    assert_eq!(0b110, a);
}

#[test]
fn test_set_bit() {
    let a = 0_u32.set_bit(0, true);
    assert_eq!(1, a);

    let a = 0_u32.set_bit(2, true);
    assert_eq!(0b100, a);

    let a = 0b111_u32.set_bit(2, false);
    assert_eq!(0b011, a);
}
