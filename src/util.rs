
/// Trait to extract a value between two given bit positions.
pub trait BitExtract {
    /// Extract a single bit.
    fn extract_bit(self, n: u32) -> Self;

    /// Extract a range of bits. Both are inclusive.
    fn extract_bits(self, a: u32, b: u32) -> Self;
}

impl BitExtract for u32 {
    fn extract_bit(self, n: u32) -> Self {
        (self >> n) & 1
    }

    fn extract_bits(self, a: u32, b: u32) -> Self {
        let mask = ((1 << (b - a + 1)) - 1) << a;
        (self & mask) >> a
    }
}

impl BitExtract for u16 {
    fn extract_bit(self, n: u32) -> Self {
        (self >> n) & 1
    }

    fn extract_bits(self, a: u32, b: u32) -> Self {
        let mask = ((1 << (b - a + 1)) - 1) << a;
        (self & mask) >> a
    }
}

impl BitExtract for u8 {
    fn extract_bit(self, n: u32) -> Self {
        (self >> n) & 1
    }

    fn extract_bits(self, a: u32, b: u32) -> Self {
        let mask = ((1 << (b - a + 1)) - 1) << a;
        (self & mask) >> a
    }
}

impl BitExtract for i32 {
    fn extract_bit(self, n: u32) -> Self {
        (self >> n) & 1
    }

    fn extract_bits(self, a: u32, b: u32) -> Self {
        let mask = ((1 << (b - a + 1)) - 1) << a;
        (self & mask) >> a
    }
}

pub trait BitSet {
    fn set_bit(&mut self, bit: usize, val: bool);

    fn set_bit_range(&mut self, ls: usize, ms: usize, val: Self);
}

impl BitSet for u32 {
    fn set_bit(&mut self, bit: usize, val: bool) {
        *self = (*self & !(1 << bit)) | ((val as Self) << bit);
    }

    fn set_bit_range(&mut self, ls: usize, ms: usize, val: Self) {
        let mask = (1 << (ms - ls + 1)) - 1;
        *self = (*self & !(mask << ls)) | ((val & mask) << ls)
    }
}

impl BitSet for u16 {
    fn set_bit(&mut self, bit: usize, val: bool) {
        *self = (*self & !(1 << bit)) | ((val as Self) << bit);
    }

    fn set_bit_range(&mut self, ls: usize, ms: usize, val: Self) {
        let mask = (1 << (ms - ls + 1)) - 1;
        *self = (*self & !(mask << ls)) | ((val & mask) << ls)
    }
}

#[test]
fn test_set_bit_range() {
    let mut a = 0_u32;
    a.set_bit_range(3, 4, 0b11);
    assert_eq!(0b11000, a);

    let mut a = 0_u32;
    a.set_bit_range(0, 10, u32::MAX);
    assert_eq!(0b11111111111, a);

    let mut a = 0_u32;
    a.set_bit_range(0, 10, 0b0101010101);
    assert_eq!(0b0101010101, a);

    let mut a = 0_u32;
    a.set_bit_range(1, 2, 0b11);
    assert_eq!(0b110, a);
}

#[test]
fn test_set_bit() {
    let mut a = 0_u32;
    a.set_bit(0, true);
    assert_eq!(1, a);

    let mut a = 0_u32;
    a.set_bit(2, true);
    assert_eq!(0b100, a);

    let mut a = 0b111_u32;
    a.set_bit(2, false);
    assert_eq!(0b011, a);
}
